//! Windows BLE advertisement watcher built on `Windows.Devices.Bluetooth.Advertisement`.
//!
//! Ported from `Source/Core/Bluetooth_win.cpp`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use windows::Devices::Bluetooth::Advertisement::{
    BluetoothLEAdvertisementReceivedEventArgs, BluetoothLEAdvertisementWatcher,
    BluetoothLEAdvertisementWatcherStoppedEventArgs, BluetoothLEAdvertisementWatcherStatus,
};
use windows::Foundation::{EventRegistrationToken, TypedEventHandler};

use crate::ble::{ManufacturerData, ReceivedAdvertisement};

/// Callback for each advertisement that carries manufacturer data.
pub type OnReceived = Arc<dyn Fn(ReceivedAdvertisement) + Send + Sync>;
/// Callback when the watcher starts or stops.
pub type OnState = Arc<dyn Fn(WatcherState, Option<String>) + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatcherState {
    Started,
    Stopped,
}

#[derive(Debug)]
pub struct WatcherError(pub String);

impl std::fmt::Display for WatcherError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BLE watcher error: {}", self.0)
    }
}

impl std::error::Error for WatcherError {}

/// Event args passed to callbacks (owned copies).
#[derive(Debug, Clone)]
pub struct AdvertisementData {
    pub address: u64,
    pub rssi: i16,
    pub timestamp: i64,
    pub manufacturer_data: Vec<ManufacturerData>,
}

struct Callbacks {
    on_received: Option<OnReceived>,
    on_state: Option<OnState>,
}

/// Owns a `BluetoothLEAdvertisementWatcher` and dispatches events.
pub struct BleWatcher {
    watcher: BluetoothLEAdvertisementWatcher,
    received_token: Mutex<Option<EventRegistrationToken>>,
    stopped_token: Mutex<Option<EventRegistrationToken>>,
    callbacks: Arc<Mutex<Callbacks>>,
    running: Arc<AtomicBool>,
}

impl BleWatcher {
    pub fn new() -> windows::core::Result<Self> {
        let watcher = BluetoothLEAdvertisementWatcher::new()?;
        Ok(BleWatcher {
            watcher,
            received_token: Mutex::new(None),
            stopped_token: Mutex::new(None),
            callbacks: Arc::new(Mutex::new(Callbacks {
                on_received: None,
                on_state: None,
            })),
            running: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn set_on_received(&self, cb: OnReceived) {
        self.callbacks.lock().on_received = Some(cb);
    }

    pub fn set_on_state(&self, cb: OnState) {
        self.callbacks.lock().on_state = Some(cb);
    }

    pub fn start(&self) -> Result<(), WatcherError> {
        if self.running.load(Ordering::SeqCst) {
            return Ok(());
        }

        let callbacks = self.callbacks.clone();
        let received_handler = TypedEventHandler::<
            BluetoothLEAdvertisementWatcher,
            BluetoothLEAdvertisementReceivedEventArgs,
        >::new(move |_sender, args| {
            if let Some(args) = args.as_ref() {
                if let Some(data) = extract_advertisement(args) {
                    if !data.manufacturer_data.is_empty() {
                        if let Some(cb) = callbacks.lock().on_received.as_ref() {
                            cb(data);
                        }
                    }
                }
            }
            Ok(())
        });

        let callbacks = self.callbacks.clone();
        let running = self.running.clone();
        let stopped_handler = TypedEventHandler::<
            BluetoothLEAdvertisementWatcher,
            BluetoothLEAdvertisementWatcherStoppedEventArgs,
        >::new(move |sender, args| {
            running.store(false, Ordering::SeqCst);
            let mut error: Option<String> = None;
            if let Some(args) = args.as_ref() {
                if let Ok(err) = args.Error() {
                    // 0 = Success
                    if err.0 != 0 {
                        error = Some(format!("BluetoothError({})", err.0));
                    }
                }
            }
            if error.is_none() {
                if let Some(sender) = sender.as_ref() {
                    if let Ok(status) = sender.Status() {
                        if status == BluetoothLEAdvertisementWatcherStatus::Aborted {
                            error = Some("Aborted".into());
                        }
                    }
                }
            }
            if let Some(cb) = callbacks.lock().on_state.as_ref() {
                cb(WatcherState::Stopped, error);
            }
            Ok(())
        });

        let rt = self
            .watcher
            .Received(&received_handler)
            .map_err(|e| WatcherError(e.to_string()))?;
        *self.received_token.lock() = Some(rt);

        let st = self
            .watcher
            .Stopped(&stopped_handler)
            .map_err(|e| WatcherError(e.to_string()))?;
        *self.stopped_token.lock() = Some(st);

        self.watcher
            .Start()
            .map_err(|e| WatcherError(e.to_string()))?;
        self.running.store(true, Ordering::SeqCst);

        if let Some(cb) = self.callbacks.lock().on_state.as_ref() {
            cb(WatcherState::Started, None);
        }
        Ok(())
    }

    pub fn stop(&self) -> Result<(), WatcherError> {
        if !self.running.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.watcher
            .Stop()
            .map_err(|e| WatcherError(e.to_string()))?;
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

impl Drop for BleWatcher {
    fn drop(&mut self) {
        let _ = self.stop();
        if let Some(token) = self.received_token.lock().take() {
            let _ = self.watcher.RemoveReceived(token);
        }
        if let Some(token) = self.stopped_token.lock().take() {
            let _ = self.watcher.RemoveStopped(token);
        }
    }
}

fn extract_advertisement(
    args: &BluetoothLEAdvertisementReceivedEventArgs,
) -> Option<ReceivedAdvertisement> {
    let address = args.BluetoothAddress().ok()?;
    let rssi = args.RawSignalStrengthInDBm().ok()?;
    // DateTime::UniversalTime is in 100-ns ticks since 1601-01-01.
    let timestamp = args.Timestamp().ok()?.UniversalTime;

    let advertisement = args.Advertisement().ok()?;
    let mfrs = advertisement.ManufacturerData().ok()?;

    let mut manufacturer_data = Vec::new();
    for i in 0..mfrs.Size().unwrap_or(0) {
        if let Ok(mfr) = mfrs.GetAt(i) {
            let company_id = mfr.CompanyId().unwrap_or(0);
            let data_buf = match mfr.Data().ok() {
                Some(d) => d,
                None => continue,
            };
            let len = data_buf.Length().unwrap_or(0) as usize;
            let mut bytes = vec![0u8; len];
            if len > 0 {
                use windows::Storage::Streams::DataReader;
                if let Ok(reader) = DataReader::FromBuffer(&data_buf) {
                    let _ = reader.ReadBytes(&mut bytes);
                }
            }
            manufacturer_data.push(ManufacturerData {
                company_id,
                data: bytes,
            });
        }
    }

    Some(ReceivedAdvertisement {
        address,
        rssi,
        timestamp,
        manufacturer_data,
    })
}
