//! AirPods state manager.
//!
//! AirPods use random non-resolvable BLE addresses, so devices cannot be tracked
//! by address alone. The manager keeps the most recent advertisement from each
//! ear side, merges them into a single [`DeviceState`], and emits updates.
//!
//! Ported from `Source/Core/AirPodsStateManager.cpp` in the original project.

use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use crate::protocol::airpods::{AirPodsPacket, PodReading};
use crate::protocol::{AirPodsModel, Side};
use crate::state::battery::BatteryState;

/// Per-ear state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PodState {
    pub battery: BatteryState,
    pub is_in_ear: bool,
}

/// Charging-case state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CaseState {
    pub battery: BatteryState,
    pub is_both_pods_in_case: bool,
    pub is_lid_opened: bool,
}

/// Fully merged device snapshot published to the UI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeviceState {
    pub model: AirPodsModel,
    pub display_name: String,
    pub left: PodState,
    pub right: PodState,
    pub case: CaseState,
    pub rssi: i16,
}

impl DeviceState {
    /// True when both ears report in-ear.
    pub fn is_both_in_ear(&self) -> bool {
        self.left.is_in_ear && self.right.is_in_ear
    }

    /// True when at least one ear reports in-ear.
    pub fn is_any_in_ear(&self) -> bool {
        self.left.is_in_ear || self.right.is_in_ear
    }

    /// True when either battery is low and not charging.
    pub fn has_low_battery(&self) -> bool {
        (self.left.battery.is_low() && !self.left.battery.is_charging)
            || (self.right.battery.is_low() && !self.right.battery.is_charging)
    }
}

/// Snapshot type used on the FFI boundary.
pub type StateSnapshot = DeviceState;

/// Emitted when the merged state changes.
#[derive(Debug, Clone)]
pub struct UpdateEvent {
    pub old: Option<DeviceState>,
    pub new: DeviceState,
}

/// Higher-level manager notifications (lid toggle, lost, etc.).
#[derive(Debug, Clone)]
pub enum ManagerEvent {
    StateUpdated(UpdateEvent),
    LidToggled { opened: bool },
    BothInEar { in_ear: bool },
    Lost,
    Disconnected,
}

/// Listener callback type.
pub type Listener = Arc<dyn Fn(ManagerEvent) + Send + Sync>;

/// Time-to-live for a single-side advertisement before it is considered stale.
const ADV_TTL: Duration = Duration::from_secs(2);
/// Time without any advertisement before the whole device is considered lost.
const LOST_TIMEOUT: Duration = Duration::from_secs(5);

/// Internal per-side advertisement slot.
#[derive(Debug, Clone)]
struct AdvSlot {
    packet: AirPodsPacket,
    address: u64,
    rssi: i16,
    at: Instant,
}

struct Inner {
    left: Option<AdvSlot>,
    right: Option<AdvSlot>,
    cached: Option<DeviceState>,
    rssi_min: i16,
    lost: bool,
    prev_both_in_ear: bool,
    prev_lid_opened: bool,
    bound_address: Option<u64>,
    listeners: Vec<Listener>,
}

/// Thread-safe state manager.
#[derive(Clone)]
pub struct Manager {
    inner: Arc<Mutex<Inner>>,
}

impl Default for Manager {
    fn default() -> Self {
        Self::new()
    }
}

impl Manager {
    pub fn new() -> Self {
        Manager {
            inner: Arc::new(Mutex::new(Inner {
                left: None,
                right: None,
                cached: None,
                rssi_min: i16::MIN,
                lost: true,
                prev_both_in_ear: false,
                prev_lid_opened: false,
                bound_address: None,
                listeners: Vec::new(),
            })),
        }
    }

    /// Register a state-change listener. Returns a remove-handle index.
    pub fn add_listener(&self, listener: Listener) -> usize {
        let mut g = self.inner.lock();
        g.listeners.push(listener);
        g.listeners.len() - 1
    }

    /// Minimum RSSI required for an advertisement to be accepted.
    pub fn set_rssi_min(&self, rssi_min: i16) {
        self.inner.lock().rssi_min = rssi_min;
    }

    /// Only accept advertisements from a specific Bluetooth address (0 = any).
    pub fn set_bound_address(&self, address: u64) {
        let changed = {
            let mut g = self.inner.lock();
            let next = if address == 0 { None } else { Some(address) };
            let changed = g.bound_address != next;
            g.bound_address = next;
            changed
        };
        if changed { self.mark_lost(); }
    }

