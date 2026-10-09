# AirPodsDesktop-WinUI3

Windows 上的 AirPods 电量监视器，使用 WinUI 3 和 Rust 构建。

**[下载安装包](https://github.com/CupidQ/AirPodsDesktop-WinUI3/releases/download/v0.1.0-winui3/AirPodsDesktop-WinUI3-0.1.0-Setup-x64.exe)** · [Release 与校验文件](https://github.com/CupidQ/AirPodsDesktop-WinUI3/releases/tag/v0.1.0-winui3)

支持 Windows 10 1809+ / Windows 11 x64，安装包已包含所需运行环境。

## 功能

- 显示左耳、右耳和充电盒电量，支持托盘与电量弹窗。
- 入耳检测，摘下暂停、佩戴播放。
- 浅色、深色和跟随系统主题。
- 信号过滤、广播地址锁定，以及可选的登录自启动。

## 使用

1. 下载并安装，打开 AirPodsDesktop-WinUI3。
2. 在 Windows 蓝牙设置中配对 AirPods，再打开充电盒或佩戴耳机。
3. 在应用或托盘查看电量；自启动可在应用设置中开启。

这是预览版，真实 AirPods 兼容性尚未实测。安装包未签名，Windows 可能提示未知发布者。

媒体控制需要播放器支持 Windows SMTC。地址变化后可解除锁定；附近多台设备可能干扰识别。低延迟音频切换尚未支持。

## 开发

```powershell
.\scripts\build.ps1            # 构建与测试
.\scripts\build-installer.ps1  # 制作安装包
```

[开发说明](docs/development.md)包含环境要求、运行方式和打包说明。

## 许可

基于 [AirPodsDesktop](https://github.com/SpriteOvO/AirPodsDesktop) 重写，协议与功能参考 [OpenPods](https://github.com/adolfintel/OpenPods) 和 [MagicPods](https://magicpods.app/)。以 [GPL-3.0](LICENSE) 发布。
