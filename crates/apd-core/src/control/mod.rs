//! Optional active AirPods control through an already installed MagicAAP driver.
//! Passive BLE advertisements and this paired-device channel stay independent.
#![cfg_attr(not(any(windows, test)), allow(dead_code))]

mod ffi;
pub mod protocol;
mod volume;
#[cfg(windows)]
mod windows;

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use protocol::{Command, Decoder, Field, Values};
use serde::Serialize;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const ACK_TIMEOUT: Duration = Duration::from_secs(3);
const READ_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Endpoint {
    pub path: String,
    pub name: String,
    pub product_id: Option<u16>,
}

impl Endpoint {
    pub fn supports_adaptive(&self) -> bool {
        matches!(
            self.product_id,
            Some(0x2014 | 0x2024 | 0x2027 | 0x201b | 0x2030 | 0x2036 | 0x202d)
        )
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub phase: &'static str,
    pub device: Option<Endpoint>,
    #[serde(flatten)]
    pub values: Values,
    pub adaptive_supported: bool,
    pub duck_enabled: bool,
    pub volume_message: Option<String>,
    pub message: String,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            phase: "disconnected",
            device: None,
            values: Values::default(),
            adaptive_supported: false,
            duck_enabled: false,
            volume_message: None,
            message: "请选择已配对的耳机并连接控制通道".into(),
        }
    }
}

trait Transport {
    fn write(&mut self, packet: &[u8]) -> Result<(), String>;
    /// None is a timeout; an empty packet is a disconnected device.
    fn read(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, String>;
    fn duck(&mut self, _enabled: bool) -> Result<(), String> {
        Ok(())
    }
}

struct Request {
    command: Command,
    reply: mpsc::Sender<Result<(), String>>,
}
struct Session {
    commands: mpsc::SyncSender<Request>,
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

pub struct ControlHub {
    session: Mutex<Option<Session>>,
    state: Arc<Mutex<Snapshot>>,
}

pub static CONTROL: Lazy<ControlHub> = Lazy::new(|| ControlHub {
    session: Mutex::new(None),
    state: Arc::new(Mutex::new(Snapshot::default())),
});

pub fn endpoints() -> Result<Vec<Endpoint>, String> {
    #[cfg(windows)]
    {
        windows::endpoints()
    }
    #[cfg(not(windows))]
    {
        Err("耳机控制仅支持 Windows 与已安装的 MagicAAP 驱动".into())
    }
}

impl ControlHub {
    pub fn snapshot(&self) -> Snapshot {
        self.state.lock().clone()
    }

    /// Call on a worker thread: device open, handshake and old session teardown may wait.
    pub fn connect(&self, path: &str) -> Result<(), String> {
        let endpoint = endpoints()?
            .into_iter()
            .find(|e| e.path == path)
            .ok_or("所选控制设备已不可用，请刷新设备列表")?;
        let mut session = self.session.lock();
        Self::stop(session.take());
        *self.state.lock() = Snapshot {
            phase: "connecting",
            adaptive_supported: endpoint.supports_adaptive(),
            device: Some(endpoint.clone()),
            message: "正在连接耳机控制通道…".into(),
            ..Snapshot::default()
        };
        self.start(endpoint, &mut session)
    }