    /// Feed a parsed advertisement. Returns `true` if the merged state changed.
    pub fn on_advertisement(
        &self,
        packet: AirPodsPacket,
        address: u64,
        rssi: i16,
    ) -> Option<UpdateEvent> {
        let mut events: Vec<ManagerEvent> = Vec::new();
        let update = {
            let mut g = self.inner.lock();

            // RSSI floor.
            if rssi < g.rssi_min {
                return None;
            }

            // Optional address binding.
            if let Some(bound) = g.bound_address {
                if address != bound {
                    return None;
                }
            }

            let slot = AdvSlot {
                packet,
                address,
                rssi,
                at: Instant::now(),
            };

            match slot.packet.broadcast_side {
                Side::Left => g.left = Some(slot),
                Side::Right => g.right = Some(slot),
            }

            // Drop stale opposite-side data.
            let now = Instant::now();
            if let Some(l) = g.left.as_ref() {
                if now.duration_since(l.at) > ADV_TTL {
                    g.left = None;
                }
            }
            if let Some(r) = g.right.as_ref() {
                if now.duration_since(r.at) > ADV_TTL {
                    g.right = None;
                }
            }

            match Self::merge_locked(&g) {
                Some(new) => {
                    let old = g.cached.clone();
                    let changed = old.as_ref() != Some(&new);

                    // Edge triggers.
                    if changed {
                        let both = new.is_both_in_ear();
                        if both != g.prev_both_in_ear {
                            g.prev_both_in_ear = both;
                            events.push(ManagerEvent::BothInEar { in_ear: both });
                        }
                        let lid = new.case.is_lid_opened;
                        if lid != g.prev_lid_opened {
                            g.prev_lid_opened = lid;
                            events.push(ManagerEvent::LidToggled { opened: lid });
                        }
                    }

                    if g.lost {
                        g.lost = false;
                    }

                    if changed {
                        g.cached = Some(new.clone());
                        Some(UpdateEvent { old, new })
                    } else {
                        None
                    }
                }
                None => None,
            }
        };

        if let Some(ref up) = update {
            events.push(ManagerEvent::StateUpdated(up.clone()));
        }

        if !events.is_empty() {
            let listeners = self.inner.lock().listeners.clone();
            for l in listeners {
                for e in &events {
                    l(e.clone());
                }
            }
        }

        update
    }

    /// Mark the device as lost / disconnected and notify listeners.
    pub fn mark_lost(&self) {
        let mut events = Vec::new();
        {
            let mut g = self.inner.lock();
            if !g.lost {
                g.lost = true;
                g.left = None;
                g.right = None;
                g.cached = None;
                g.prev_both_in_ear = false;
                g.prev_lid_opened = false;
                events.push(ManagerEvent::Lost);
                events.push(ManagerEvent::Disconnected);
            }
        }
        if !events.is_empty() {
            let listeners = self.inner.lock().listeners.clone();
            for l in listeners {
                for e in &events {
                    l(e.clone());
                }
            }
        }
    }

    /// Current merged state, if any.
    pub fn current_state(&self) -> Option<DeviceState> {
        self.inner.lock().cached.clone()
    }

    /// Latest BLE address; AirPods can rotate this address at any time.
    pub fn current_address(&self) -> Option<u64> {
        let g = self.inner.lock();
        [g.left.as_ref(), g.right.as_ref()]
            .into_iter()
            .flatten()
            .max_by_key(|slot| slot.at)
            .map(|slot| slot.address)
    }

    /// Run a lost-check using the wall clock; call periodically from the UI timer.
    pub fn tick(&self) {
        let mut should_lost = false;
        {
            let mut g = self.inner.lock();
            if g.lost {
                return;
            }
            let now = Instant::now();
            let last = [g.left.as_ref(), g.right.as_ref()]
                .into_iter()
                .flatten()
                .map(|s| s.at)
                .max();
            match last {
                Some(t) if now.duration_since(t) <= LOST_TIMEOUT => {}
                _ => {
                    g.lost = true;
                    g.left = None;
                    g.right = None;
                    g.cached = None;
                    g.prev_both_in_ear = false;
                    g.prev_lid_opened = false;
                    should_lost = true;
                }
            }
        }
        if should_lost {
            let listeners = self.inner.lock().listeners.clone();
            for l in listeners {
                l(ManagerEvent::Lost);
                l(ManagerEvent::Disconnected);
            }
        }
    }

    /// Merge left/right advertisement slots into a single snapshot.
    ///
    /// Prefers the freshest reading for each field; falls back to the other side
    /// when one side has not advertised recently.
    fn merge_locked(g: &Inner) -> Option<DeviceState> {
        let left_slot = g.left.as_ref();
        let right_slot = g.right.as_ref();

        let (packet, rssi) = match (left_slot, right_slot) {
            (Some(l), Some(r)) => {
                if l.at >= r.at {
                    (l.packet.clone(), r.rssi.max(l.rssi))
                } else {
                    (r.packet.clone(), r.rssi.max(l.rssi))
                }
            }
            (Some(l), None) => (l.packet.clone(), l.rssi),
            (None, Some(r)) => (r.packet.clone(), r.rssi),
            (None, None) => return None,
        };

        // Merge per-ear: prefer the side that is actually broadcasting that ear's data.
        let left_pod = pick_pod(&packet, Side::Left, left_slot, right_slot);
        let right_pod = pick_pod(&packet, Side::Right, left_slot, right_slot);

        Some(DeviceState {
            model: packet.model,
            display_name: packet.model.display_name().to_string(),
            left: left_pod,
            right: right_pod,
            case: CaseState {
                battery: BatteryState::new(packet.case.battery, packet.case.is_charging),
                is_both_pods_in_case: packet.case.is_both_pods_in_case,
                is_lid_opened: packet.case.is_lid_opened,
            },
            rssi,
        })
    }
}

