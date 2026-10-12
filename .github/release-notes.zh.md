AI Usage Meter 是面向多 AI 平台的 Windows 桌面用量监测器，目前支持 Codex 与 Cursor，可在同一界面查看额度与 Token 消耗。

本次更新完善会话视角与托盘状态，并修复 Cursor Token 同步卡住、会话名称显示不准等问题。

## 更新内容

- **托盘状态**：悬停系统托盘图标可弹出额度 / Token / 会话摘要；单击可固定弹层，双击打开主窗口；主窗口最小化后隐藏到托盘。
- **会话视角**：展示当前进行中的 Codex / Cursor 会话；会话名优先显示真实项目文件夹名，不再显示无意义的 `default` 模型与 `out` 计数。
- **Cursor Token**：有本地缓存时先立即展示，Tokscale 同步改为后台进行，避免界面一直卡在「正在更新」。
- **稳定性**：同步超时会强制结束卡住的 Tokscale 进程，避免读取线程长期阻塞。

## 下载说明

请按电脑类型选择（文件名均为英文，不含中文）：

| 文件 | 说明 |
| --- | --- |
| `AI-Usage-Meter-64bit-Setup.exe` | **大多数电脑用这个**（Intel / AMD），带安装程序 |
| `AI-Usage-Meter-64bit.exe` | 64 位便携版，下载后可直接运行 |
| `AI-Usage-Meter-ARM64-Setup.exe` | ARM 电脑（如骁龙本）安装包 |
| `AI-Usage-Meter-ARM64.exe` | ARM 电脑便携版 |
