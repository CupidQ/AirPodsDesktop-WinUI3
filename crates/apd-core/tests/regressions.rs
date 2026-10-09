use apd_core::protocol::airpods::AirPodsPacket;
use apd_core::state::{Manager, ManagerEvent};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn packet() -> AirPodsPacket {
    let mut raw = [0u8; 27];
    raw[0] = 7;
    raw[1] = 25;
    raw[3] = 0x0e;
    raw[4] = 0x20;
    raw[5] = 0x2a;
    raw[6] = 0x88;
    raw[7] = 6;
    AirPodsPacket::parse(&raw).unwrap()
}

fn check_reconnect(use_tick: bool) {
    let manager = Manager::new();
    let events = Arc::new(Mutex::new(Vec::new()));
    let capture = events.clone();
    manager.add_listener(Arc::new(move |event| capture.lock().unwrap().push(event)));
    manager.on_advertisement(packet(), 0xaa, -40);
    assert!(manager.current_state().unwrap().is_both_in_ear());
    if use_tick {
        std::thread::sleep(Duration::from_millis(5100));
        manager.tick();
    } else {
        manager.mark_lost();
    }
    assert!(manager.current_state().is_none());
    events.lock().unwrap().clear();
    manager.on_advertisement(packet(), 0xaa, -40);
    let events = events.lock().unwrap();
    assert!(events.iter().any(|e| matches!(e, ManagerEvent::StateUpdated(_))));
    assert!(events.iter().any(|e| matches!(e, ManagerEvent::LidToggled { opened: true })), "reconnected open lid did not emit popup trigger; events: {events:?}");
    assert!(events.iter().any(|e| matches!(e, ManagerEvent::BothInEar { in_ear: true })), "reconnected inserted pods did not emit media trigger");
}

#[test]
fn reconnect_after_mark_lost() { check_reconnect(false); }

#[test]
fn reconnect_after_timeout() { check_reconnect(true); }

#[test]
fn binding_clears_previous_state_and_filters_addresses() {
    let manager = Manager::new();
    manager.on_advertisement(packet(), 0xaa, -40);
    assert_eq!(manager.current_address(), Some(0xaa));
    manager.set_bound_address(0xbb);
    assert!(manager.current_state().is_none());
    assert_eq!(manager.current_address(), None);
    assert!(manager.on_advertisement(packet(), 0xaa, -40).is_none());
    assert!(manager.on_advertisement(packet(), 0xbb, -40).is_some());
    assert_eq!(manager.current_address(), Some(0xbb));
    manager.set_bound_address(0);
    assert!(manager.current_state().is_none());
    assert!(manager.on_advertisement(packet(), 0xcc, -40).is_some());
}

#[test]
fn ffi_struct_layout_matches_managed_bindings() {
    use apd_core::ffi::*;
    use std::mem::{size_of, offset_of};
    assert_eq!(size_of::<ApdBattery>(), 8);
    assert_eq!(size_of::<ApdPod>(), 16);
    assert_eq!(size_of::<ApdCase>(), 12);
    assert_eq!(size_of::<ApdDeviceState>(), 56);
    assert_eq!(offset_of!(ApdDeviceState, left), 4);
    assert_eq!(offset_of!(ApdDeviceState, right), 20);
    assert_eq!(offset_of!(ApdDeviceState, case), 36);
    assert_eq!(size_of::<ApdSettings>(), 24);
    assert_eq!(offset_of!(ApdSettings, bound_device_address), 8);
}