/// Choose the best reading for `side`.
fn pick_pod(
    packet: &AirPodsPacket,
    side: Side,
    left: Option<&AdvSlot>,
    right: Option<&AdvSlot>,
) -> PodState {
    // The advertising packet always contains both ears; prefer the side whose
    // own advertisement is fresher for that ear (more accurate charging/in-ear).
    let from_packet = match side {
        Side::Left => packet.left,
        Side::Right => packet.right,
    };

    // If the opposite-side slot is newer and reports that ear, use it.
    let alt: Option<PodReading> = match side {
        Side::Left => right.map(|s| s.packet.left),
        Side::Right => left.map(|s| s.packet.right),
    };

    let primary_slot_at = match side {
        Side::Left => left.map(|s| s.at),
        Side::Right => right.map(|s| s.at),
    };
    let alt_slot_at = match side {
        Side::Left => right.map(|s| s.at),
        Side::Right => left.map(|s| s.at),
    };

    let reading = match (primary_slot_at, alt_slot_at, alt) {
        (Some(p_at), Some(a_at), Some(a)) if a_at > p_at => a,
        _ => from_packet,
    };

    PodState {
        battery: BatteryState::new(reading.battery, reading.is_charging),
        is_in_ear: reading.is_in_ear,
    }
}

/// Convenience: default display name for a model.
pub fn default_display_name(model: AirPodsModel) -> String {
    model.display_name().to_string()
}

// Silence unused warning for Battery re-export used by tests / FFI.
#[allow(unused_imports)]
use crate::protocol::Battery as _BatteryForDocs;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::airpods::AirPodsPacket;

    fn make_packet(side_left: bool, left_batt: u8, right_batt: u8, left_in_ear: bool) -> AirPodsPacket {
        let mut p = vec![0u8; 27];
        p[0] = 0x07;
        p[1] = 25;
        p[3] = 0x0E;
        p[4] = 0x20;
        let mut status = 0u8;
        if side_left {
            status |= 0b0010_0000;
        }
        // curr in ear
        if left_in_ear == side_left {
            status |= 0b0000_0010;
        }
        p[5] = status;
        // curr/anot nibbles
        let (curr, anot) = if side_left {
            (left_batt, right_batt)
        } else {
            (right_batt, left_batt)
        };
        p[6] = ((anot & 0x0F) << 4) | (curr & 0x0F);
        p[7] = 0x06;
        p[8] = 0x00;
        p[9] = 0x00;
        AirPodsPacket::parse(&p).unwrap()
    }

    #[test]
    fn merge_both_sides() {
        let mgr = Manager::new();

        let _ = mgr.on_advertisement(
            make_packet(true, 8, 7, true),
            0xAA,
            -50,
        );
        let last = mgr.on_advertisement(
            make_packet(false, 8, 7, true),
            0xBB,
            -55,
        );

        let st = last.expect("state update").new;
        assert_eq!(st.left.battery.percent, Some(80));
        assert_eq!(st.right.battery.percent, Some(70));
        assert_eq!(st.model, AirPodsModel::AirPodsPro);
    }

    #[test]
    fn rssi_filter() {
        let mgr = Manager::new();
        mgr.set_rssi_min(-60);
        assert!(mgr
            .on_advertisement(make_packet(true, 8, 8, true), 0xAA, -70)
            .is_none());
        assert!(mgr
            .on_advertisement(make_packet(true, 8, 8, true), 0xAA, -40)
            .is_some());
    }

    #[test]
    fn bound_address_filter() {
        let mgr = Manager::new();
        mgr.set_bound_address(0x1234);
        assert!(mgr
            .on_advertisement(make_packet(true, 8, 8, true), 0x9999, -40)
            .is_none());
        assert!(mgr
            .on_advertisement(make_packet(true, 8, 8, true), 0x1234, -40)
            .is_some());
    }

    #[test]
    fn lost_resets_state() {
        let mgr = Manager::new();
        let _ = mgr.on_advertisement(make_packet(true, 8, 8, true), 0xAA, -40);
        assert!(mgr.current_state().is_some());
        mgr.mark_lost();
        assert!(mgr.current_state().is_none());
    }
}
