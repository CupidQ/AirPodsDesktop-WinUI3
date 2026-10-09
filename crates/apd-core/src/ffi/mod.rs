//! C ABI surface for the WinUI 3 (C#) frontend.
//!
//! # Ownership & threading
//! - All `apd_*` functions are safe to call from any thread unless documented.
//! - Callbacks are invoked on background threads; the UI must marshal to the
//!   dispatcher itself (`DispatcherQueue.TryEnqueue`).
//! - Strings returned through out-parameters are UTF-8, owned by the caller
//!   via [`apd_string_free`].
//!
//! # State encoding
//! [`ApdDeviceState`] is a blittable struct matching the C# `NativeDeviceState`
//! layout (sequential, pack 8).

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

use once_cell::sync::OnceCell;
use parking_lot::Mutex;

use crate::protocol::AirPodsPacket;
use crate::settings::SettingsStore;
use crate::state::{CaseState, DeviceState, Manager, ManagerEvent, PodState};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// C-compatible battery info.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ApdBattery {
    /// 0–100, or -1 when unavailable.
    pub percent: i32,
    pub is_charging: u8,
    pub _pad: [u8; 3],
}

impl From<crate::state::battery::BatteryState> for ApdBattery {
    fn from(b: crate::state::battery::BatteryState) -> Self {
        ApdBattery {
            percent: b.percent.map(|p| p as i32).unwrap_or(-1),
            is_charging: b.is_charging as u8,
            _pad: [0; 3],
        }
    }
}

/// C-compatible per-ear state.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ApdPod {
    pub battery: ApdBattery,
    pub is_in_ear: u8,
    pub _pad: [u8; 7],
}

/// C-compatible case state.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ApdCase {
    pub battery: ApdBattery,
    pub is_both_pods_in_case: u8,
    pub is_lid_opened: u8,
    pub _pad: [u8; 2],
}

/// C-compatible device snapshot (must match C# `NativeDeviceState`).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ApdDeviceState {
    pub model: u32,
    pub left: ApdPod,
    pub right: ApdPod,
    pub case: ApdCase,
    pub rssi: i16,
    pub _pad: [u8; 6],
}

impl From<&DeviceState> for ApdDeviceState {
    fn from(s: &DeviceState) -> Self {
        ApdDeviceState {
            model: s.model.as_ffi(),
            left: ApdPod {
                battery: s.left.battery.into(),
                is_in_ear: s.left.is_in_ear as u8,
                _pad: [0; 7],
            },
            right: ApdPod {
                battery: s.right.battery.into(),
                is_in_ear: s.right.is_in_ear as u8,
                _pad: [0; 7],
            },
            case: ApdCase {
                battery: s.case.battery.into(),
                is_both_pods_in_case: s.case.is_both_pods_in_case as u8,
                is_lid_opened: s.case.is_lid_opened as u8,
                _pad: [0; 2],
            },
            rssi: s.rssi,
            _pad: [0; 6],
        }
    }
}

/// Event codes delivered to [`ApdEventCallback`].
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApdEvent {
    StateUpdated = 0,
    LidToggled = 1,
    BothInEar = 2,
    Lost = 3,
    Disconnected = 4,
    ScannerStarted = 5,
    ScannerStopped = 6,
    LowBattery = 7,
}

/// `void (*)(uint32_t event, const ApdDeviceState* state, int32_t aux, void* user_data)`
///
/// `state` is null for Lost/Disconnected/Scanner* events.
/// `aux`: LidToggled → 1 opened / 0 closed; BothInEar → 1/0; Scanner* → error flag.
pub type ApdEventCallback =
    Option<unsafe extern "C" fn(event: u32, state: *const ApdDeviceState, aux: i32, user_data: *mut c_void)>;

// ---------------------------------------------------------------------------
// Global app context
// ---------------------------------------------------------------------------

struct AppContext {
    manager: Manager,
    settings: SettingsStore,
    #[cfg(windows)]
    watcher: Mutex<Option<crate::ble::BleWatcher>>,
    callback: Mutex<Option<(ApdEventCallback, *mut c_void)>>,
    // Keep handlers alive for the lifetime of the process.
    _handler_guard: Mutex<Option<()>>,
}

