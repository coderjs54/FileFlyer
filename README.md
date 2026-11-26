# FileFlyer

FileFlyer 是一个完全基于局域网的跨端文件互传应用。使用一套 Vue 3 + Rust（Tauri 2）代码，同时构建 Windows / macOS / Linux 桌面程序与 Android / iOS 应用。

传输不经过任何外部服务器或云端，不使用互联网。设备在同一局域网内互相发现，文件通过 TCP 直连分片传输，并按文件进行 SHA-256 完整性校验。

## 功能

- 局域网设备自动发现：UDP 广播，2 秒心跳，8 秒离线判定
- 手机与电脑、电脑与电脑双向互传
- 先选设备、再选文件，符合直觉的操作流程
- 多文件批量发送，实时进度、速度与状态展示
- 每次接收默认弹窗确认，可在设置中开启自动接收
- 分片传输（256 KiB），支持大文件
- 每个文件独立 SHA-256 校验，校验通过后才会保留最终文件
- 接收完成后可在系统文件管理器中显示（桌面端）
- 支持手动输入 IP 连接，应对路由器屏蔽广播的场景
- 亮色 / 暗色主题自适应，桌面与移动端响应式布局

## 技术架构

```
┌────────────────────┐        UDP 45731 广播发现        ┌────────────────────┐
│  手机 / 电脑 A      │  ◄───────────────────────────►  │  手机 / 电脑 B      │
│  Vue 3 + WebView   │                                 │  Vue 3 + WebView   │
│        │ IPC       │                                 │        │ IPC       │
│  Rust / Tauri 2    │        TCP 直连文件传输          │  Rust / Tauri 2    │
│  发现 + 传输服务    │  ◄───────────────────────────►  │  发现 + 传输服务    │
└────────────────────┘                                 └────────────────────┘
```

- 前端：Vue 3、TypeScript、Vite
- 壳层：Tauri 2（桌面与移动端共用同一套前端与 Rust 代码）
- 发现：UDP 广播 + 定向广播，广播内容包含设备名、平台和 TCP 端口
- 传输：TCP 长连接，自定义长度前缀协议，文件按 256 KiB 分片
- 校验：发送端与接收端分别流式计算 SHA-256，比对一致后接收端才将临时文件重命名为正式文件
- 保存设置：本地 JSON 文件，无需数据库

## 快速开始

任选一台电脑作为开发机：

```bash
npm install
npm run tauri dev
```

构建桌面安装包：

```bash
npm run tauri build
```

更完整的 Windows、macOS、Linux、Android、iOS 构建步骤见 [docs/BUILD.md](docs/BUILD.md)。

## 使用方式

1. 在两台设备上打开 FileFlyer，并连接到同一个 Wi-Fi / 局域网。
2. 首次启动时，在系统防火墙提示中允许访问（建议仅勾选“专用网络”）。
3. 在“附近设备”中等待对方出现。
4. 点击设备右侧的“发送文件”，选择一个或多个文件。
5. 对方在弹出的确认框中点击“接收”。
6. 双方都可以在“传输记录”中查看进度、速度与校验结果。

如果自动发现失败，可在设备列表下方点击“通过 IP 地址连接”，输入对方设备在局域网中的 IPv4 地址。

## 文件保存位置

- 桌面端默认保存到系统“下载”目录下的 `FileFlyer` 文件夹，可在设置中修改。
- Android 默认保存到应用专属外部目录（`Android/data/<包名>/files/Documents/FileFlyer`），具体路径会显示在设置页。
- iOS 默认保存到应用沙箱的 `Documents/FileFlyer` 目录。按 [docs/BUILD.md](docs/BUILD.md) 配置 `UIFileSharingEnabled` 后，可在“文件”App 中访问。

接收到的文件在传输完成并通过 SHA-256 校验后才会出现在上述目录中。

## 网络与防火墙

| 用途 | 协议 / 端口 |
| --- | --- |
| 设备发现 | UDP 45731（有限广播 + 各网卡定向广播） |
| 文件传输 | TCP 随机端口，通过 UDP 广播告知其他设备 |

