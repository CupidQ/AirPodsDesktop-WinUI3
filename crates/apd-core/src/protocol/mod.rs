//! Apple Continuity Protocol (ACP) parsers.
//!
//! Reference: Apple's BLE proximity-pairing advertisements (manuf. company id 0x004C),
//! reverse-engineered by OpenPods / MagicPods / AirPodsDesktop.

pub mod airpods;
pub mod models;

pub use airpods::{AirPodsPacket, PacketType, PACKET_LEN, VENDOR_ID};
pub use models::{AirPodsModel, Color, Side};

/// Battery percentage stored as an optional 0–100 value.
///
/// The wire format carries a 0–10 nibble; `None` means "unavailable".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Battery(pub Option<u8>);

impl Battery {
    pub const UNAVAILABLE: Battery = Battery(None);

    pub fn from_nibble(nibble: u8) -> Self {
        if nibble <= 10 {
            // Wire nibble is 0–10 representing 0 %–100 % in steps of 10.
            Battery(Some(nibble * 10))
        } else {
            Battery(None)
        }
    }

    pub fn is_available(&self) -> bool {
        self.0.is_some()
    }

    pub fn value(&self) -> Option<u8> {
        self.0
    }

    /// Low battery threshold: ≤ 20 %.
    pub fn is_low(&self) -> bool {
        matches!(self.0, Some(v) if v <= 20)
    }
}

impl From<u8> for Battery {
    fn from(v: u8) -> Self {
        if v <= 100 {
            Battery(Some(v))
        } else {
            Battery(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn battery_from_nibble() {
        assert_eq!(Battery::from_nibble(0).value(), Some(0));
        assert_eq!(Battery::from_nibble(5).value(), Some(50));
        assert_eq!(Battery::from_nibble(10).value(), Some(100));
        assert_eq!(Battery::from_nibble(11).value(), None);
        assert_eq!(Battery::from_nibble(15).value(), None);
    }

    #[test]
    fn battery_low() {
        assert!(Battery(Some(20)).is_low());
        assert!(Battery(Some(10)).is_low());
        assert!(!Battery(Some(30)).is_low());
        assert!(!Battery(None).is_low());
    }
}
