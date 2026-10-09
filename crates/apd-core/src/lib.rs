//! # apd-core
//!
//! AirPodsDesktop core library — Rust rewrite of the original Qt/C++ core.
//!
//! ## Modules
//! - [`protocol`] — Apple Continuity Protocol (AirPods proximity pairing advertisements)
//! - [`state`] — Device state machine (merge left/right advertisements, RSSI filter, lost timer)
//! - [`ble`] — Windows BLE advertisement watcher
//! - [`media`] — Global system media transport control (ear-detection pause/play)
//! - [`settings`] — Persistent user settings
//! - [`ffi`] — C ABI surface consumed by the WinUI 3 C# frontend

pub mod ble;
pub mod ffi;
pub mod media;
pub mod protocol;
pub mod settings;
pub mod state;

pub use protocol::{AirPodsModel, Battery, Color, Side};
pub use state::{CaseState, DeviceState, Manager, PodState};

/// Library version string, useful for diagnostics.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