unsafe impl Send for AppContext {}
unsafe impl Sync for AppContext {}

static APP: OnceCell<AppContext> = OnceCell::new();

fn app() -> &'static AppContext {
    APP.get_or_init(|| {
        let settings = SettingsStore::new();
        let manager = Manager::new();
        let s = settings.get();
        manager.set_rssi_min(s.rssi_min);
        manager.set_bound_address(s.bound_device_address);

        AppContext {
            manager,
            settings,
            #[cfg(windows)]
            watcher: Mutex::new(None),
            callback: Mutex::new(None),
            _handler_guard: Mutex::new(None),
        }
    })
}

fn emit(event: ApdEvent, state: Option<&DeviceState>, aux: i32) {
    let ctx = app();
    let cb = ctx.callback.lock();
    if let Some((Some(f), user)) = *cb {
        let local: ApdDeviceState = state.map(ApdDeviceState::from).unwrap_or_default();
        let ptr = if state.is_some() {
            &local as *const ApdDeviceState
        } else {
            std::ptr::null()
        };
        unsafe { f(event as u32, ptr, aux, user) };
    }
}

fn emit_raw(event: ApdEvent, state: &ApdDeviceState, aux: i32) {
    let ctx = app();
    let cb = ctx.callback.lock();
    if let Some((Some(f), user)) = *cb {
        unsafe { f(event as u32, state as *const ApdDeviceState, aux, user) };
    }
}

// ---------------------------------------------------------------------------
// API
// ---------------------------------------------------------------------------

