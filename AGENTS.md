# 开发约定

环境、目录和命令见 [docs/development.md](docs/development.md)。

- 使用 `scripts/build.ps1` 构建，使用 `scripts/build-installer.ps1` 打包。
- `crates/apd-core` 为 Rust 核心，`app/AirPodsDesktop` 为 WinUI 前端。
- Rust 与 C# 的 FFI 结构体布局必须同步，界面更新使用 `DispatcherQueue.TryEnqueue`。
- 托盘使用 `TrayIconService.cs` 的 Win32 实现。
- 保留 `Cargo.lock` 和 `LICENSE`，不提交构建产物、日志或凭据。
