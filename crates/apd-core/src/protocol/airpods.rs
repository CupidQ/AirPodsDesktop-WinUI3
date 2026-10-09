//! AirPods proximity-pairing advertisement payload.
//!
//! Layout (27 bytes total, matching `AppleCP::AirPods` in the original C++):
//!
//! ```text
//! offset  size  field
//! 0       1     packet_type   (must be 0x07 ProximityPairing)
//! 1       1     remaining_length (must be 25)
//! 2       1     unknown
//! 3       2     model_id (LE)
//! 5       1     status bits
//! 6       2     battery nibbles + charging bits
//! 8       1     lid state
//! 9       1     color
//! 10      1     unknown
//! 11      16    hash / encrypted payload
//! ```
//!
//! About "flipped" left/right:
//! The protocol has `curr` / `anot` (current / another) fields rather than left/right.
//! `status.broadcast_from` tells which ear is currently advertising; when it is the
//! left ear, `curr` is left and `anot` is right, and vice versa.

use super::models::{AirPodsModel, Color, Side};
use super::Battery;

/// Apple company identifier in BLE manufacturer-specific data.
pub const VENDOR_ID: u16 = 76; // 0x004C

/// `PacketType::ProximityPairing`
pub const PACKET_TYPE_PROXIMITY_PAIRING: u8 = 0x07;

/// Expected `remainingLength` byte (sizeof(AirPods) - 2).
pub const EXPECTED_REMAINING_LEN: u8 = 25;

/// Full manufacturer-data payload size for AirPods proximity pairing.
pub const PACKET_LEN: usize = 27;

/// Known Apple Continuity packet type identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PacketType {
    AirPrint = 0x03,
    AirDrop = 0x05,
    HomeKit = 0x06,
    ProximityPairing = 0x07,
    HeySiri = 0x08,
    AirPlay = 0x09,
    MagicSwitch = 0x0B,
    Handoff = 0x0C,
    InstantHotspotTarget = 0x0D,
    InstantHotspotSource = 0x0E,
    NearbyAction = 0x0F,
    NearbyInfo = 0x10,
}

/// Parsed AirPods proximity-pairing advertisement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AirPodsPacket {
    pub model_id: u16,
    pub model: AirPodsModel,
    pub color: Option<Color>,
    /// Which ear is broadcasting this advertisement.
    pub broadcast_side: Side,
    pub left: PodReading,
    pub right: PodReading,
    pub case: CaseReading,
    pub raw: [u8; PACKET_LEN],
}

/// Per-ear reading extracted from the advertisement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PodReading {
    pub battery: Battery,
    pub is_charging: bool,
    pub is_in_ear: bool,
}

/// Charging-case reading extracted from the advertisement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CaseReading {
    pub battery: Battery,
    pub is_charging: bool,
    pub is_both_pods_in_case: bool,
    pub is_lid_opened: bool,
}

impl AirPodsPacket {
    /// Validate and parse a manufacturer-data payload.
    ///
    /// Returns `None` when the buffer is not a valid AirPods proximity-pairing packet.
    pub fn parse(data: &[u8]) -> Option<Self> {
        if !Self::is_valid(data) {
            return None;
        }

        let mut raw = [0u8; PACKET_LEN];
        raw.copy_from_slice(data);

        let model_id = u16::from_le_bytes([raw[3], raw[4]]);
        let model = AirPodsModel::from_model_id(model_id);
        let color = Color::from_u8(raw[9]);

        // Status byte (offset 5)
        // bit0 unk, bit1 currInEar, bit2 bothInCase, bit3 anotInEar,
        // bit4 unk, bit5 broadcastFrom (1 = left), bit6 unk, bit7 unk
        let status = raw[5];
        let curr_in_ear = (status & 0b0000_0010) != 0;
        let both_in_case = (status & 0b0000_0100) != 0;
        let anot_in_ear = (status & 0b0000_1000) != 0;
        let broadcast_from = (status & 0b0010_0000) != 0;

        let broadcast_side = if broadcast_from {
            Side::Left
        } else {
            Side::Right
        };

        // Battery byte 0 (offset 6): high nibble = anot, low nibble = curr
        // Battery byte 1 (offset 7): low nibble = case; bits 0–2 of high nibble = charging flags
        let batt0 = raw[6];
        let batt1 = raw[7];
        let curr_batt = batt0 & 0x0F;
        let anot_batt = (batt0 >> 4) & 0x0F;
        let case_batt = batt1 & 0x0F;

        let curr_charging = (batt1 & 0b0001_0000) != 0;
        let anot_charging = (batt1 & 0b0010_0000) != 0;
        let case_charging = (batt1 & 0b0100_0000) != 0;

        // Lid byte (offset 8): bit3 = closed (1 = closed / 0 = open)
        let lid = raw[8];
        let lid_closed = (lid & 0b0000_1000) != 0;

        // Map curr/anot → left/right depending on which ear is broadcasting.
        let (left_raw_batt, right_raw_batt) = match broadcast_side {
            Side::Left => (curr_batt, anot_batt),
            Side::Right => (anot_batt, curr_batt),
        };
        let (left_charging, right_charging) = match broadcast_side {
            Side::Left => (curr_charging, anot_charging),
            Side::Right => (anot_charging, curr_charging),
        };
        let (left_in_ear_flag, right_in_ear_flag) = match broadcast_side {
            Side::Left => (curr_in_ear, anot_in_ear),
            Side::Right => (anot_in_ear, curr_in_ear),
        };

        // The original filters in-ear when charging (spurious "in ear" while docked).
        let left = PodReading {
            battery: Battery::from_nibble(left_raw_batt),
            is_charging: left_charging,
            is_in_ear: !left_charging && left_in_ear_flag,
        };
        let right = PodReading {
            battery: Battery::from_nibble(right_raw_batt),
            is_charging: right_charging,
            is_in_ear: !right_charging && right_in_ear_flag,
        };

        let case = CaseReading {
            battery: Battery::from_nibble(case_batt),
            is_charging: case_charging,
            is_both_pods_in_case: both_in_case,
            is_lid_opened: !lid_closed,
        };

        Some(AirPodsPacket {
            model_id,
            model,
            color,
            broadcast_side,
            left,
            right,
            case,
            raw,
        })
    }

