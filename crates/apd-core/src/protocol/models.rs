//! Device model IDs and display metadata for Apple / Beats headphones.

/// Which ear an advertisement is broadcast from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    pub fn opposite(&self) -> Side {
        match self {
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        }
    }

    /// FFI: 0 = Left, 1 = Right
    pub fn as_ffi(&self) -> u32 {
        match self {
            Side::Left => 0,
            Side::Right => 1,
        }
    }
}

/// Device housing colour nibble from the advertisement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    White = 0x0,
    Black = 0x1,
    Red = 0x2,
    Blue = 0x3,
    Pink = 0x4,
    Gray = 0x5,
    Silver = 0x6,
    Gold = 0x7,
    RoseGold = 0x8,
    SpaceGray = 0x9,
    DarkBlue = 0xA,
    LightBlue = 0xB,
    Yellow = 0xC,
}

impl Color {
    pub fn from_u8(v: u8) -> Option<Color> {
        Some(match v {
            0x0 => Color::White,
            0x1 => Color::Black,
            0x2 => Color::Red,
            0x3 => Color::Blue,
            0x4 => Color::Pink,
            0x5 => Color::Gray,
            0x6 => Color::Silver,
            0x7 => Color::Gold,
            0x8 => Color::RoseGold,
            0x9 => Color::SpaceGray,
            0xA => Color::DarkBlue,
            0xB => Color::LightBlue,
            0xC => Color::Yellow,
            _ => return None,
        })
    }
}

/// Known AirPods / Beats models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AirPodsModel {
    #[default]
    Unknown,
    AirPods1,
    AirPods2,
    AirPods3,
    AirPods4,
    AirPods4Anc,
    AirPods5,
    AirPodsPro,
    AirPodsPro2,
    AirPodsPro2UsbC,
    AirPodsPro3,
    AirPodsMax,
    AirPodsMaxUsbC,
    Powerbeats3,
    BeatsX,
    BeatsSolo3,
    BeatsFitPro,
}

impl AirPodsModel {
    /// Map the 16-bit model identifier from the proximity-pairing payload.
    pub fn from_model_id(id: u16) -> Self {
        match id {
            0x2002 => AirPodsModel::AirPods1,
            0x200F => AirPodsModel::AirPods2,
            0x2013 => AirPodsModel::AirPods3,
            0x2019 => AirPodsModel::AirPods4,
            0x201B => AirPodsModel::AirPods4Anc,
            0x2030 | 0x2036 => AirPodsModel::AirPods5,
            0x200E => AirPodsModel::AirPodsPro,
            0x2014 => AirPodsModel::AirPodsPro2,
            0x2024 => AirPodsModel::AirPodsPro2UsbC,
            0x2027 => AirPodsModel::AirPodsPro3,
            0x200A => AirPodsModel::AirPodsMax,
            0x201F => AirPodsModel::AirPodsMaxUsbC,
            0x2003 => AirPodsModel::Powerbeats3,
            0x2005 => AirPodsModel::BeatsX,
            0x2006 => AirPodsModel::BeatsSolo3,
            0x2012 => AirPodsModel::BeatsFitPro,
            _ => AirPodsModel::Unknown,
        }
    }

    /// Human-readable English name (matches the original Qt app).
    pub fn display_name(&self) -> &'static str {
        match self {
            AirPodsModel::AirPods1 => "AirPods 1",
            AirPodsModel::AirPods2 => "AirPods 2",
            AirPodsModel::AirPods3 => "AirPods 3",
            AirPodsModel::AirPods4 => "AirPods 4",
            AirPodsModel::AirPods4Anc => "AirPods 4 (ANC)",
            AirPodsModel::AirPods5 => "AirPods 5",
            AirPodsModel::AirPodsPro => "AirPods Pro",
            AirPodsModel::AirPodsPro2 => "AirPods Pro 2",
            AirPodsModel::AirPodsPro2UsbC => "AirPods Pro 2 (USB-C)",
            AirPodsModel::AirPodsPro3 => "AirPods Pro 3",
            AirPodsModel::AirPodsMax => "AirPods Max",
            AirPodsModel::AirPodsMaxUsbC => "AirPods Max (USB-C)",
            AirPodsModel::Powerbeats3 => "Powerbeats 3",
            AirPodsModel::BeatsX => "BeatsX",
            AirPodsModel::BeatsSolo3 => "BeatsSolo3",
            AirPodsModel::BeatsFitPro => "Beats Fit Pro",
            AirPodsModel::Unknown => "Unknown",
        }
    }

    /// Stable integer for FFI / persistence.
    pub fn as_ffi(&self) -> u32 {
        match self {
            AirPodsModel::Unknown => 0,
            AirPodsModel::AirPods1 => 1,
            AirPodsModel::AirPods2 => 2,
            AirPodsModel::AirPods3 => 3,
            AirPodsModel::AirPods4 => 4,
            AirPodsModel::AirPods4Anc => 5,
            AirPodsModel::AirPods5 => 6,
            AirPodsModel::AirPodsPro => 7,
            AirPodsModel::AirPodsPro2 => 8,
            AirPodsModel::AirPodsPro2UsbC => 9,
            AirPodsModel::AirPodsPro3 => 10,
            AirPodsModel::AirPodsMax => 11,
            AirPodsModel::AirPodsMaxUsbC => 12,
            AirPodsModel::Powerbeats3 => 13,
            AirPodsModel::BeatsX => 14,
            AirPodsModel::BeatsSolo3 => 15,
            AirPodsModel::BeatsFitPro => 16,
        }
    }

    pub fn from_ffi(v: u32) -> Self {
        match v {
            1 => AirPodsModel::AirPods1,
            2 => AirPodsModel::AirPods2,
            3 => AirPodsModel::AirPods3,
            4 => AirPodsModel::AirPods4,
            5 => AirPodsModel::AirPods4Anc,
            6 => AirPodsModel::AirPods5,
            7 => AirPodsModel::AirPodsPro,
            8 => AirPodsModel::AirPodsPro2,
            9 => AirPodsModel::AirPodsPro2UsbC,
            10 => AirPodsModel::AirPodsPro3,
            11 => AirPodsModel::AirPodsMax,
            12 => AirPodsModel::AirPodsMaxUsbC,
            13 => AirPodsModel::Powerbeats3,
            14 => AirPodsModel::BeatsX,
            15 => AirPodsModel::BeatsSolo3,
            16 => AirPodsModel::BeatsFitPro,
            _ => AirPodsModel::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_id_mapping() {
        assert_eq!(AirPodsModel::from_model_id(0x200E), AirPodsModel::AirPodsPro);
        assert_eq!(AirPodsModel::from_model_id(0x2014), AirPodsModel::AirPodsPro2);
        assert_eq!(AirPodsModel::from_model_id(0x2019), AirPodsModel::AirPods4);
        assert_eq!(AirPodsModel::from_model_id(0x2027), AirPodsModel::AirPodsPro3);
        assert_eq!(AirPodsModel::from_model_id(0xFFFF), AirPodsModel::Unknown);
    }

    #[test]
    fn model_ffi_roundtrip() {
        for m in [
            AirPodsModel::AirPods1,
            AirPodsModel::AirPodsPro2,
            AirPodsModel::AirPodsMaxUsbC,
            AirPodsModel::BeatsFitPro,
        ] {
            assert_eq!(AirPodsModel::from_ffi(m.as_ffi()), m);
        }
    }
}
