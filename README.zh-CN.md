# Yeravich

[English](README.md) | 简体中文

一个常驻后台的中英翻译工具。选中文字或复制文本，按 Alt+Q 唤出翻译，用完按 Esc 收起。Mac 上用 Option+Q。

默认用 bilibili 的免费 [Index Translate](https://github.com/bilibili/Index-Translate#free-public-api) 服务，不需要 API Key。翻译需要联网，原文会发送到该服务。

![Yeravich 界面](https://github.com/user-attachments/assets/7ab96e45-3120-4ebe-a80d-636a22e5abe9)

目前只在 Hyprland 和 macOS（Apple Silicon）上做过实机验证。

## 使用

先启动 Yeravich，再从其他应用按快捷键唤出。选区或剪贴板里的文本会自动填入左侧并开始翻译，右侧显示结果。点击“复制”复制译文。

手动输入或粘贴后，按 Enter 翻译，Shift+Enter 换行。顶部的 ⇄ 切换中英翻译方向，右上角的语言菜单切换界面语言。中文系统默认中译英，其他系统默认英译中。

| 操作 | Hyprland | macOS |
| --- | --- | --- |
| 唤出并翻译 | Alt+Q（需绑定） | Option+Q |
| 收起窗口 | Esc | Esc |
| 退出应用 | Ctrl+Q | Cmd+Q |
| 翻译输入框中的原文 | Enter | Enter |
| 换行 | Shift+Enter | Shift+Enter |

除唤出快捷键外，其余按键都在 Yeravich 窗口内使用。

关闭窗口和 Esc 一样，只收起界面，原文和译文都会保留。点击“退出”或按 Ctrl+Q / Cmd+Q 才会结束进程，之后需要重新启动应用。

在 Yeravich 中编辑时，再按 Alt+Q / Option+Q 不会覆盖当前内容。托盘菜单的“显示 Yeravich”也只恢复窗口，不会重新读取文本。

## Hyprland

使用 Linux x86_64 AppImage。发布后的安装包在 [Releases](https://github.com/zonowry/yeravich/releases)；手动构建的包在 **Actions → AppImage → 运行记录 → Artifacts → Yeravich-linux-x86_64**。

下载 `.AppImage` 和同名 `.sha256`，从 Artifacts 下载的压缩包需要先解压。以下以 0.0.1 为例：

```sh
sha256sum --check Yeravich-0.0.1-x86_64.AppImage.sha256
chmod +x Yeravich-0.0.1-x86_64.AppImage
./Yeravich-0.0.1-x86_64.AppImage
```

无法挂载 AppImage 时，在启动命令后加 `--appimage-extract-and-run`。

快捷键需要 `xdg-desktop-portal` 和 `xdg-desktop-portal-hyprland`。AppImage 已带有读取剪贴板的工具；自行编译运行时还要安装 `wl-clipboard`。

在仓库目录执行以下命令，安装桌面入口和图标：

```sh
install -Dm644 data/com.zonowry.yeravich.desktop "${XDG_DATA_HOME:-$HOME/.local/share}/applications/com.zonowry.yeravich.desktop"
install -Dm644 data/com.zonowry.yeravich.svg "${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps/com.zonowry.yeravich.svg"
```

把安装后的 `.desktop` 文件中的 `Exec=yeravich` 改为 AppImage 的绝对路径，路径含空格时加双引号。

在 `~/.config/hypr/hyprland.lua` 中绑定 Alt+Q：

```lua
hl.bind("ALT + Q", hl.dsp.global("com.zonowry.yeravich:show"))
```

保存后执行 `hyprctl reload config-only`。如果还在用旧的 `hyprland.conf`，写法是：

```ini
bind = ALT, Q, global, com.zonowry.yeravich:show
```

旧配置用 `hyprctl reload` 重载。Alt+Q 已有其他用途的话，需要先改掉冲突的绑定。

这里的 `global` 唤出的是已经运行的 Yeravich，使用前要先启动应用。

选区和剪贴板都能用于翻译，程序优先读取内容有变化的一处，两处都变化时选区优先。带入的文本不对时，直接在原文框里粘贴，再按 Enter。

## macOS

M 系列 Mac 使用文件名含 `macos-arm64` 的 DMG。发布后的安装包在 [Releases](https://github.com/zonowry/yeravich/releases)；手动构建的包在 **Actions → DMG → 运行记录 → Artifacts → Yeravich-macos-arm64**。

打开 DMG，把 Yeravich.app 拖进 Applications，再从“应用程序”启动。从 Artifacts 下载的压缩包需要先解压。

应用目前使用自签名，未经过 Apple 公证。第一次打开可能被系统拦截。确认下载来源后，在“系统设置 → 隐私与安全性”里点击“仍要打开”。[Apple 的相关说明](https://support.apple.com/en-us/102445)

想直接翻译选中的文字，需要先授权：

1. 点击 Yeravich 顶部的“选区权限”。
2. 在“系统设置 → 隐私与安全性 → 辅助功能”里开启 Yeravich。列表里没有的话，用 `+` 添加“应用程序”中的 Yeravich.app。
3. 回到其他应用，选中文字，按 Option+Q。

不授权也能用：先 Cmd+C 复制，再按 Option+Q。有些应用不支持读取选区，也用这个方法。

更新或移动应用后，如果划选翻译失效，重新添加并授权当前的 Yeravich.app。

## 修改配置

首次启动会自动创建配置文件，已有文件不会被覆盖：

| 平台 | 路径 |
| --- | --- |
| Hyprland | `~/.config/yeravich/config.toml` |
| macOS | `~/Library/Application Support/yeravich/config.toml` |

设置了 `XDG_CONFIG_HOME` 时，Hyprland 的配置在该目录下的 `yeravich/config.toml`。

默认配置如下，也可以查看 [config.example.toml](config.example.toml)：

```toml
[shortcuts]
show_window = "Alt+Q"

[translation]
endpoint = "https://index-translate.bilibili.com/v1/chat/completions"
model = "Index-Translate-35B-A3B"
# 换用需要 API Key 的服务时填写：
# api_key = "your-api-key"
```

改快捷键就修改 `show_window`，例如 `"Ctrl+Shift+T"`。Mac 的 `Alt` 就是 Option；Hyprland 还要修改前面的桌面绑定。

换翻译服务时，填写兼容 Chat Completions 的完整接口地址、模型和所需的 API Key。

修改后退出应用再启动。只关窗口不会重新读取配置。

## 从源码运行

安装 Rust 和平台所需的系统依赖后，在仓库目录运行：

```sh
cargo run --locked
```

开发相关内容见 [Rust 阅读与实践](docs/rust-learning.md)、[构建与验证](docs/rust-learning.md#构建与验证)和[发布流程](docs/rust-learning.md#自动发布-release)。
