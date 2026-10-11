//! Original Rust Windows transport. Interface GUIDs and packet exchanges were
//! cross-checked against FIocker/aap-head-tracker; see attribution documentation.
use super::{product_id, volume::DuckedVolume, Endpoint, Transport};
use std::time::Duration;
use windows::core::{GUID, PCWSTR};
use windows::Win32::Devices::DeviceAndDriverInstallation::*;
use windows::Win32::Devices::Properties::*;
use windows::Win32::Foundation::{
    CloseHandle, ERROR_IO_PENDING, ERROR_OPERATION_ABORTED, GENERIC_READ, GENERIC_WRITE, HANDLE,
    WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{eMultimedia, eRender, IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, ReadFile, WriteFile, FILE_FLAG_OVERLAPPED, FILE_SHARE_MODE, OPEN_EXISTING,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
};
use windows::Win32::System::Threading::{CreateEventW, ResetEvent, WaitForSingleObject};
use windows::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};

const PRIVATE: GUID = GUID::from_u128(0x9eec98bb_3c54_45d4_a843_7900c4635e08);
const SERVICE: GUID = GUID::from_u128(0x74ec2172_0bad_4d01_8f77_997b2be0722a);

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn interface_paths(guid: &GUID) -> Result<Vec<String>, String> {
    // Interface arrivals/removals can change the MULTI_SZ size between calls.
    for _ in 0..3 {
        let mut count = 0;
        let result = unsafe {
            CM_Get_Device_Interface_List_SizeW(
                &mut count,
                guid,
                PCWSTR::null(),
                CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
            )
        };
        if result != CR_SUCCESS {
            return Err(format!("检测 MagicAAP 接口失败 ({})", result.0));
        }
        if count <= 1 {
            return Ok(Vec::new());
        }
        if count > 1_048_576 {
            return Err("驱动返回的设备列表过长".into());
        }
        let mut buffer = vec![0u16; count as usize];
        let result = unsafe {
            CM_Get_Device_Interface_ListW(
                guid,
                PCWSTR::null(),
                &mut buffer,
                CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
            )
        };
        if result == CR_BUFFER_SMALL {
            continue;
        }
        if result != CR_SUCCESS {
            return Err(format!("读取 MagicAAP 接口失败 ({})", result.0));
        }
        return Ok(buffer
            .split(|c| *c == 0)
            .take_while(|s| !s.is_empty())
            .map(String::from_utf16_lossy)
            .collect());
    }
    Err("控制设备列表正在变化，请重试".into())
}

fn interface_instance(path: &str) -> Option<String> {
    let path = wide(path);
    let mut kind = DEVPROPTYPE::default();
    let mut buffer = vec![0u16; 2048];
    let mut size = (buffer.len() * 2) as u32;
    let result = unsafe {
        CM_Get_Device_Interface_PropertyW(
            PCWSTR(path.as_ptr()),
            &DEVPKEY_Device_InstanceId,
            &mut kind,
            Some(buffer.as_mut_ptr().cast()),
            &mut size,
            0,
        )
    };
    property_string(result, kind, &buffer)
}

fn node_string(node: u32, key: &DEVPROPKEY) -> Option<String> {
    let mut kind = DEVPROPTYPE::default();
    let mut buffer = vec![0u16; 2048];
    let mut size = (buffer.len() * 2) as u32;
    let result = unsafe {
        CM_Get_DevNode_PropertyW(
            node,
            key,
            &mut kind,
            Some(buffer.as_mut_ptr().cast()),
            &mut size,
            0,
        )
    };
    property_string(result, kind, &buffer)
}

fn property_string(result: CONFIGRET, kind: DEVPROPTYPE, buffer: &[u16]) -> Option<String> {
    if result != CR_SUCCESS || kind != DEVPROP_TYPE_STRING {
        return None;
    }
    let end = buffer.iter().position(|c| *c == 0)?;
    (end != 0).then(|| String::from_utf16_lossy(&buffer[..end]))
}