    /// Structural validity check (type + length).
    pub fn is_valid(data: &[u8]) -> bool {
        if data.len() != PACKET_LEN {
            return false;
        }
        data[0] == PACKET_TYPE_PROXIMITY_PAIRING && data[1] == EXPECTED_REMAINING_LEN
    }

    /// Return a copy with the trailing 16-byte hash/encrypted payload zeroed.
    ///
    /// That region may contain identifying material, so logs should use this form.
    pub fn desensitized(&self) -> [u8; PACKET_LEN] {
        let mut out = self.raw;
        for b in out[11..].iter_mut() {
            *b = 0;
        }
        out
    }

    /// Hex preview of the desensitized payload (for diagnostics).
    pub fn desensitized_hex(&self) -> String {
        self.desensitized()
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a valid packet for tests.
    fn make_packet() -> Vec<u8> {
        let mut p = vec![0u8; PACKET_LEN];
        p[0] = 0x07; // ProximityPairing
        p[1] = 25; // remaining length
        p[3] = 0x0E; // model id 0x200E = AirPods Pro
        p[4] = 0x20;
        // status: broadcast from left (bit5), left in ear (bit1)
        p[5] = 0b0010_0010;
        // battery: curr=8 (left 80%), anot=7 (right 70%)
        p[6] = 0x78;
        // battery: case=6 (60%), currCharging=0, anotCharging=0, caseCharging=0
        p[7] = 0x06;
        // lid: closed bit3 = 0 → lid open
        p[8] = 0x00;
        // color white
        p[9] = 0x00;
        p
    }

    #[test]
    fn parse_basic() {
        let p = AirPodsPacket::parse(&make_packet()).expect("valid packet");
        assert_eq!(p.model, AirPodsModel::AirPodsPro);
        assert_eq!(p.model_id, 0x200E);
        assert_eq!(p.broadcast_side, Side::Left);
        assert_eq!(p.left.battery.value(), Some(80));
        assert_eq!(p.right.battery.value(), Some(70));
        assert_eq!(p.case.battery.value(), Some(60));
        assert!(p.left.is_in_ear);
        assert!(!p.right.is_in_ear);
        assert!(p.case.is_lid_opened);
        assert!(!p.case.is_both_pods_in_case);
    }

    #[test]
    fn parse_right_broadcast_swaps_sides() {
        let mut raw = make_packet();
        // broadcast from right (bit5 = 0)
        raw[5] = 0b0000_0010; // anot in ear? wait — curr in ear, broadcast right
                              // When broadcast_side == Right: curr is right, anot is left
                              // curr in ear → right in ear
        let p = AirPodsPacket::parse(&raw).unwrap();
        assert_eq!(p.broadcast_side, Side::Right);
        // curr=8 → right 80%, anot=7 → left 70%
        assert_eq!(p.right.battery.value(), Some(80));
        assert_eq!(p.left.battery.value(), Some(70));
        assert!(p.right.is_in_ear);
        assert!(!p.left.is_in_ear);
    }

    #[test]
    fn charging_blocks_in_ear() {
        let mut raw = make_packet();
        // curr charging + curr in ear
        raw[5] = 0b0010_0010;
        raw[7] = 0x06 | 0x10; // currCharging
        let p = AirPodsPacket::parse(&raw).unwrap();
        assert!(p.left.is_charging);
        assert!(!p.left.is_in_ear);
    }

    #[test]
    fn lid_closed() {
        let mut raw = make_packet();
        raw[8] = 0x08; // closed
        let p = AirPodsPacket::parse(&raw).unwrap();
        assert!(!p.case.is_lid_opened);
    }

    #[test]
    fn reject_wrong_length() {
        let mut raw = make_packet();
        raw.push(0);
        assert!(AirPodsPacket::parse(&raw).is_none());
    }

    #[test]
    fn reject_wrong_type() {
        let mut raw = make_packet();
        raw[0] = 0x10;
        assert!(AirPodsPacket::parse(&raw).is_none());
    }

    #[test]
    fn desensitize_zeroes_tail() {
        let mut raw = make_packet();
        raw[11] = 0xAB;
        raw[26] = 0xCD;
        let p = AirPodsPacket::parse(&raw).unwrap();
        let d = p.desensitized();
        assert_eq!(&d[11..], &[0u8; 16][..]);
        assert_eq!(d[0], 0x07);
    }
}
