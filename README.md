# AI Usage Meter

面向多 AI 平台的 Windows 桌面用量监测器，在同一个界面查看额度和 Token 消耗。目前支持 Codex 与 Cursor，并采用可扩展的平台切换结构。支持缓存优先切换、按秒设置自动同步，以及可贴边停靠的悬浮卡片/水波球。

## 下载

从 [GitHub Releases](https://github.com/clockclock1/ai-usage-meter/releases) 获取最新 Windows 安装包或独立可执行文件。

## 功能

- **多平台统计**：侧边栏切换当前平台；目前接入 Codex 与 Cursor，后续可扩展更多 AI 平台。任一时刻只显示所选平台的数据。
- **额度视角与 Token 视角**：额度视角展示当前周期用量、重置时间与账户信息；Token 视角展示本机可统计到的累计与今日 Token 用量。
- **独立刷新**：右上角按钮只刷新额度；Token 视角中的“刷新统计”只刷新 Token 数据。
- **后台定时同步**：默认每 60 秒同步，可设置为 5–86,400 秒。额度与 Token 不依赖当前打开的视角，都会按间隔自动刷新。
- **平台切换冷却**：切换平台时尝试同时刷新额度与 Token；距离上次自动同步不足设定间隔时，先使用缓存，并在冷却结束后同步，避免每次切换都重复请求。
- **悬浮显示**：支持紧凑卡片或可拖动、贴边收起的悬浮球；悬浮球水波的速度和幅度可调。
- **八套主题**：鎏金暗夜、翡翠墨玉、靛空电蓝、紫曜石、绯红黑曜、铂银极简、熔铜落日和极夜冰蓝；字体、进度、水波等颜色随主题切换。
- **全局设置**：可配置可用量/已用量显示、自动同步间隔、主题、代理和悬浮窗样式。

## 数据来源与隐私

- **Codex 额度**通过本机 Codex App Server 的 JSON-RPC 接口读取；Codex Token 统计读取本机 `~/.codex/sessions` 会话日志。今日用量按本机时区统计。
- **Cursor 额度**使用 Cursor 桌面端的本机登录状态读取账户用量；Cursor Token 统计由随应用提供的 Tokscale 组件同步并读取其本机缓存。Token 同步会复用新鲜缓存并遵守 Tokscale 的同步节流。
- 应用不经过本项目的服务器转发统计请求。Cursor 登录凭据会从 Cursor 本机登录数据库读取，并写入当前 Windows 用户目录下的 Tokscale 本地凭据文件（`.config/tokscale/cursor-credentials.json`），供 Tokscale 使用；请妥善保护该 Windows 用户目录。
- 应用设置、窗口位置和额度缓存保存在可执行文件旁的 `data/state.json`。Codex 会话日志与 Cursor 用量缓存不会上传到本项目。
- 支持系统代理、关闭代理或自定义 `http`、`https`、`socks5` 代理。

## 使用要求

- Windows 10/11 与 WebView2
- Codex Desktop 或 Codex CLI，并已使用 ChatGPT 账号登录（API Key 登录不提供 ChatGPT 订阅额度窗口）
- 如需 Cursor 统计：安装 Cursor Desktop，并在应用中登录 Cursor 账号

应用会自动查找 Codex Desktop、npm/nvm 全局安装目录及系统 `PATH` 中的 Codex。自定义安装位置可通过环境变量 `CODEX_QUOTA_CODEX_PATH` 指定 `codex.exe`。

首次在电脑上配置 Codex 时，可在终端检查登录状态：

```powershell
codex login
codex login status
```

## 开发

```powershell
npm install
npm run tauri dev
```

## 本地构建

```powershell
npm run tauri build
```

Windows 安装包位于 `src-tauri/target/release/bundle/`。应用同时使用随包提供的 Tokscale Windows 组件来读取 Cursor Token 用量。

## GitHub Actions 构建

`.github/workflows/windows-build.yml` 会在推送分支、创建 Pull Request 或手动运行时构建 Windows x64 和 ARM64 版本。每个架构会生成：

- `AI-Usage-Meter-64bit-Setup.exe` / `AI-Usage-Meter-ARM64-Setup.exe`：NSIS 安装包。
- `AI-Usage-Meter-64bit.exe` / `AI-Usage-Meter-ARM64.exe`：独立可执行文件。

普通工作流产物可从对应运行记录的 Artifacts 下载，保留 14 天。推送 `v` 开头的标签（例如 `v1.0.0`）时，安装包和独立 EXE 会附加到 GitHub Release。工作流只生成 Windows 产物。

## 本地交叉编译

安装 Rust MSVC 工具链和对应架构的 C++ 构建工具后，可编译单个目标：

```powershell
# Windows x64（Intel / AMD）
rustup target add x86_64-pc-windows-msvc
npm run tauri -- build --target x86_64-pc-windows-msvc --bundles nsis

# Windows ARM64
rustup target add aarch64-pc-windows-msvc
npm run tauri -- build --target aarch64-pc-windows-msvc --bundles nsis
```