fn endpoint(path: String, index: usize) -> Endpoint {
    let mut name = None;
    let instance = interface_instance(&path);
    let mut pid = product_id(&path).or_else(|| instance.as_deref().and_then(product_id));
    if let Some(instance) = instance {
        let instance = wide(&instance);
        let mut node = 0;
        if unsafe {
            CM_Locate_DevNodeW(
                &mut node,
                PCWSTR(instance.as_ptr()),
                CM_LOCATE_DEVNODE_NORMAL,
            )
        } == CR_SUCCESS
        {
            for _ in 0..4 {
                if pid.is_none() {
                    pid = node_string(node, &DEVPKEY_Device_InstanceId)
                        .as_deref()
                        .and_then(product_id);
                }
                if let Some(candidate) = node_string(node, &DEVPKEY_Device_FriendlyName)
                    .or_else(|| node_string(node, &DEVPKEY_NAME))
                {
                    let lower = candidate.to_ascii_lowercase();
                    if !lower.contains("magicaap")
                        && !lower.contains("aap server")
                        && !lower.contains("aap client")
                        && !lower.contains("bluetooth")
                    {
                        name = Some(candidate);
                        break;
                    }
                }
                let mut parent = 0;
                if unsafe { CM_Get_Parent(&mut parent, node, 0) } != CR_SUCCESS {
                    break;
                }
                node = parent;
            }
        }
    }
    let name = name.unwrap_or_else(|| {
        pid.map(crate::AirPodsModel::from_model_id)
            .map(|m| m.display_name().to_owned())
            .unwrap_or_else(|| "AirPods".into())
    });
    Endpoint {
        path,
        name: format!("{name} · 控制设备 {}", index + 1),
        product_id: pid,
    }
}

pub fn endpoints() -> Result<Vec<Endpoint>, String> {
    // Prefer private interfaces, avoiding aliases of the same device/service.
    let mut paths = interface_paths(&PRIVATE)?;
    if paths.is_empty() {
        paths = interface_paths(&SERVICE)?;
    }
    paths.sort();
    paths.dedup();
    Ok(paths
        .into_iter()
        .enumerate()
        .map(|(i, p)| endpoint(p, i))
        .collect())
}

struct OwnedHandle(HANDLE);
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

#[derive(Default)]
struct Volume {
    captured: Option<(IAudioEndpointVolume, DuckedVolume)>,
}
impl Volume {
    fn set(&mut self, duck: bool) -> Result<(), String> {
        if duck && self.captured.is_none() {
            unsafe {
                let enumerator: IMMDeviceEnumerator =
                    CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                        .map_err(|e| format!("无法访问 Windows 音量: {e}"))?;
                let endpoint = enumerator
                    .GetDefaultAudioEndpoint(eRender, eMultimedia)
                    .map_err(|e| format!("当前没有默认音频输出: {e}"))?;
                let volume: IAudioEndpointVolume = endpoint
                    .Activate(CLSCTX_ALL, None)
                    .map_err(|e| format!("无法访问输出音量: {e}"))?;
                let capture = DuckedVolume::new(
                    volume
                        .GetMasterVolumeLevelScalar()
                        .map_err(|e| e.to_string())?,
                );
                volume
                    .SetMasterVolumeLevelScalar(capture.applied(), std::ptr::null())
                    .map_err(|e| e.to_string())?;
                self.captured = Some((volume, capture));
            }
        } else if !duck {
            if let Some((volume, capture)) = &self.captured {
                // Restore the captured endpoint, even if the default changed.
                // Respect any user adjustment made during the conversation.
                unsafe {
                    if let Some(original) = capture.restore(
                        volume
                            .GetMasterVolumeLevelScalar()
                            .map_err(|e| e.to_string())?,
                    ) {
                        volume
                            .SetMasterVolumeLevelScalar(original, std::ptr::null())
                            .map_err(|e| e.to_string())?;
                    }
                }
                self.captured = None;
            }
        }
        Ok(())
    }
}
impl Drop for Volume {
    fn drop(&mut self) {
        if let Err(e) = self.set(false) {
            log::warn!("Conversation volume restore failed: {e}");
        }
    }
}

pub struct Device {
    file: OwnedHandle,
    read_event: OwnedHandle,
    write_event: OwnedHandle,
    read_buffer: Vec<u8>,
    volume: Volume,
    // COM must outlive the volume interface (fields drop in declaration order).
    _apartment: Apartment,
}