    #[cfg(windows)]
    fn start(&self, endpoint: Endpoint, session: &mut Option<Session>) -> Result<(), String> {
        let state = Arc::clone(&self.state);
        let (tx, rx) = mpsc::sync_channel(8);
        let (ready_tx, ready_rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let thread = std::thread::Builder::new()
            .name("apd-aap-control".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    match windows::Device::open(&endpoint.path) {
                        Ok(mut device) => run(&mut device, &state, &worker_stop, rx, &ready_tx),
                        Err(error) => Err(error),
                    }
                }))
                .unwrap_or_else(|_| Err("控制线程发生内部错误，请重新连接".into()));
                if let Err(error) = result {
                    let mut snapshot = state.lock();
                    snapshot.phase = "error";
                    snapshot.values = Values::default();
                    snapshot.message = error.clone();
                    let _ = ready_tx.send(Err(error));
                }
            })
            .map_err(|e| {
                let error = format!("无法启动控制线程: {e}");
                let mut state = self.state.lock();
                state.phase = "error";
                state.message = error.clone();
                error
            })?;
        let result = ready_rx
            .recv_timeout(Duration::from_secs(8))
            .unwrap_or_else(|_| Err("连接控制通道超时，请检查耳机连接和驱动".into()));
        if let Err(error) = &result {
            stop.store(true, Ordering::Release);
            let _ = thread.join();
            let mut state = self.state.lock();
            state.phase = "error";
            state.message = error.clone();
        } else {
            *session = Some(Session {
                commands: tx,
                stop,
                thread,
            });
        }
        result
    }

    #[cfg(not(windows))]
    fn start(&self, _endpoint: Endpoint, _session: &mut Option<Session>) -> Result<(), String> {
        Err("耳机控制仅支持 Windows".into())
    }

    pub fn disconnect(&self) {
        let mut session = self.session.lock();
        Self::stop(session.take());
        *self.state.lock() = Snapshot::default();
    }

    fn stop(session: Option<Session>) {
        if let Some(session) = session {
            session.stop.store(true, Ordering::Release);
            let _ = session.thread.join();
        }
    }

    pub fn set(&self, setting: u32, value: u32) -> Result<(), String> {
        let command = Command::from_ui(setting, value)?;
        let session = self.session.lock();
        let session = session.as_ref().ok_or("请先连接耳机控制通道")?;
        let (reply, receive) = mpsc::channel();
        session
            .commands
            .try_send(Request { command, reply })
            .map_err(|_| "控制通道忙或已断开")?;
        receive
            .recv_timeout(Duration::from_secs(6))
            .map_err(|_| "等待耳机确认超时")?
    }

    pub fn set_duck_enabled(&self, enabled: bool) {
        self.state.lock().duck_enabled = enabled;
    }
}

fn product_id(path: &str) -> Option<u16> {
    let path = path.to_ascii_lowercase();
    let start = path.find("pid&")? + 4;
    u16::from_str_radix(path.get(start..start + 4)?, 16).ok()
}

// Also compiled for non-Windows unit tests, with no OS or live-device dependency.
#[cfg_attr(not(windows), allow(dead_code))]
fn run(
    device: &mut impl Transport,
    state: &Mutex<Snapshot>,
    stop: &AtomicBool,
    commands: mpsc::Receiver<Request>,
    ready: &mpsc::Sender<Result<(), String>>,
) -> Result<(), String> {
    let mut decoder = Decoder::default();
    device.write(&protocol::HANDSHAKE)?;
    let deadline = Instant::now() + ACK_TIMEOUT;
    let mut handshake = Vec::new();
    loop {
        if stop.load(Ordering::Acquire) {
            return Err("连接已取消".into());
        }
        if Instant::now() >= deadline {
            return Err("耳机未确认控制握手，请检查 MagicAAP 驱动及蓝牙连接".into());
        }
        if let Some(data) = device.read(READ_INTERVAL)? {
            if data.is_empty() {
                return Err("耳机控制连接已断开".into());
            }
            apply_data(state, &mut decoder, &data);
            handshake.extend_from_slice(&data);
            if handshake.windows(4).any(|w| w == protocol::HANDSHAKE_ACK) {
                break;
            }
            // Retain only a split handshake prefix, not unrelated traffic.
            let keep = handshake.len().min(3);
            handshake.drain(..handshake.len() - keep);
        }
    }
    device.write(&protocol::NOTIFICATIONS)?;
    device.write(&protocol::FEATURES)?;
    {
        let mut state = state.lock();
        state.phase = "connected";
        state.message = "控制通道已连接，等待耳机回传设置".into();
    }
    let _ = ready.send(Ok(()));
    while !stop.load(Ordering::Acquire) {
        if let Ok(request) = commands.try_recv() {
            let result = apply_command(device, state, stop, &mut decoder, request.command);
            let _ = request.reply.send(result);
        }
        read_updates(device, state, &mut decoder)?;
    }
    // Windows transport also restores a captured ducked volume on Drop.
    device.duck(false)?;
    Ok(())
}

fn apply_data(state: &Mutex<Snapshot>, decoder: &mut Decoder, data: &[u8]) {
    let mut snapshot = state.lock();
    for notification in decoder.feed(data) {
        snapshot.values.apply(notification);
    }
    if snapshot.values.listening_mode == Some(4) {
        snapshot.adaptive_supported = true;
    }
    if snapshot.phase == "connected"
        && (snapshot.values.listening_mode.is_some()
            || snapshot.values.conversation_awareness.is_some()
            || snapshot.values.one_ear_anc.is_some())
    {
        snapshot.message = "控制通道已连接".into();
    }
}

