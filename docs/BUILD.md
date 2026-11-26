# FileFlyer 编译构建说明

本文档说明如何在 Windows、macOS、Linux 桌面端，以及 Android、iOS 移动端构建 FileFlyer。

## 1. 通用环境

| 依赖 | 要求 |
| --- | --- |
| Node.js | 20 及以上，推荐 22 LTS |
| Rust | stable，建议最新稳定版（`rustup update stable`） |
| Tauri CLI | 已包含在 devDependencies 中，通过 `npm run tauri` 调用 |
| Git | 可选 |

安装依赖并做基础校验：

```bash
npm install
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
```

`npm run build` 会执行 Vue 类型检查和前端生产构建；Rust 单元测试覆盖协议帧与文件名处理。

`src-tauri/gen/android` 与 `src-tauri/gen/apple` 是本地生成的平台工程，不随源码提交；首次构建移动端时通过 `tauri android init` / `tauri ios init` 生成。

## 2. Windows 桌面端

### 前置依赖

1. 安装 [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)，勾选“使用 C++ 的桌面开发”和 Windows 10/11 SDK。
2. Windows 10 需要安装 [WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)；Windows 11 已内置。
3. 安装 Rust（MSVC 工具链）：

```powershell
winget install Rustlang.Rustup
rustup default stable-msvc
```

### 开发与构建

```powershell
npm install
npm run tauri dev
```

生成安装包：

```powershell
npm run tauri build
```

产物位置：

- 可执行文件：`src-tauri/target/release/fileflyer.exe`
- MSI 安装包：`src-tauri/target/release/bundle/msi/`
- NSIS 安装包：`src-tauri/target/release/bundle/nsis/`

只构建可执行文件、不打安装包（适合快速验证）：

```powershell
npm run tauri build -- --debug --no-bundle
```

首次运行会触发 Windows 防火墙提示。需要允许“专用网络”，否则 UDP 发现和 TCP 传输会被拦截。

## 3. macOS 桌面端

```bash
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
npm install
npm run tauri dev
npm run tauri build
```

产物：

- 应用：`src-tauri/target/release/bundle/macos/FileFlyer.app`
- 安装镜像：`src-tauri/target/release/bundle/dmg/`

如需同时构建 Intel 与 Apple Silicon 通用包：

```bash
rustup target add x86_64-apple-darwin aarch64-apple-darwin
npm run tauri build -- --target universal-apple-darwin
```

macOS 14 及以上首次访问局域网时，系统会询问“本地网络”权限；请允许，否则无法发现其他设备。若误点拒绝，可在“系统设置 → 隐私与安全性 → 本地网络”中重新开启。

## 4. Linux 桌面端

以 Debian / Ubuntu 为例，安装 Tauri 2 所需系统依赖：

```bash
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

然后：

```bash
npm install
npm run tauri dev
npm run tauri build
```

产物：

- 可执行文件：`src-tauri/target/release/fileflyer`
- 安装包：`src-tauri/target/release/bundle/deb/`、`bundle/rpm/`、`bundle/appimage/`

AppImage 运行需要 FUSE；无 FUSE 的环境可使用 `--appimage-extract-and-run` 参数运行。

## 5. Android

### 5.1 前置依赖

1. 安装 [Android Studio](https://developer.android.com/studio)。
2. 在 SDK Manager 中安装：
   - Android SDK Platform（本工程生成时使用 Platform 37，建议安装最新版）
   - Android SDK Build-Tools
   - NDK (Side by side)，推荐 27.x
   - Android SDK Platform-Tools（包含 adb）
3. 安装 JDK 17，并设置 `JAVA_HOME`。
4. 配置环境变量（以 Windows 为例，路径按本机实际安装位置调整）：

```powershell
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
$env:ANDROID_SDK_ROOT = $env:ANDROID_HOME
$env:NDK_HOME = "$env:ANDROID_HOME\ndk\27.1.12297006"
$env:JAVA_HOME = "C:\Program Files\Java\jdk-17"
```

macOS / Linux：

```bash
export ANDROID_HOME="$HOME/Library/Android/sdk"   # Linux 通常为 $HOME/Android/Sdk
export ANDROID_SDK_ROOT="$ANDROID_HOME"
export NDK_HOME="$ANDROID_HOME/ndk/27.1.12297006"
export JAVA_HOME="/Library/Java/JavaVirtualMachines/jdk-17.jdk/Contents/Home"
```

5. 安装 Rust Android 目标（`tauri android init` 也会自动安装）：

```bash
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
```

6. Windows 特有：Tauri CLI 在构建 Android 时需要把 Rust 动态库链接或复制到 Gradle 工程中。Windows 创建符号链接需要开发者模式，否则会报：

```
Creation symbolic link is not allowed for this system.
```

请在“设置 → 系统 → 开发者选项”中开启“开发者模式”，或改用 macOS / Linux / CI 构建。

### 5.2 初始化与开发

首次执行一次初始化，生成 `src-tauri/gen/android`：

```bash
npm run tauri android init
```

连接已开启 USB 调试的 Android 设备（或使用 Android 11 以上的无线调试），确认设备可见：

```bash
adb devices
```

启动开发版本：

```bash
npm run tauri android dev
```

### 5.3 打包

```bash
# 发布 APK
npm run tauri android build -- --apk

