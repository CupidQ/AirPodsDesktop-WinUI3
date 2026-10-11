//! AAP control messages, based on LibrePods and MagicPodsCore protocol research.
//! See docs/airpods-control.md for pinned sources and attribution.

use serde::Serialize;

pub const HANDSHAKE: [u8; 16] = [0, 0, 4, 0, 1, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0];
pub const HANDSHAKE_ACK: [u8; 4] = [1, 0, 4, 0];
pub const NOTIFICATIONS: [u8; 10] = [4, 0, 4, 0, 15, 0, 255, 255, 255, 255];
pub const FEATURES: [u8; 14] = [4, 0, 4, 0, 77, 0, 14, 0, 0, 0, 0, 0, 0, 0];
const PREFIX: [u8; 4] = [4, 0, 4, 0];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    ListeningMode = 0x0d,
    ConversationAwareness = 0x28,
    OneEarAnc = 0x1b,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Command {
    pub field: Field,
    pub value: u8,
}

impl Command {
    /// UI modes: 1 Off, 2 ANC, 3 Transparency, 4 Adaptive. Boolean values: 0/1.
    pub fn from_ui(setting: u32, value: u32) -> Result<Self, String> {
        let (field, value) = match (setting, value) {
            (0, 1..=4) => (Field::ListeningMode, value as u8),
            (1, 0..=1) => (Field::ConversationAwareness, if value == 1 { 1 } else { 2 }),
            (2, 0..=1) => (Field::OneEarAnc, if value == 1 { 1 } else { 2 }),
            _ => return Err("无效的耳机设置或取值".into()),
        };
        Ok(Self { field, value })
    }