- Windows 首次运行会弹出防火墙提示，请允许“专用网络”访问，否则其他设备无法发现本机。使用便携版 exe 时，Windows 会把新路径视为新程序，需要再次允许。
- Android 17（targetSdk 37）首次启动会请求“附近设备 / 本地网络”权限，请选择允许；拒绝后系统会拦截局域网 UDP/TCP，导致无法发现或传输。
- iOS 首次访问局域网时也会请求本地网络权限；UDP 广播还要求 Apple 批准的 Multicast Networking entitlement，未获批时自动发现可能不可用，可在允许权限后使用手动 IP 传输。
- 如果使用公共网络或访客 Wi-Fi，路由器可能开启 AP 隔离，设备之间无法直连；请改用普通 Wi-Fi 或手机热点，也可以使用应用内的手动 IP 连接。
- 目前仅支持 IPv4 局域网与手动 IPv4 地址连接。
- 同一台电脑同时只能运行一个 FileFlyer 实例，因为发现端口固定。

## 安全说明

- 文件仅在同一局域网内点对点传输，不经过任何服务器。
- 接收默认需要人工确认；自动接收需要在设置中主动开启。
- 文件按 SHA-256 校验，能发现传输损坏，但传输过程本身未加密，不适合在不可信网络传输敏感文件。
- 接收端会过滤路径分隔符与 Windows 保留文件名，避免路径穿越；同名文件自动追加序号。

## 项目结构

```
src/                     Vue 3 前端
  components/            设备列表、传输记录、接收弹窗、设置页
  lib/api.ts             Tauri 命令封装
  lib/format.ts          文件大小、速度、状态等格式化
  App.vue                应用主界面与事件订阅
src-tauri/src/
  discovery.rs           UDP 局域网发现与手动 IP 连接
  protocol.rs            帧格式与控制消息定义
  transfer.rs            TCP 传输、分片、SHA-256 校验、进度事件
  files.rs               跨端文件读取（含 Android content:// 与 iOS 安全作用域）
  storage.rs             设置持久化
  state.rs               全局状态与前端视图模型
  lib.rs                 Tauri 启动与命令注册
src-tauri/icons/         各平台应用图标
.github/workflows/       GitHub Actions 多平台发布工作流
scripts/                 CI 构建辅助脚本
docs/BUILD.md            各平台编译构建说明
```

## 已验证环境

- Windows 11，Node.js 22.20，Rust 1.99，Tauri 2.12
- 桌面调试版构建成功
- 接收端通过脚本完成 3 MiB 文件传输与 SHA-256 校验
- 发送端在模拟局域网设备上完成 2 MiB 文件传输与 SHA-256 校验
- Android aarch64 Rust 库编译通过，Android 工程生成成功
- Android x86_64 调试 APK 构建成功，并已在 Medium_Phone（Android 37）模拟器上安装运行
- 模拟器内已验证：主界面、设置页、自动接收开关保存、应用配置文件持久化

## 传输协议简述

控制帧与数据帧均为 `1 字节类型 + 4 字节大端长度 + 负载`：

1. 发送方发送 `offer`，包含传输 ID、设备信息和文件列表。
2. 接收方确认后返回 `accept`，拒绝则返回 `reject`。
3. 发送方按文件序号发送数据分片，接收端写入同目录的临时 `.part` 文件。
4. 单个文件结束后发送 `fileComplete` 与 SHA-256，接收端比对后返回 `fileVerified`。
5. 全部文件完成后发送 `finish`，接收端返回 `result`，双方标记完成。

任何一方都可以发送 `cancel` 终止传输；连接中断时未完成的临时文件会被清理。

## 开发命令

```bash
npm run build          # 前端类型检查与生产构建
npm run tauri dev      # 启动桌面开发环境
npm run tauri build    # 构建桌面安装包
npm run desktop:build:portable   # 构建免安装便携版（单个 exe）
cargo test --manifest-path src-tauri/Cargo.toml   # Rust 单元测试
```

## GitHub Actions

推送 `v*` 标签，或在 GitHub Actions 中手动运行 `Build and Release`，会自动构建 Windows、macOS、Linux、Android、iOS 产物并创建 GitHub Release。

签名 Secrets、产物格式与发布说明见 [docs/BUILD.md](docs/BUILD.md) 第 8 节。
