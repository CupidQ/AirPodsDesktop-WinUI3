//! Caller-owned UTF-8 JSON ABI; existing device/settings layouts are unchanged.
use super::{endpoints, CONTROL};
use serde_json::json;
use std::ffi::{c_char, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};

fn string(value: impl serde::Serialize) -> *mut c_char {
    CString::new(serde_json::to_string(&value).unwrap_or_default())
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

fn action(f: impl FnOnce() -> Result<(), String>) -> *mut c_char {
    let result =
        catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|_| Err("控制操作发生内部错误".into()));
    match result {
        Ok(()) => string(json!({"ok": true, "error": null})),
        Err(error) => string(json!({"ok": false, "error": error})),
    }
}

/// Enumerate installed MagicAAP interfaces. Free result with apd_string_free.
#[no_mangle]
pub extern "C" fn apd_control_devices() -> *mut c_char {
    match catch_unwind(endpoints) {
        Ok(Ok(devices)) => string(json!({"devices": devices, "error": null})),
        Ok(Err(error)) => string(json!({"devices": [], "error": error})),
        Err(_) => string(json!({"devices": [], "error": "检测控制设备失败"})),
    }
}

/// Call off the UI thread; only accepts a currently enumerated driver interface.
/// Free the returned JSON result with apd_string_free.
/// # Safety
/// path is null or a readable NUL-terminated UTF-8 string for this call.
#[no_mangle]
pub unsafe extern "C" fn apd_control_connect(path: *const c_char) -> *mut c_char {
    action(|| {
        if path.is_null() {
            return Err("请选择控制设备".into());
        }
        let path = CStr::from_ptr(path)
            .to_str()
            .map_err(|_| "设备路径不是 UTF-8")?;
        CONTROL.connect(path)
    })
}

/// Call off the UI thread. Joins the control worker and restores ducked volume.
#[no_mangle]
pub extern "C" fn apd_control_disconnect() -> *mut c_char {
    action(|| {
        CONTROL.disconnect();
        Ok(())
    })
}

/// Free returned JSON snapshot with apd_string_free. Values are device-confirmed.
#[no_mangle]
pub extern "C" fn apd_control_state() -> *mut c_char {
    string(CONTROL.snapshot())
}

/// Call off the UI thread. 0=mode (1..4), 1=conversation (0/1), 2=one-ear ANC (0/1).
/// Success requires a matching device notification, or an already confirmed value.
#[no_mangle]
pub extern "C" fn apd_control_set(setting: u32, value: u32) -> *mut c_char {
    action(|| CONTROL.set(setting, value))
}

/// Local opt-in: duck Windows output during device-reported conversations.
#[no_mangle]
pub extern "C" fn apd_control_set_duck(enabled: u8) -> *mut c_char {
    action(|| {
        if enabled > 1 {
            return Err("无效的音量联动设置".into());
        }
        CONTROL.set_duck_enabled(enabled == 1);
        Ok(())
    })
}
