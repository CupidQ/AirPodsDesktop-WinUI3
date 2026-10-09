# AirPodsDesktop-WinUI3 开发说明

## 架构

- `crates/apd-core`：Rust 核心。协议解析、BLE、状态机、媒体控制、FFI。
- `app/AirPodsDesktop`：WinUI 3 前端。P/Invoke 调用 `apd_core.dll`。

## 常用命令

```powershell
.\build.ps1                          # 全量构建
cargo test -p apd-core --lib         # Rust 单元测试
dotnet build app\AirPodsDesktop\AirPodsDesktop.csproj -c Release -p:Platform=x64
```

## 环境坑

1. **`PROGRAMFILES(X86)` 必须存在**，否则 NuGet restore 报 `path1` 空引用。
2. 长路径可能导致 XAML 编译静默失败，优先在短路径构建。
3. **不要用 H.NotifyIcon 的 XAML `TaskbarIcon`**，会使 XamlCompiler 无报错退出。托盘用 `Services/TrayIconService.cs`（Win32）。

## FFI 约定

`crates/apd-core/src/ffi/mod.rs` 与 `app/AirPodsDesktop/Services/Native.cs` 的结构体布局必须保持同步（`#[repr(C)]` / `LayoutKind.Sequential`）。

回调在后台线程触发，UI 必须自行 `DispatcherQueue.TryEnqueue`。