/// Library version (UTF-8). Caller frees with [`apd_string_free`].
#[no_mangle]
pub unsafe extern "C" fn apd_version() -> *mut c_char {
    match CString::new(crate::VERSION) {
        Ok(s) => s.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Free a string returned by this library.
#[no_mangle]
pub unsafe extern "C" fn apd_string_free(s: *mut c_char) {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}

/// Register the global event callback. Pass null to clear.
///
/// # Safety
/// `user_data` must remain valid until the callback is replaced or the process exits.
#[no_mangle]
pub unsafe extern "C" fn apd_set_event_callback(cb: ApdEventCallback, user_data: *mut c_void) {
    let ctx = app();
    *ctx.callback.lock() = Some((cb, user_data));
}

/// Initialize core (loads settings, wires listeners). Idempotent.
#[no_mangle]
pub extern "C" fn apd_init() -> i32 {
    let result = catch_unwind(|| {
        let ctx = app();
        let mut initialized = ctx._handler_guard.lock();
        if initialized.is_some() {
            return;
        }
        // Wire manager events → FFI callback.
        let listener: crate::state::manager::Listener = Arc::new(|event| match event {
            ManagerEvent::StateUpdated(up) => {
                emit(ApdEvent::StateUpdated, Some(&up.new), 0);
                if up.new.has_low_battery() {
                    emit(ApdEvent::LowBattery, Some(&up.new), 0);
                }
            }
            ManagerEvent::LidToggled { opened } => {
                emit(ApdEvent::LidToggled, None, opened as i32);
            }
            ManagerEvent::BothInEar { in_ear } => {
                emit(ApdEvent::BothInEar, None, in_ear as i32);
            }
            ManagerEvent::Lost => emit(ApdEvent::Lost, None, 0),
            ManagerEvent::Disconnected => emit(ApdEvent::Disconnected, None, 0),
        });
        ctx.manager.add_listener(listener);
        *initialized = Some(());
    });
    if result.is_ok() { 0 } else { -99 }
}

/// Start the BLE advertisement scanner.
///
/// On Windows this creates a `BluetoothLEAdvertisementWatcher`.
/// Returns 0 on success, negative on error.
#[no_mangle]
pub extern "C" fn apd_start_scanner() -> i32 {
    #[cfg(windows)]
    {
        let result = catch_unwind(AssertUnwindSafe(|| {
            let ctx = app();
            let mut guard = ctx.watcher.lock();
            if guard.as_ref().is_some_and(|watcher| watcher.is_running()) {
                return 0;
            }
            // Recreate a watcher that was aborted (for example when Bluetooth was disabled).
            guard.take();
            match crate::ble::BleWatcher::new() {
                Ok(watcher) => {
                    let manager = ctx.manager.clone();
                    watcher.set_on_received(Arc::new(move |adv| {
                        if let Some(payload) = adv.apple_payload() {
                            if let Some(packet) = AirPodsPacket::parse(payload) {
                                manager.on_advertisement(packet, adv.address, adv.rssi);
                            }
                        }
                    }));
                    watcher.set_on_state(Arc::new(|state, err| {
                        let aux = if err.is_some() { 1 } else { 0 };
                        match state {
                            crate::ble::WatcherState::Started => {
                                emit(ApdEvent::ScannerStarted, None, 0)
                            }
                            crate::ble::WatcherState::Stopped => {
                                emit(ApdEvent::ScannerStopped, None, aux)
                            }
                        }
                    }));
                    match watcher.start() {
                        Ok(()) => {
                            *guard = Some(watcher);
                            0
                        }
                        Err(e) => {
                            log::warn!("failed to start BLE watcher: {e}");
                            -2
                        }
                    }
                }
                Err(e) => {
                    log::warn!("failed to create BLE watcher: {e}");
                    -1
                }
            }
        }));
        match result {
            Ok(code) => code,
            Err(_) => -99,
        }
    }
    #[cfg(not(windows))]
    {
        -100 // unsupported platform
    }
}

/// Stop the BLE scanner.
#[no_mangle]
pub extern "C" fn apd_stop_scanner() -> i32 {
    #[cfg(windows)]
    {
        let ctx = app();
        let mut guard = ctx.watcher.lock();
        if let Some(w) = guard.take() {
            let _ = w.stop();
        }
        0
    }
    #[cfg(not(windows))]
    {
        -100
    }
}

/// Read the current merged device state into `out`.
/// Returns 1 when a state is available, 0 when none.
#[no_mangle]
pub extern "C" fn apd_get_state(out: *mut ApdDeviceState) -> i32 {
    if out.is_null() {
        return -1;
    }
    match app().manager.current_state() {
        Some(s) => {
            let converted = ApdDeviceState::from(&s);
            unsafe {
                *out = converted;
            }
            1
        }
        None => 0,
    }
}

/// Display name of the current model (UTF-8). Caller frees with [`apd_string_free`].
#[no_mangle]
pub extern "C" fn apd_get_display_name() -> *mut c_char {
    let name = app()
        .manager
        .current_state()
        .map(|s| s.display_name)
        .unwrap_or_else(|| "AirPods".into());
    CString::new(name).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut())
}

/// Current advertising address; zero when no device is available.
#[no_mangle]
pub extern "C" fn apd_get_device_address() -> u64 {
    app().manager.current_address().unwrap_or(0)
}

/// Request the core to poll lost-detection (call from a UI timer ~1 Hz).
#[no_mangle]
pub extern "C" fn apd_tick() {
    app().manager.tick();
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ApdSettings {
    pub rssi_min: i16,
    pub _pad0: [u8; 6],
    pub bound_device_address: u64,
    pub automatic_ear_detection: u8,
    pub low_audio_latency: u8,
    pub auto_start: u8,
    pub show_popup_on_connect: u8,
    pub _pad1: [u8; 4],
}

impl From<crate::settings::Settings> for ApdSettings {
    fn from(s: crate::settings::Settings) -> Self {
        ApdSettings {
            rssi_min: s.rssi_min,
            _pad0: [0; 6],
            bound_device_address: s.bound_device_address,
            automatic_ear_detection: s.automatic_ear_detection as u8,
            low_audio_latency: s.low_audio_latency as u8,
            auto_start: s.auto_start as u8,
            show_popup_on_connect: s.show_popup_on_connect as u8,
            _pad1: [0; 4],
        }
    }
}

#[no_mangle]
pub extern "C" fn apd_get_settings(out: *mut ApdSettings) -> i32 {
    if out.is_null() {
        return -1;
    }
    let s = ApdSettings::from(app().settings.get());
    unsafe { *out = s };
    0
}

#[no_mangle]
pub extern "C" fn apd_set_settings(settings: ApdSettings) -> i32 {
    let ctx = app();
    let store = ctx.settings.clone();
    let manager = ctx.manager.clone();
    store.update(|s| {
        s.rssi_min = settings.rssi_min;
        s.bound_device_address = settings.bound_device_address;
        s.automatic_ear_detection = settings.automatic_ear_detection != 0;
        s.low_audio_latency = settings.low_audio_latency != 0;
        s.auto_start = settings.auto_start != 0;
        s.show_popup_on_connect = settings.show_popup_on_connect != 0;
    });
    let s = store.get();
    manager.set_rssi_min(s.rssi_min);
    manager.set_bound_address(s.bound_device_address);
    0
}

/// Theme string ("system" / "light" / "dark"). Caller frees.
#[no_mangle]
pub extern "C" fn apd_get_theme() -> *mut c_char {
    let theme = app().settings.get().theme;
    CString::new(theme).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn apd_set_theme(theme: *const c_char) -> i32 {
    if theme.is_null() {
        return -1;
    }
    let s = match CStr::from_ptr(theme).to_str() {
        Ok(s) => s.to_owned(),
        Err(_) => return -1,
    };
    if !matches!(s.as_str(), "system" | "light" | "dark") {
        return -2;
    }
    app().settings.update(|st| st.theme = s);
    0
}

// ---------------------------------------------------------------------------
// Media control (ear detection)
// ---------------------------------------------------------------------------

/// Pause global media playback.
#[no_mangle]
pub extern "C" fn apd_media_pause() -> i32 {
    #[cfg(windows)]
    {
        media_command(false)
    }
    #[cfg(not(windows))]
    {
        -100
    }
}

/// Resume global media playback.
#[no_mangle]
pub extern "C" fn apd_media_play() -> i32 {
    #[cfg(windows)]
    {
        media_command(true)
    }
    #[cfg(not(windows))]
    {
        -100
    }
}

#[cfg(windows)]
fn media_command(play: bool) -> i32 {
    use windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED};
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) { unsafe { RoUninitialize() }; }
    }
    // Called from a C# worker thread, never the WinUI STA thread.
    if unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_err() {
        return -1;
    }
    let _apartment = Apartment;
    let result = catch_unwind(AssertUnwindSafe(|| {
        let controller = crate::media::MediaController::new()?;
        if play { controller.play_blocking() } else { controller.pause_blocking() }
    }));
    match result {
        Ok(Ok(true)) => 0,
        Ok(Ok(false)) => -3,
        Ok(Err(error)) => { log::warn!("SMTC command failed: {error}"); -2 },
        Err(_) => -99,
    }
}