impl Device {
    pub fn open(path: &str) -> Result<Self, String> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED)
                .ok()
                .map_err(|e| e.to_string())?;
        }
        let apartment = Apartment;
        let path = wide(path);
        let file = OwnedHandle(
            unsafe {
                CreateFileW(
                    PCWSTR(path.as_ptr()),
                    GENERIC_READ.0 | GENERIC_WRITE.0,
                    FILE_SHARE_MODE(0),
                    None,
                    OPEN_EXISTING,
                    FILE_FLAG_OVERLAPPED,
                    None,
                )
            }
            .map_err(|e| {
                format!("无法打开控制设备，请检查耳机连接，或退出占用通道的其他软件: {e}")
            })?,
        );
        let read_event = OwnedHandle(
            unsafe { CreateEventW(None, true, false, PCWSTR::null()) }
                .map_err(|e| e.to_string())?,
        );
        let write_event = OwnedHandle(
            unsafe { CreateEventW(None, true, false, PCWSTR::null()) }
                .map_err(|e| e.to_string())?,
        );
        Ok(Self {
            file,
            read_event,
            write_event,
            read_buffer: vec![0; 16 * 1024],
            volume: Volume::default(),
            _apartment: apartment,
        })
    }

    fn finish(
        file: HANDLE,
        event: HANDLE,
        op: *mut OVERLAPPED,
        result: windows::core::Result<()>,
        timeout: Duration,
    ) -> Result<Option<u32>, String> {
        if let Err(error) = result {
            if error.code() != ERROR_IO_PENDING.to_hresult() {
                return Err(format!("控制通道读写失败: {error}"));
            }
            let wait = unsafe {
                WaitForSingleObject(event, timeout.as_millis().min(u32::MAX as u128) as u32)
            };
            if wait != WAIT_OBJECT_0 {
                // OVERLAPPED and its buffer stay alive until canceled I/O completes.
                unsafe {
                    let _ = CancelIoEx(file, Some(op));
                }
                let mut count = 0;
                let drained = unsafe { GetOverlappedResult(file, op, &mut count, true) };
                if wait == WAIT_TIMEOUT {
                    return match drained {
                        Ok(()) => Ok(Some(count)), // completed concurrently with cancellation
                        Err(e) if e.code() == ERROR_OPERATION_ABORTED.to_hresult() => Ok(None),
                        Err(e) => Err(format!("控制通道已断开: {e}")),
                    };
                }
                return Err("等待控制通道失败".into());
            }
        }
        let mut count = 0;
        unsafe { GetOverlappedResult(file, op, &mut count, true) }
            .map_err(|e| format!("读取控制结果失败: {e}"))?;
        Ok(Some(count))
    }
}

impl Transport for Device {
    fn write(&mut self, packet: &[u8]) -> Result<(), String> {
        unsafe { ResetEvent(self.write_event.0) }.map_err(|e| e.to_string())?;
        let mut op = OVERLAPPED {
            hEvent: self.write_event.0,
            ..Default::default()
        };
        // No shared Rust reference to OVERLAPPED while Windows can mutate it.
        let op = std::ptr::addr_of_mut!(op);
        let result = unsafe { WriteFile(self.file.0, Some(packet), None, Some(op)) };
        let count = Self::finish(
            self.file.0,
            self.write_event.0,
            op,
            result,
            Duration::from_secs(1),
        )?
        .ok_or("发送控制指令超时")?;
        if count != packet.len() as u32 {
            return Err("控制指令未完整发送".into());
        }
        Ok(())
    }
    fn read(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, String> {
        unsafe { ResetEvent(self.read_event.0) }.map_err(|e| e.to_string())?;
        let mut op = OVERLAPPED {
            hEvent: self.read_event.0,
            ..Default::default()
        };
        let op = std::ptr::addr_of_mut!(op);
        let result = unsafe { ReadFile(self.file.0, Some(&mut self.read_buffer), None, Some(op)) };
        match Self::finish(self.file.0, self.read_event.0, op, result, timeout)? {
            Some(count) if count as usize <= self.read_buffer.len() => {
                Ok(Some(self.read_buffer[..count as usize].to_vec()))
            }
            Some(_) => Err("驱动返回的控制数据长度无效".into()),
            None => Ok(None),
        }
    }
    fn duck(&mut self, enabled: bool) -> Result<(), String> {
        self.volume.set(enabled)
    }
}
