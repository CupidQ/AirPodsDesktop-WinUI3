//! Battery value helpers used by the UI / tray.

use crate::protocol::Battery;

/// Presentation-friendly battery state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BatteryState {
    pub percent: Option<u8>,
    pub is_charging: bool,
}

impl BatteryState {
    pub fn new(battery: Battery, is_charging: bool) -> Self {
        BatteryState {
            percent: battery.value(),
            is_charging,
        }
    }

    pub fn is_available(&self) -> bool {
        self.percent.is_some()
    }

    pub fn is_low(&self) -> bool {
        matches!(self.percent, Some(p) if p <= 20)
    }

    /// String suitable for a tray tooltip, e.g. `"80% ⚡"` / `"--"`.
    pub fn display_string(&self) -> String {
        match self.percent {
            Some(p) if self.is_charging => format!("{p}% ⚡"),
            Some(p) => format!("{p}%"),
            None => "--".into(),
        }
    }
}