# 调试 APK（使用调试签名，适合直接安装测试）
npm run tauri android build -- --apk --debug

# 仅构建 ARM64，加快速度
npm run tauri android build -- --apk --target aarch64

# 构建 Android Studio x86_64 模拟器使用的调试 APK
npm run tauri android build -- --apk --debug --target x86_64

# Google Play 使用的 AAB
npm run tauri android build -- --aab
```

产物位于：

```
src-tauri/gen/android/app/build/outputs/apk/<flavor>/<debug|release>/
src-tauri/gen/android/app/build/outputs/bundle/<flavor>/<debug|release>/
```

安装到已连接设备：

```bash
adb install -r src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk
```

具体文件名与目录以构建输出为准。

### 5.4 权限与保存位置

- `AndroidManifest.xml` 包含 `INTERNET`、`ACCESS_NETWORK_STATE`、`ACCESS_WIFI_STATE`、
  `CHANGE_WIFI_MULTICAST_STATE`，以及 Android 17 局域网访问所需的 `ACCESS_LOCAL_NETWORK`。
- `MainActivity` 会在 Android 17 及以上请求本地网络运行时权限，并持有 Wi-Fi 组播锁以稳定接收 UDP 广播。
  首次启动请允许“附近设备 / 本地网络”权限；拒绝后需要在系统设置中重新授予，否则无法发现设备或传输文件。
- 接收文件默认保存在应用专属外部目录（`Android/data/<包名>/files/Documents/FileFlyer`），设置页会显示实际路径。
- 由于 Android 分区存储限制，当前版本不会直接写入公共“下载”目录。后续如需导出，可通过系统的文件管理或分享功能扩展。
- 部分厂商的省电策略会限制后台网络；传输时请保持应用在前台。

### 5.5 正式签名

发布到应用商店前需要配置签名。生成本地密钥：

```bash
keytool -genkeypair -v -keystore fileflyer-release.keystore \
  -alias fileflyer -keyalg RSA -keysize 2048 -validity 10000
```

然后按 Tauri 官方 Android 签名文档，将密钥信息配置到生成的 Gradle 工程中：

- https://v2.tauri.app/distribute/sign/android/

修改 `tauri.conf.json` 中的 `identifier` 后，建议删除 `src-tauri/gen/android` 并重新执行 `npm run tauri android init`，避免包名与旧工程不一致。
注意：重新生成 Android 工程会丢失 `AndroidManifest.xml` 中的本地网络权限，以及 `MainActivity` 中的权限请求和组播锁代码，需要重新补齐。

## 6. iOS

iOS 构建必须在 macOS + Xcode 环境中进行。

### 6.1 前置依赖

```bash
xcode-select --install
# 安装 Xcode 并在 Xcode 设置中登录 Apple 开发者账号
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
```

### 6.2 初始化与运行

```bash
npm run tauri ios init
npm run tauri ios dev
```

构建：

```bash
npm run tauri ios build
```

首次在真机运行需要在 Xcode 中为工程选择开发团队（Signing & Capabilities），并信任开发者证书。

### 6.3 局域网权限与文件访问

iOS 14 起，应用访问局域网会触发系统授权。项目已通过 `src-tauri/Info.ios.plist` 提供
`NSLocalNetworkUsageDescription`，并在 `tauri.conf.json` 的 `bundle.iOS.infoPlist` 中合并，
无需手动修改生成的 Xcode 工程。

注意：iOS 还会限制 UDP 广播/组播流量。除本地网络权限外，自动发现还需要 Apple 批准的
`com.apple.developer.networking.multicast` entitlement。未获批准时，允许本地网络权限后仍可通过
应用内的手动 IP 功能进行 TCP 传输。申请地址：
https://developer.apple.com/contact/request/networking-multicast

如需让接收目录出现在系统“文件”App 中，可在 `Info.ios.plist` 中加入：

```xml
<key>UIFileSharingEnabled</key>
<true/>
<key>LSSupportsOpeningDocumentsInPlace</key>
<true/>
```

首次运行若拒绝本地网络权限，可到“设置 → 隐私与安全性 → 本地网络”重新开启。传输时请保持应用在前台。

## 7. 构建验证清单

```bash
# 前端类型检查与生产构建
npm run build

