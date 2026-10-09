# AirPodsDesktop-WinUI3

用 **WinUI 3** 重写 [SpriteOvO/AirPodsDesktop](https://github.com/SpriteOvO/AirPodsDesktop)：上层界面采用 C# / WinUI 3，底层协议、BLE 扫描与状态机用 **Rust** 实现，通过 C ABI（P/Invoke）桥接。

## 功能

- 通知区域（托盘）电池信息：左耳 / 右耳 / 充电盒
- 开盖 / 连接时电量弹窗
- 入耳检测，摘下暂停 / 佩戴播放（Windows SMTC）
- 低信号强度过滤、当前广播地址锁定与解除锁定
- 浅色 / 深色 / 跟随系统主题
- 设置持久化（`%APPDATA%\AirPodsDesktop\settings.json`）
- 用户主动开启的登录自启动（当前用户注册表 Run 项，默认关闭）

### 当前限制

- BLE 广播不代表 Windows 已完成音频配对；仍需在 Windows 蓝牙设置中配对耳机。
- AirPods 使用随机广播地址。地址锁定只过滤当前地址，地址变化或另一耳使用不同地址时可能收不到更新；此时请解除锁定。它不是永久设备身份绑定。
- 未锁定地址时，附近多台设备的广播可能互相干扰；没有经过验证的多设备身份识别。
- 低延迟音频切换尚未实现，相应开关已禁用。
- 媒体控制依赖播放器支持 Windows SMTC；无会话或播放器拒绝命令时会记录失败。
- 构建与自动化回归已验证，真实 AirPods、蓝牙兼容性和界面交互仍需实机测试。

## 架构

```
┌─────────────────────────────────────────────┐
│  WinUI 3 (C#)  app/AirPodsDesktop           │
│  MainWindow / DevicePage / BatteryPage      │
│  TrayIconService (Win32 Shell_NotifyIcon)   │
│  CoreBridge (P/Invoke + 回调)               │
└───────────────────┬─────────────────────────┘
                    │  C ABI  (apd_core.dll)
┌───────────────────▼─────────────────────────┐
│  apd-core (Rust)  crates/apd-core           │
│  protocol/  Apple Continuity 解析           │
│  state/     双耳广播合并状态机              │
│  ble/       Windows BLE AdvertisementWatcher│
│  media/     SMTC 播放控制                   │
│  settings/  JSON 设置                       │
│  ffi/       C ABI 导出                      │
└─────────────────────────────────────────────┘
```

## 协议要点

AirPods 通过 BLE 广播 Apple Continuity **Proximity Pairing**（company id `0x004C`，type `0x07`）上报电量。载荷 27 字节：

| 偏移 | 长度 | 含义 |
|------|------|------|
| 0 | 1 | packet type = 0x07 |
| 1 | 1 | remaining length = 25 |
| 3 | 2 | model id（小端） |
| 5 | 1 | 状态位：入耳 / 双耳入盒 / 广播来源耳 |
| 6 | 2 | 电量 nibble（0–10 → 0–100%）+ 充电标志 |
| 8 | 1 | 开盖状态 |
| 9 | 1 | 颜色 |
| 11 | 16 | hash / 加密载荷（日志需脱敏） |

协议字段是 `curr` / `anot`（当前耳 / 另一侧），由 `broadcast_from` 决定左右映射。完整字段说明见 `crates/apd-core/src/protocol/airpods.rs`。

## 构建

### 环境要求

- Windows 10 1809+ / Windows 11 x64
- [Rust](https://rustup.rs)（`stable-msvc`）
- [.NET 8 SDK](https://dotnet.microsoft.com/download/dotnet/8.0)
- Visual Studio 2022 **Build Tools**（C++ 桌面开发 + Windows SDK）

### 一键构建

```powershell
.\build.ps1
```

### 手动构建

```powershell
# 1. Rust 核心
cargo build -p apd-core --release
cargo test  -p apd-core --lib

# 2. WinUI 3 前端（注意环境变量，见下）
dotnet build app\AirPodsDesktop\AirPodsDesktop.csproj -c Release -p:Platform=x64

# Rust DLL 在第二步自动拷贝到前端输出目录。
# 必须先完成 Rust Release 构建，再构建前端。
```

### 环境变量注意

NuGet restore 在缺失 `PROGRAMFILES(X86)` 时会报
`Value cannot be null. (Parameter 'path1')`。
构建前请保证：

```powershell
$env:PROGRAMFILES = "C:\Program Files"
# PowerShell 中带括号的变量名：
[Environment]::SetEnvironmentVariable("PROGRAMFILES(X86)", "C:\Program Files (x86)", "Process")
$env:DOTNET_CLI_TELEMETRY_OPTOUT = "1"
```

## 运行

```powershell
.\app\AirPodsDesktop\bin\x64\Release\net8.0-windows10.0.19041.0\AirPodsDesktop.exe
```

或从 Visual Studio 打开 `app\AirPodsDesktop\AirPodsDesktop.csproj` 运行（x64）。

运行时需要 .NET 8 x64。Windows App SDK 文件随构建输出一起提供；移动程序时请保留整个输出目录，不要只复制 EXE。

## 项目布局

```
AirPodsDesktop-WinUI3/
├── Cargo.toml                 # Rust workspace
├── crates/apd-core/           # Rust 核心库（cdylib + staticlib + rlib）
│   └── src/
│       ├── protocol/          # Apple Continuity / AirPods 广播解析
│       ├── state/             # 双耳合并状态机
│       ├── ble/               # Windows BLE watcher
│       ├── media/             # SMTC 入耳检测媒体控制
│       ├── settings/          # 设置
│       └── ffi/               # C ABI
├── app/AirPodsDesktop/        # WinUI 3 前端
│   ├── Services/              # Native P/Invoke, CoreBridge, TrayIcon
│   └── Views/                 # DevicePage, BatteryPage, BatteryPopup, LogsPage
├── build.ps1
└── README.md
```

## 测试

Rust 单元测试和集成回归覆盖协议解析、左右翻转、充电时入耳过滤、RSSI/地址过滤、状态合并、两种断开重连方式、地址锁定切换及 FFI 布局：

```powershell
cargo test -p apd-core --locked
# 17 个单元测试 + 4 个集成回归测试
```

## 致谢与许可

协议逆向与功能灵感来自 [OpenPods](https://github.com/adolfintel/OpenPods)、[MagicPods](https://magicpods.app/) 与原 [AirPodsDesktop](https://github.com/SpriteOvO/AirPodsDesktop)（GPL-3.0）。本重写同样以 GPL-3.0 发布。

原项目许可证全文位于 [LICENSE](LICENSE)。