    pub fn packet(self) -> [u8; 11] {
        [4, 0, 4, 0, 9, 0, self.field as u8, self.value, 0, 0, 0]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Values {
    pub listening_mode: Option<u8>,
    pub conversation_awareness: Option<bool>,
    pub one_ear_anc: Option<bool>,
    pub speaking: Option<bool>,
}

impl Values {
    pub fn matches(self, command: Command) -> bool {
        match command.field {
            Field::ListeningMode => self.listening_mode == Some(command.value),
            Field::ConversationAwareness => self.conversation_awareness == Some(command.value == 1),
            Field::OneEarAnc => self.one_ear_anc == Some(command.value == 1),
        }
    }

    pub fn has(self, field: Field) -> bool {
        match field {
            Field::ListeningMode => self.listening_mode.is_some(),
            Field::ConversationAwareness => self.conversation_awareness.is_some(),
            Field::OneEarAnc => self.one_ear_anc.is_some(),
        }
    }

    pub fn apply(&mut self, notification: Notification) {
        match notification {
            Notification::Control(command) => match command.field {
                Field::ListeningMode => self.listening_mode = Some(command.value),
                Field::ConversationAwareness => {
                    self.conversation_awareness = Some(command.value == 1);
                    if command.value == 2 {
                        self.speaking = Some(false);
                    }
                }
                Field::OneEarAnc => self.one_ear_anc = Some(command.value == 1),
            },
            Notification::Speaking(speaking) => self.speaking = Some(speaking),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notification {
    Control(Command),
    Speaking(bool),
}

/// Handles split/coalesced control notifications without retaining arbitrary traffic.
#[derive(Default)]
pub struct Decoder {
    pending: Vec<u8>,
}

impl Decoder {
    pub fn feed(&mut self, data: &[u8]) -> Vec<Notification> {
        let mut output = Vec::new();
        // Driver reads are bounded; malformed/unknown traffic cannot accumulate.
        if self.pending.len() + data.len() > 32 * 1024 {
            self.pending.clear();
        }
        if data.len() > 32 * 1024 {
            return output;
        }
        self.pending.extend_from_slice(data);
        loop {
            let Some(start) = self.pending.windows(4).position(|w| w == PREFIX) else {
                let keep = self.pending.len().min(3);
                self.pending.drain(..self.pending.len() - keep);
                break;
            };
            self.pending.drain(..start);
            if self.pending.len() < 6 {
                break;
            }
            // Some protocol implementations add the data prefix twice.
            if self.pending.get(4..8) == Some(PREFIX.as_slice()) {
                self.pending.drain(..4);
                continue;
            }
            let length = match (self.pending[4], self.pending[5]) {
                (9, 0) => 11,
                (0x4b, 0) => 10,
                _ => {
                    // A retained suffix of an unrelated packet can overlap a
                    // real prefix. Advance one byte so that prefix is not lost.
                    self.pending.drain(..1);
                    continue;
                }
            };
            if self.pending.len() < length {
                break;
            }
            let p = &self.pending[..length];
            if p[4] == 9 && p[8..11] == [0, 0, 0] {
                let field = match (p[6], p[7]) {
                    (0x0d, 1..=4) => Some(Field::ListeningMode),
                    (0x28, 1..=2) => Some(Field::ConversationAwareness),
                    (0x1b, 1..=2) => Some(Field::OneEarAnc),
                    _ => None,
                };
                if let Some(field) = field {
                    output.push(Notification::Control(Command { field, value: p[7] }));
                }
            } else if p[4] == 0x4b && p[6..9] == [2, 0, 1] {
                match p[9] {
                    1 => output.push(Notification::Speaking(true)),
                    0 | 4 | 6 | 8 | 9 => output.push(Notification::Speaking(false)),
                    _ => {} // Unknown intermediate states must not restore volume.
                }
            }
            self.pending.drain(..length);
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_bytes_match_protocol_references() {
        assert_eq!(
            Command::from_ui(0, 2).unwrap().packet(),
            [4, 0, 4, 0, 9, 0, 13, 2, 0, 0, 0]
        );
        assert_eq!(
            Command::from_ui(1, 0).unwrap().packet(),
            [4, 0, 4, 0, 9, 0, 40, 2, 0, 0, 0]
        );
        assert_eq!(
            Command::from_ui(2, 1).unwrap().packet(),
            [4, 0, 4, 0, 9, 0, 27, 1, 0, 0, 0]
        );
        for (s, v) in [(0, 0), (0, 5), (1, 2), (2, 2), (3, 1), (u32::MAX, u32::MAX)] {
            assert!(Command::from_ui(s, v).is_err());
        }
    }

    #[test]
    fn every_split_and_coalesced_notification_is_decoded() {
        let command = Command::from_ui(0, 3).unwrap();
        let packet = command.packet();
        for split in 0..packet.len() {
            let mut decoder = Decoder::default();
            assert!(decoder.feed(&packet[..split]).is_empty());
            assert_eq!(
                decoder.feed(&packet[split..]),
                [Notification::Control(command)]
            );
        }
        let mut data = vec![0, 1, 2];
        data.extend_from_slice(&packet);
        data.extend_from_slice(&Command::from_ui(2, 1).unwrap().packet());
        assert_eq!(Decoder::default().feed(&data).len(), 2);
        let mut nested = PREFIX.to_vec();
        nested.extend_from_slice(&packet);
        assert_eq!(
            Decoder::default().feed(&nested),
            [Notification::Control(command)]
        );
    }

    #[test]
    fn invalid_notifications_do_not_invent_confirmed_state() {
        let mut decoder = Decoder::default();
        let mut packet = Command::from_ui(0, 2).unwrap().packet();
        packet[7] = 99;
        assert!(decoder.feed(&packet).is_empty());
        packet[7] = 2;
        packet[10] = 1;
        assert!(decoder.feed(&packet).is_empty());
        assert!(decoder.feed(&vec![0; 40_000]).is_empty());
        assert!(decoder.pending.len() <= 3);
        assert_eq!(Values::default().listening_mode, None);
    }

    #[test]
    fn speaking_and_setting_notifications_update_confirmed_values() {
        let mut values = Values::default();
        let mut decoder = Decoder::default();
        for state in [1, 2, 3, 4, 6, 8, 9] {
            for n in decoder.feed(&[4, 0, 4, 0, 0x4b, 0, 2, 0, 1, state]) {
                values.apply(n);
            }
            assert_eq!(values.speaking, Some(state < 4));
        }
        let enabled = Command::from_ui(1, 1).unwrap();
        values.apply(Notification::Control(enabled));
        assert!(values.matches(enabled));
        values.apply(Notification::Control(Command::from_ui(1, 0).unwrap()));
        assert_eq!(values.speaking, Some(false));
    }
}
