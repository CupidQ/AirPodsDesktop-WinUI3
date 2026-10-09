//! Device state management.

pub mod battery;
pub mod manager;

pub use battery::BatteryState;
pub use manager::{
    CaseState, DeviceState, Manager, ManagerEvent, PodState, StateSnapshot, UpdateEvent,
};
