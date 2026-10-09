//! Bluetooth Low Energy advertisement scanning.

#[cfg(windows)]
pub mod watcher;

#[cfg(windows)]
pub use watcher::{AdvertisementData, BleWatcher, WatcherError, WatcherState};

/// Manufacturer-data entry extracted from an advertisement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManufacturerData {
    pub company_id: u16,
    pub data: Vec<u8>,
}

/// Normalized advertisement received from the OS.
#[derive(Debug, Clone)]
pub struct ReceivedAdvertisement {
    pub address: u64,
    pub rssi: i16,
    /// Monotonic timestamp from the OS (100 ns units on Windows).
    pub timestamp: i64,
    pub manufacturer_data: Vec<ManufacturerData>,
}

impl ReceivedAdvertisement {
    /// Return Apple manufacturer payload if present.
    pub fn apple_payload(&self) -> Option<&[u8]> {
        self.manufacturer_data
            .iter()
            .find(|m| m.company_id == crate::protocol::VENDOR_ID)
            .map(|m| m.data.as_slice())
    }
}
