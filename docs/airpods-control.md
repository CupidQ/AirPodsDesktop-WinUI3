# AirPods 原生控制（预览）

耳机控制页提供降噪、通透、自适应模式切换，对话感知开关，以及单耳降噪开关。控制通道连接的是 Windows 已配对的设备，与电量页收到的随机 BLE 广播地址独立。

## 使用条件

- AirPods 已在 Windows 中配对并连接，建议佩戴耳机。
- 安装兼容的 MagicAAP 驱动。查看维护者的[驱动说明](https://help.magicpods.app/fun-magicaap/)与[社区版本说明](https://help.magicpods.app/fun-magicaap-community/)，按适合系统的方案安装。此应用的安装包包含用户态控制客户端，驱动需另行安装。
- 控制接口被其他应用占用时，退出占用接口的软件后重试。

打开“耳机控制”，刷新设备列表，选择耳机，再点击“连接控制通道”。只有耳机已回传的设置才开放。自适应模式还要求匹配到支持的硬件型号，或耳机已回报处于自适应模式。

界面显示耳机最后确认的设置。发送命令后等待匹配的设置通知；超时会提示“未回传确认”，不会将写入成功等同于设置成功。耳机断开后各项设置恢复未知状态；重连重新获取状态。首版提供三种听音模式，耳机处于“关闭”时会显示该状态。

## 对话音量联动

对话感知开关修改耳机内部设置。“对话时降低 Windows 输出音量”是额外的本地选项，默认关闭，需要用户主动开启。

开启后，耳机发来开始说话的通知时，应用把当时默认输出设备的主音量降至原来的 30%，影响该输出设备上的所有应用。收到结束对话的通知、关闭联动、断开通道或退出应用时，恢复捕获的输出设备音量。若用户在对话期间手动调整过音量，则保留新值；默认输出切换后也只恢复原先捕获的设备。

此联动依赖控制通道持续连接。强制终止进程无法执行恢复操作，可在 Windows 音量设置中手动恢复。音量 API 失败时，页面会显示原因。

## 实现与协议参考

协议编码、状态与通知解析、控制会话、超时处理、音量联动均在 Rust 核心中实现。C# 负责后台调用和 UI 呈现。沿用原有字符串释放函数，新增接口返回 UTF-8 JSON，原有电量与设置结构体布局不变。

Windows 客户端枚举 MagicAAP 私有接口 `{9eec98bb-3c54-45d4-a843-7900c4635e08}`；未发现时检查服务接口 `{74ec2172-0bad-4d01-8f77-997b2be0722a}`。仅打开本次枚举得到的接口。后台线程使用重叠 I/O、取消并等待 I/O 完成后释放缓冲区，串行执行设置命令。

| 设置 | AAP 标识 | 值 |
| --- | --- | --- |
| 听音模式 | `0x0D` | 1 关闭、2 降噪、3 通透、4 自适应 |
| 对话感知 | `0x28` | 1 开、2 关 |
| 单耳降噪 | `0x1B` | 1 开、2 关 |

公开协议研究及接口参考（固定到移植时读取的版本）：

- [LibrePods](https://github.com/librepods-org/librepods/tree/efccc4499b2c1fd2ac243e975ae1f42d8e828ffd)：`linux/airpods_packets.h`、`linux/BasicControlCommand.hpp`、Android `AACPManager.kt`。GPL-3.0，Copyright 2025 LibrePods contributors。
- [MagicPodsCore](https://github.com/steam3d/MagicPodsCore/tree/173245a14beb40fafef3a276da55747a0c0a1c59)：AAP setters/watchers/enums；用于交叉核对命令、初始化、产品 ID 和对话状态。GPL-3.0，Copyright 2020–2026 Aleksandr Maslov 与 Andrei Litvintsev。
- [AAP Head Tracker](https://github.com/FIocker/aap-head-tracker/tree/767bbd37b93979084bdcc3e4dc9b9f13c72f82ce)：Windows MagicAAP 接口 GUID 与公开的 ReadFile/WriteFile 用法参考。GPL-3.0-only，FIocker。该项目实测硬件为 AirPods Max 2，不能视为本应用 Pro 3 实测。

这里的 Rust 实现为本项目编写，按仓库 GPL 许可发布；保留协议来源与贡献者署名。应用与 Apple、MagicPods、LibrePods 均无隶属关系。

## 验证范围

自动化测试覆盖命令字节、分段和合并通知、无效通知、握手、设置确认、无响应超时、断开、未知能力限制、音量恢复规则，以及新增 C# / Rust JSON 接口。Windows CI 编译 WinUI 与原生核心并打包。

尚未连接真实 MagicAAP 驱动和 AirPods Pro 3 测试。请在实机确认：三种模式切换后耳机声音与回传一致；在 iPhone 或耳机柄切换后页面跟随更新；对话与单耳降噪开关生效；音量联动在结束对话、手动改音量、切换输出和断开时处理正确。不同驱动、型号与固件的兼容性以实机结果为准。