// ---------------------------------------------------------------------------
// Diagnostics
// ---------------------------------------------------------------------------

/// Parse a raw 27-byte manufacturer payload and write a debug description.
/// `out` must have capacity ≥ `out_len`. Returns bytes written (excluding NUL).
#[no_mangle]
pub unsafe extern "C" fn apd_debug_parse(
    data: *const u8,
    data_len: usize,
    out: *mut c_char,
    out_len: usize,
) -> i32 {
    if data.is_null() || out.is_null() || out_len == 0 {
        return -1;
    }
    let slice = std::slice::from_raw_parts(data, data_len);
    let text = match AirPodsPacket::parse(slice) {
        Some(p) => format!(
            "model={} id=0x{:04X} side={:?} L={:?} R={:?} C={:?}",
            p.model.display_name(),
            p.model_id,
            p.broadcast_side,
            p.left,
            p.right,
            p.case
        ),
        None => "invalid packet".into(),
    };
    let bytes = text.as_bytes();
    let n = bytes.len().min(out_len - 1);
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), out as *mut u8, n);
    *out.add(n) = 0;
    n as i32
}

// Silence unused warnings for types referenced only in docs / tests.
#[allow(unused)]
fn _touch_types(_: PodState, _: CaseState) {
    let _ = emit_raw as fn(ApdEvent, &ApdDeviceState, i32);
}