# Rust 单元测试
cargo test --manifest-path src-tauri/Cargo.toml

# 桌面调试版完整构建
npm run tauri build -- --debug --no-bundle

# Android Rust 目标检查（需先配置 Android 环境变量）
rustup target add aarch64-linux-android
cargo check --manifest-path src-tauri/Cargo.toml --target aarch64-linux-android --lib
```

## 8. GitHub Actions 自动打包与发布

工作流文件：`.github/workflows/release.yml`。

触发方式：

- 推送 `v` 开头的标签：

```bash
git tag v0.1.0
git push origin v0.1.0
```

- 在 GitHub 仓库的 Actions 页面手动运行 `Build and Release`，填写发布标签、是否创建草稿，以及 iOS 导出方式。

工作流会在以下平台并行构建：

| 平台 | Runner | 产物 |
| --- | --- | --- |
| Windows | `windows-latest` | `.msi`、NSIS `.exe`、`FileFlyer-portable.exe`（免安装） |
| macOS | `macos-latest` | 通用架构 `.dmg` |
| Linux | `ubuntu-22.04` | `.deb`、`.AppImage` |
| Android | `ubuntu-latest` | 签名 `.apk` / `.aab`；未配置签名时为调试 `.apk` |
| iOS | `macos-latest` | 签名 `.ipa`；未配置签名时为模拟器 `.app` 压缩包（仅工作流产物） |

所有平台构建成功后，`release` 任务会汇总产物、生成 `SHA256SUMS.txt`，并创建或更新对应的 GitHub Release。

### 8.1 Android 签名 Secrets（可选）

未配置签名时工作流仍会产出可安装的调试 APK，但不适合发布到应用商店。

在 GitHub 仓库的 `Settings → Secrets and variables → Actions` 中添加：

| Secret | 说明 |
| --- | --- |
| `ANDROID_KEY_BASE64` | keystore 文件的 base64 内容 |
| `ANDROID_KEY_ALIAS` | key alias |
| `ANDROID_KEY_PASSWORD` | keystore 与 key 密码（脚本使用同一个值） |

生成本地密钥并将 keystore 转为 base64：

```bash
keytool -genkeypair -v -keystore upload-keystore.jks \
  -alias upload -keyalg RSA -keysize 2048 -validity 10000