fn read_updates(
    device: &mut impl Transport,
    state: &Mutex<Snapshot>,
    decoder: &mut Decoder,
) -> Result<(), String> {
    if let Some(data) = device.read(READ_INTERVAL)? {
        if data.is_empty() {
            return Err("耳机控制连接已断开，请重新连接".into());
        }
        apply_data(state, decoder, &data);
    }
    let duck = {
        let s = state.lock();
        s.duck_enabled
            && s.values.conversation_awareness == Some(true)
            && s.values.speaking == Some(true)
    };
    let result = device.duck(duck);
    state.lock().volume_message = result.err();
    Ok(())
}

fn validate(snapshot: &Snapshot, command: Command) -> Result<(), String> {
    if snapshot.phase != "connected" {
        return Err("耳机控制通道未连接".into());
    }
    if !snapshot.values.has(command.field) {
        return Err("耳机尚未回传此设置，或不支持此功能".into());
    }
    if command.field == Field::ListeningMode && command.value == 4 && !snapshot.adaptive_supported {
        return Err("尚未确认此耳机支持自适应模式".into());
    }
    Ok(())
}

fn apply_command(
    device: &mut impl Transport,
    state: &Mutex<Snapshot>,
    stop: &AtomicBool,
    decoder: &mut Decoder,
    command: Command,
) -> Result<(), String> {
    {
        let snapshot = state.lock();
        validate(&snapshot, command)?;
        if snapshot.values.matches(command) {
            return Ok(());
        }
    }
    device.write(&command.packet())?;
    let deadline = Instant::now() + ACK_TIMEOUT;
    while Instant::now() < deadline {
        if stop.load(Ordering::Acquire) {
            return Err("控制操作已取消".into());
        }
        read_updates(device, state, decoder)?;
        if state.lock().values.matches(command) {
            return Ok(());
        }
    }
    Err("指令已发送，但耳机未回传确认；显示值保持最后确认的状态".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[derive(Default)]
    struct Fake {
        writes: Vec<Vec<u8>>,
        reads: VecDeque<Vec<u8>>,
        fail_write: bool,
        stop_on_empty: Option<Arc<AtomicBool>>,
        duck_calls: Vec<bool>,
    }
    impl Transport for Fake {
        fn write(&mut self, p: &[u8]) -> Result<(), String> {
            if self.fail_write {
                return Err("write failed".into());
            }
            self.writes.push(p.to_vec());
            Ok(())
        }
        fn read(&mut self, _: Duration) -> Result<Option<Vec<u8>>, String> {
            let data = self.reads.pop_front();
            if data.is_none() {
                if let Some(stop) = &self.stop_on_empty {
                    stop.store(true, Ordering::Release);
                } else {
                    std::thread::sleep(READ_INTERVAL);
                }
            }
            Ok(data)
        }
        fn duck(&mut self, enabled: bool) -> Result<(), String> {
            self.duck_calls.push(enabled);
            Ok(())
        }
    }
    fn connected() -> Mutex<Snapshot> {
        Mutex::new(Snapshot {
            phase: "connected",
            values: Values {
                listening_mode: Some(2),
                ..Values::default()
            },
            ..Snapshot::default()
        })
    }

    #[test]
    fn unsupported_settings_and_adaptive_are_rejected_before_writing() {
        let s = connected();
        let mut fake = Fake::default();
        let mut d = Decoder::default();
        for command in [
            Command::from_ui(1, 1).unwrap(),
            Command::from_ui(0, 4).unwrap(),
        ] {
            assert!(
                apply_command(&mut fake, &s, &AtomicBool::new(false), &mut d, command).is_err()
            );
        }
        assert!(fake.writes.is_empty());
    }

    #[test]
    fn setting_changes_only_after_matching_device_notification() {
        let s = connected();
        let command = Command::from_ui(0, 3).unwrap();
        let mut fake = Fake {
            reads: VecDeque::from([command.packet().to_vec()]),
            ..Fake::default()
        };
        apply_command(
            &mut fake,
            &s,
            &AtomicBool::new(false),
            &mut Decoder::default(),
            command,
        )
        .unwrap();
        assert_eq!(s.lock().values.listening_mode, Some(3));
        assert_eq!(fake.writes, [command.packet()]);
    }

    #[test]
    fn failed_write_and_cancellation_preserve_confirmed_value() {
        let s = connected();
        let command = Command::from_ui(0, 3).unwrap();
        let mut d = Decoder::default();
        let mut fake = Fake {
            fail_write: true,
            ..Fake::default()
        };
        assert!(apply_command(&mut fake, &s, &AtomicBool::new(false), &mut d, command).is_err());
        fake.fail_write = false;
        assert!(apply_command(&mut fake, &s, &AtomicBool::new(true), &mut d, command).is_err());
        assert_eq!(s.lock().values.listening_mode, Some(2));
    }

    #[test]
    fn reconnect_state_resets_capabilities_and_values() {
        let s = connected();
        s.lock().values.conversation_awareness = Some(true);
        *s.lock() = Snapshot::default();
        assert_eq!(s.lock().values, Values::default());
        assert!(!s.lock().adaptive_supported);
        assert!(!s.lock().duck_enabled);
    }

    #[test]
    fn product_identity_is_taken_from_driver_not_random_ble_address() {
        assert_eq!(
            product_id("BTHENUM#VID&0001004C_PID&2027#DEVICE"),
            Some(0x2027)
        );
        assert_eq!(product_id("BTHENUM#pid&nope"), None);
        assert_eq!(product_id("BTHENUM#pid&20"), None);
        let endpoint = Endpoint {
            path: String::new(),
            name: String::new(),
            product_id: Some(0x2027),
        };
        assert!(endpoint.supports_adaptive());
        assert!(!Endpoint {
            product_id: Some(0x200e),
            ..endpoint
        }
        .supports_adaptive());
    }

    #[test]
    fn handshake_subscription_and_conversation_lifecycle() {
        let stop = Arc::new(AtomicBool::new(false));
        let s = Mutex::new(Snapshot {
            duck_enabled: true,
            ..Snapshot::default()
        });
        let mut update = Command::from_ui(1, 1).unwrap().packet().to_vec();
        update.extend_from_slice(&[4, 0, 4, 0, 0x4b, 0, 2, 0, 1, 1]);
        let mut fake = Fake {
            reads: VecDeque::from([vec![1, 0], vec![4, 0], update]),
            stop_on_empty: Some(Arc::clone(&stop)),
            ..Fake::default()
        };
        let (_tx, rx) = mpsc::sync_channel(8);
        let (ready, received) = mpsc::channel();
        run(&mut fake, &s, &stop, rx, &ready).unwrap();
        received.recv().unwrap().unwrap();
        assert_eq!(
            fake.writes,
            [
                protocol::HANDSHAKE.to_vec(),
                protocol::NOTIFICATIONS.to_vec(),
                protocol::FEATURES.to_vec()
            ]
        );
        assert_eq!(s.lock().values.conversation_awareness, Some(true));
        assert_eq!(s.lock().values.speaking, Some(true));
        assert!(fake.duck_calls.contains(&true));
        assert_eq!(fake.duck_calls.last(), Some(&false));
    }

    #[test]
    fn disconnected_handshake_never_subscribes_or_claims_connected() {
        let mut fake = Fake {
            reads: VecDeque::from([vec![]]),
            ..Fake::default()
        };
        let s = Mutex::new(Snapshot::default());
        let (_tx, rx) = mpsc::sync_channel(8);
        let (ready, _received) = mpsc::channel();
        assert!(run(&mut fake, &s, &AtomicBool::new(false), rx, &ready).is_err());
        assert_eq!(fake.writes, [protocol::HANDSHAKE]);
        assert_ne!(s.lock().phase, "connected");
    }

    #[test]
    fn silence_after_write_times_out_without_optimistic_state_change() {
        let s = connected();
        let mut fake = Fake::default();
        let error = apply_command(
            &mut fake,
            &s,
            &AtomicBool::new(false),
            &mut Decoder::default(),
            Command::from_ui(0, 3).unwrap(),
        )
        .unwrap_err();
        assert!(error.contains("未回传确认"));
        assert_eq!(s.lock().values.listening_mode, Some(2));
    }
}
