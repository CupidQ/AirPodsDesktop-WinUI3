# 开发说明

从仓库根目录执行以下命令。

## 环境

- Windows 10 1809+ / Windows 11 x64。
- Rust stable，MSVC 工具链。
- .NET 8 SDK。
- Visual Studio 2022 Build Tools：C++ 桌面开发与 Windows SDK。
- 打包另外需要 [Inno Setup 6.5+](https://jrsoftware.org/isdl.php) 和 Visual C++ x64 可再发行文件。

## 构建与运行

```powershell
.\scripts\build.ps1
.\app\AirPodsDesktop\bin\x64\Release\net8.0-windows10.0.19041.0\AirPodsDesktop.exe
```

构建脚本依次编译 Rust 核心、执行测试、编译 WinUI 前端并复制 `apd_core.dll`。普通构建输出需要 .NET 8 x64；运行或移动程序时保留整个输出目录。

只运行 Rust 测试：

```powershell
cargo test -p apd-core --locked
```

目前有 17 个单元测试和 4 个集成回归，覆盖协议解析、状态合并、断开重连、地址过滤及 FFI 布局。

## 打包

```powershell
.\scripts\build-installer.ps1
# 工具不在默认位置时，可指定 -IsccPath 和 -VcRedistDir。
```

脚本完成构建与测试、自包含发布、运行组件打包，输出到 `dist/`：

- `AirPodsDesktop-WinUI3-0.1.0-Setup-x64.exe`
- `SHA256SUMS.txt`

安装包包含 .NET、Windows App SDK 和 Visual C++ 运行组件。卸载保留个人设置和日志，并清理指向此安装目录的自启动项。

## 目录

| 目录 | 内容 |
| --- | --- |
| `app/AirPodsDesktop` | C# / WinUI 3 界面、托盘和 P/Invoke |
| `crates/apd-core` | Rust 协议、蓝牙、状态机、媒体控制和 FFI |
| `scripts` | 构建脚本及安装包模板 |
| `docs` | 开发说明 |

生成的 `target/`、`bin/`、`obj/` 和 `dist/` 不提交到仓库。协议字段直接参考 [airpods.rs](../crates/apd-core/src/protocol/airpods.rs)，避免维护重复说明。

## 约定与排错

- Rust FFI 结构体使用 `#[repr(C)]`，C# 使用 `LayoutKind.Sequential`，两端布局必须同步。
- 回调来自后台线程，更新界面需使用 `DispatcherQueue.TryEnqueue`。
- 托盘使用 `TrayIconService.cs` 的 Win32 实现；H.NotifyIcon 的 XAML `TaskbarIcon` 曾导致编译失败。
- NuGet 报 `path1` 空引用时检查 `PROGRAMFILES(X86)`；构建脚本会补全此变量。
- XAML 编译静默失败时，可将仓库放在更短的路径下构建。
- 用户设置位于 `%APPDATA%\AirPodsDesktop\settings.json`，日志位于 `%LOCALAPPDATA%\AirPodsDesktop\logs`。

真实耳机及蓝牙兼容性仍需实机验证。