# Linux
base64 -w 0 upload-keystore.jks > upload-keystore.base64
# macOS
base64 -i upload-keystore.jks -o upload-keystore.base64
```

将 `upload-keystore.base64` 的内容保存到 `ANDROID_KEY_BASE64`。工作流会调用
`scripts/configure-android-signing.mjs`，把签名配置写入生成的 Gradle 工程。

### 8.2 iOS 签名 Secrets（可选）

配置以下 Secrets 后，工作流会生成可安装的 IPA；否则只会构建模拟器版本，作为 CI 工作流产物保留，不放入 Release。

| Secret | 说明 |
| --- | --- |
| `IOS_CERTIFICATE` | `.p12` 导出证书的 base64 内容 |
| `IOS_CERTIFICATE_PASSWORD` | 导出 `.p12` 时设置的密码 |
| `IOS_MOBILE_PROVISION` | `.mobileprovision` 文件的 base64 内容 |
| `APPLE_DEVELOPMENT_TEAM` | Apple Developer Team ID |

iOS 导出方式在手动运行时可选择：

- `release-testing`：Ad Hoc / Release Testing，安装到已登记 UDID 的设备。
- `app-store-connect`：用于上传 App Store Connect。

证书类型、Provisioning Profile 类型必须与导出方式匹配。工作流的 iOS 步骤使用 Tauri 的
`IOS_CERTIFICATE`、`IOS_CERTIFICATE_PASSWORD`、`IOS_MOBILE_PROVISION` 环境变量完成手动签名。
详细说明参考：https://v2.tauri.app/distribute/sign/ios/

未配置上述 Secrets 时，工作流会使用占位 Team ID 完成 iOS 工程初始化，只构建无签名的模拟器 App；
该产物用于验证 iOS 编译链路，不能安装到真机，也不会放入 Release。

### 8.3 桌面端签名

当前工作流默认不配置 Windows 代码签名，安装包可能触发 SmartScreen 提示。macOS 产物同样默认未签名；
如需签名与公证，可在桌面构建步骤中按 Tauri 文档注入 `APPLE_CERTIFICATE`、`APPLE_SIGNING_IDENTITY`
等环境变量。

### 8.4 发布说明

- 推送标签创建的 Release 会直接发布；手动触发默认创建草稿，可在 GitHub 上检查后发布。
- 工作流只负责构建和创建 GitHub Release，不会自动上传 Google Play 或 App Store。
- `tauri.conf.json` 中的版本号应与标签保持一致，例如标签 `v0.1.0` 对应版本 `0.1.0`。

## 9. 常见问题

### Windows 打包报 “failed to bundle project: timeout: global”

Rust 编译和应用本体已经成功，失败发生在 Tauri 下载 WiX 工具（约 39 MB）时，属于网络超时。

处理方式：

1. 只生成 NSIS 安装包：

```bash
npm run desktop:build -- --bundles nsis
```

2. 或手动补齐 WiX 缓存，使 MSI 打包不再联网下载：
   - 下载 https://github.com/wixtoolset/wix3/releases/download/wix3141rtm/wix314-binaries.zip
   - 确认 SHA256 为 `6ac824e1642d6f7277d0ed7ea09411a508f6116ba6fae0aa5f2c7daa2ff43d31`
   - 解压到 `%LOCALAPPDATA%\tauri\WixTools314`
   - 重新运行 `npm run desktop:build`

### Gradle 下载超时

首次 Android 构建会下载 Gradle 发行包和 Android 依赖。如果访问 `services.gradle.org` 较慢或超时，可以：

- 为 Gradle 配置 HTTP / HTTPS 代理：在 `~/.gradle/gradle.properties` 中添加 `systemProp.https.proxyHost`、`systemProp.https.proxyPort` 等配置；
- 或使用可正常访问 Gradle 分发服务的网络 / CI 环境；
- 也可以手动下载相同版本的 `gradle-x.y-bin.zip`（例如使用国内镜像），放到
  `~/.gradle/wrapper/dists/gradle-x.y-bin/<哈希目录>/` 下，再重新执行构建。Gradle Wrapper 会校验并解压本地压缩包。

Windows 上建议同时开启开发者模式，避免 Rust 动态库复制阶段创建符号链接失败。

### Windows 报 “Creation symbolic link is not allowed”

开启 Windows 开发者模式，或改用 macOS / Linux / CI 构建 Android。详见 5.1。

### 找不到 NDK / JDK

确认 `NDK_HOME`、`JAVA_HOME`、`ANDROID_HOME` 已设置，并在当前终端中生效。`NDK_HOME` 应指向具体的 NDK 版本目录，而不是 `ndk` 父目录。

### 设备之间互相发现不到

- 确认两台设备在同一网段，且连接的是普通 Wi-Fi 而不是访客 Wi-Fi。
- 检查 Windows 防火墙是否允许 FileFlyer 访问专用网络。
- 路由器可能开启了 AP 隔离，可改用手机热点，或在设备列表下方手动输入对方 IPv4 地址。
- 确认没有同时运行多个 FileFlyer 实例（发现端口固定为 UDP 45731）。

### 传输失败或校验不通过

- 检查接收端磁盘剩余空间。
- 传输期间不要修改正在发送的源文件。
- 校验失败时接收端会自动删除临时文件，不会留下损坏文件，可直接重试。

### Linux 启动报 WebKit 相关错误

确认已安装 `libwebkit2gtk-4.1-dev`，并检查发行版与 Tauri 2 的系统依赖要求：
https://v2.tauri.app/start/prerequisites/
