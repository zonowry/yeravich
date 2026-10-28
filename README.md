# Yeravich

Yeravich 的目标是成为一个跨平台、启动迅速的翻译工具客户端：用户通过全局快捷键快速唤出窗口，看到需要翻译的文本及其译文，用完后继续原来的工作。

这个项目同时承载一个重要目标：通过开发实际可用的工具，逐步熟悉 Rust。代码应当符合 Rust 的常用写法，也应当让学习者能够追踪数据、理解状态变化，并解释设计选择。功能体验和学习价值共同影响实现方式。

## 产品方向

核心使用流程是：唤出窗口 → 输入或带入文本 → 翻译 → 查看或复制译文 → 收起窗口。优先让这条流程顺畅，再扩展其他功能。

- **响应快**：区分进程首次启动与进程已运行时的窗口唤出；窗口显示和输入不应等待翻译请求完成。
- **操作少**：围绕全局快捷键、输入焦点、原文和译文展示设计交互。
- **跨平台**：目标覆盖 Windows、macOS 和 Linux；平台相关能力集中处理，分别验证实际行为。
- **便于理解**：使用职责清楚的模块和显式的状态变化，随功能介绍所需的 Rust 知识。

翻译服务通过配置文件接入 OpenAI Chat Completions 兼容接口；快捷键唤出时可自动带入选区或剪贴板文本并翻译，手动输入仍按 Enter 提交。

## 常驻与窗口行为

应用使用 `iced::daemon` 常驻，日常通过快捷键唤出已有进程中的界面。关闭窗口时收起界面并保留应用状态，通过“退出”按钮结束进程。同一个应用进程在没有可见窗口时继续等待系统事件。

| 用户操作 | 行为 |
| --- | --- |
| 首次启动应用 | 启动进程，初始化应用、系统托盘并注册全局快捷键 |
| 快捷键或托盘可用后点击关闭按钮或在窗口中按 Esc | 收起界面，进程及原文、译文等应用状态保留 |
| 从其他应用按快捷键唤出 | 显示界面并请求输入焦点，复用已有进程；带入选区或剪贴板文本后自动翻译 |
| 托盘菜单选择“显示 Yeravich” | 恢复已有界面并保留文本，不读取剪贴板或自动提交翻译 |
| 在窗口或托盘选择“退出”，或在应用内按 Ctrl+Q（macOS 为 Cmd+Q） | 移除托盘、注销快捷键或关闭 Portal 会话，再结束进程 |

Windows、macOS 和 X11 窗口采用隐藏后再次显示的实现，保留窗口及其渲染资源。Wayland 下，当前 winit 不支持隐藏或主动聚焦已有窗口，因此收起时销毁窗口，唤出时重建；已经聚焦的窗口直接复用，未聚焦的窗口重新创建并交由桌面激活。进程、文本、翻译状态和窗口尺寸都保留，但 Iced 在最后一个窗口关闭时会释放 compositor，Wayland 的这条路径仍有图形初始化开销。[winit 平台说明](https://docs.rs/winit/0.30.13/winit/window/struct.Window.html#method.set_visible)

原生唤出快捷键默认为 **Alt+Q**，在 Mac 键盘上对应 **Option+Q**；可通过 `config.toml` 的 `shortcuts.show_window` 自定义，见下文。Wayland 会话（包括其中的 Xwayland 窗口）通过系统 Portal 注册，由桌面决定实际按键。注册成功后即可收起窗口，无需先触发一次唤出快捷键。Hyprland 可能返回空的按键描述，这不代表注册失败；实际按键仍须在 Hyprland 中绑定。快捷键和托盘均不可用时保持窗口可见；两个恢复入口都失效时恢复窗口，保留退出入口。

**Esc 收起窗口，Ctrl+Q（macOS 为 Cmd+Q）退出应用。** Esc 只在窗口收到无修饰键的按下事件且该事件未被控件消费时生效，输入区有焦点时也可使用。它与窗口关闭按钮共用检查：快捷键和托盘均不可用时保留窗口并显示原因，避免收起后无法唤回。

已在 Linux 的 Wayland、Xwayland 路径及 Apple Silicon macOS 验证 Esc/关闭按钮收起、快捷键唤回同一进程、恢复焦点与显式退出。Windows 和中文输入法组合输入期间的 Esc 行为尚未实机验证；这些交互检查不代表性能测量。

常驻唤出省去了重复创建进程的步骤，具体速度收益与资源占用尚未测量。应用通过系统回调和异步消息流接收快捷键，不设置周期性轮询。

## 系统托盘

启动后创建 Yeravich 托盘图标，菜单提供“显示 Yeravich”和“退出”，文案跟随界面语言切换。Windows 左键图标恢复窗口、右键打开菜单；macOS 点击图标打开菜单；Linux 支持 StatusNotifierItem 的桌面通常左键恢复、右键打开菜单，具体由桌面面板决定。

Linux 通过 `ksni` 使用会话 D-Bus，需要桌面提供 StatusNotifierItem 托盘宿主（例如启用了托盘的 Hyprland 面板；GNOME 通常需要 AppIndicator 扩展）。没有托盘宿主或初始化失败时，窗口显示提示，快捷键仍可使用；运行期间宿主离线会撤销托盘可用状态，宿主恢复后重新注册。启动时就没有宿主的情况，需要启动托盘宿主后重新运行应用。

Windows/macOS 通过 `tray-icon` 使用原生托盘，在窗口事件线程持有图标和菜单；Linux 的 D-Bus 工作在异步任务中执行，不增加 GTK/AppIndicator 系统库依赖。图标从已有 SVG 导出并嵌入程序，运行时不写图标或日志文件。新增依赖增加了构建、体积和维护成本，启动耗时、包大小与常驻内存的变化尚未测量。

Linux 托盘已验证：快捷键不可用时仍可收起和恢复窗口，恢复后进程不变且获得焦点；菜单退出后进程结束、托盘注销。托盘初始化失败时的关闭保护也已检查。

Windows/macOS 托盘模块已通过对应目标的独立 Clippy 检查，但完整应用的跨平台检查及托盘实机交互仍需补充。其他窗口功能的验证不代表托盘验证。

## 当前进度

目前基于 Rust 和 Iced，已接入通过配置文件设置的翻译服务，支持手动输入、快捷键带入文本、翻译和复制。

| 能力 | 当前状态 |
| --- | --- |
| 原文输入与译文展示 | 左右两栏同时可见，等分可用宽度并填满剩余高度；长文本各自滚动，译文只读，可选取和复制 |
| 界面语言 | 支持简体中文和英文，初始语言根据系统区域设置选择 |
| 翻译方向 | 支持中英方向切换，与界面语言切换分别处理 |
| 翻译任务状态 | 已有等待、错误及过期结果处理逻辑 |
| 真实翻译 | 已接入 Chat Completions 兼容接口，通过 TOML 文件配置 |
| 全局快捷键与窗口唤出 | 默认 Alt+Q（Mac 为 Option+Q），可通过配置文件自定义；Wayland 由桌面决定实际绑定 |
| 关闭窗口后常驻 | 已实现，尽量复用窗口；Wayland 保留进程并重建窗口 |
| 系统托盘 | 提供显示窗口、退出及语言同步；托盘可用时也允许收起窗口 |
| 自动带入待翻译文本 | Linux 按内容变更选择 PRIMARY 或剪贴板；macOS 授权后优先读取来源应用的选区，失败时使用剪贴板；Windows 读取剪贴板 |
| 启动与唤出性能 | 尚未建立测量基线 |
| 平台验证 | 已在 Hyprland 的 Wayland、Xwayland 路径，以及 Apple Silicon macOS 验证常驻与恢复；Windows、Intel Mac、其他 macOS 版本和独立 X11 会话仍需验证。DMG 工作流尚未在 GitHub 托管 runner 上实际运行 |

原文中按 **Enter** 翻译，按 **Shift+Enter** 换行，也支持多行粘贴和点击“翻译”提交。长文本自动折行，两个编辑区分别在框内滚动，原文不会再把译文推到窗口下方。默认窗口为 800×420，最小为 620×320（逻辑像素），可在 `src/theme/tokens.rs` 调整。

界面只保留一行工具栏、两个文本区和底部翻译操作。顶部 `⇄` 切换翻译方向，右侧下拉框切换界面语言，“退出”结束进程。说明移到悬停提示，不再常显大标题、介绍语、字数或就绪/常驻说明。请求进行中按钮显示“翻译中…”，配置、请求或快捷键异常仍保留反馈；底部配置/请求错误可滚动查看，避免挤占文本区域。

Linux 的 Wayland、Xwayland 路径已验证多行布局、最小窗口、滚动与选择、复制、翻译提交和错误显示。Windows、macOS 的新布局仍需实机验证；这些是交互检查，不是性能测量。

## 快捷键带入文本

在其他应用中选中文字，然后按唤出快捷键（例如 Hyprland 配置的 **Alt+Q**）。Linux 同时读取系统 PRIMARY selection 和普通剪贴板，采用**内容变更优先，选区次优先**：只有一处有效文本变化时使用该处，两处都变化时选区优先；两处都未变化时继续使用上次来源，没有上次来源则选区优先、剪贴板兜底。这样旧 PRIMARY 不会挡住 CLI 新写入的剪贴板，重复唤出也不会跳回旧文本。macOS 授予辅助功能权限后优先读取来源应用中未复制的选区，没有可读选区时使用剪贴板；Windows 当前只读取剪贴板，需先复制文本再唤出。捕获到非空文本后自动填入原文，并按当前翻译方向发送给配置的翻译服务，无需再按 Enter。手动输入或粘贴仍通过 **Enter** 或“翻译”按钮提交。

- Linux Wayland 需要安装 **wl-clipboard**，并确保 `wl-paste` 在应用的 `PATH` 中。无窗口时也能读取；缺少工具会在界面提示。Xwayland 有 `WAYLAND_DISPLAY` 时也走这条路径，否则与 X11 一样复用 Iced 的剪贴板接口；Windows 使用 Iced 剪贴板；macOS 的剪贴板兜底也使用 Iced。
- PRIMARY 由来源应用和桌面提供，可能保留上一次选区；它不保证代表鼠标当前划选的内容。程序比较启动和各次唤出时的文本快照，不监听持续变化，也不推断两次采样之间哪个复制动作更晚。仍可用 Ctrl+V / Cmd+V 粘贴指定内容。
- 窗口已聚焦时再次按快捷键保留正在编辑的内容。首次窗口就绪时只读取一次比较快照，不导入或翻译；迟到的启动快照不会覆盖已经开始的唤出。注册失败后恢复窗口不会重新捕获文本。
- 空白、非文本或读取失败时保留原文及译文，并给出提示，不触发翻译。带入不同文本会清除旧译文，使旧翻译结果失效；相同文本已有译文或正在请求时直接复用，不重复提交。翻译失败后再次捕获相同文本可自动重试。
- 捕获早于配置加载完成时，先保留文本，配置就绪后自动提交一次；期间编辑、清空或切换方向会取消这次自动提交。配置失败时保留原文并显示错误，不发送请求。
- 读取与窗口显示分别执行。Wayland 并行读取两个来源，各最多等待 500 毫秒；编辑、清空、切换方向、提交翻译、收起或退出都会取消待处理读取。连续唤出只接收最新一次结果，避免覆盖之后的输入。

此实现只读取文本，不模拟 Ctrl+C、不改写系统剪贴板，也不持续监听剪贴板变化。Linux 新增的运行时依赖为 `wl-clipboard`，Rust 侧仅启用现有 Tokio 的 `process` 和 `time` feature；其启动、体积和资源成本尚未测量。选区与文本格式选项见 [wl-clipboard 官方源码](https://github.com/bugaevc/wl-clipboard/blob/master/src/wl-paste.c)。

原生 Wayland 已验证选区优先、剪贴板回退、空白/非文本保留原文、已聚焦时保留内容与同一进程恢复。强制移除 `WAYLAND_DISPLAY` 的 Xwayland 路径尚未通过自动读取文本验证，建议保留原生 Wayland 环境。独立 X11 和 Windows 仍需实机验证。

Wayland 选区和剪贴板回退均已验证唤出后自动翻译、相同文本复用和空文本不发送请求。配置加载期间的延后提交、编辑后取消、过期结果拒绝、失败重试及读取工具错误由单元测试覆盖。

诊断日志仅输出到标准错误（stderr），应用和打包检查脚本不创建日志文件，无需日志轮转或清理。日志包括启动 PID、快捷键是否触发、是否因已聚焦而跳过、两个来源的字节数及变化情况、最终来源、是否复用译文，以及翻译请求的开始、完成、耗时或错误类型，不输出原文、译文或 API Key。可退出旧进程后在终端查看：

```sh
cargo run --locked
```

例如 `selected=Clipboard, primary_changed=false, clipboard_changed=true` 表示只有剪贴板更新，随后应出现 `request started` 与 `request finished`；`reused completed result` 表示确实读到了相同文本并复用已有译文。

已验证只有剪贴板变化时使用新剪贴板内容、重复唤出不跳回旧选区、两处同时变化时选区优先。状态回归测试还覆盖启动快照迟到、捕获取消后不更新比较快照，以及日志不包含正文。

### macOS 划选后直接翻译

1. 将 `Yeravich.app` 放在固定位置后启动，点击顶部“选区权限”。
2. 在“系统设置 → 隐私与安全性 → 辅助功能”中启用 Yeravich（部分系统版本显示为“设备控制和数据访问”）。若列表没有条目，可用 `+` 或拖入当前运行的 `.app` 添加。
3. 回到来源应用，选中文字，按 **Option+Q**。不需要 Cmd+C；程序优先带入选区并自动翻译。

授权后返回应用或再次唤出会重新检查，无需应用自行修改系统权限。未授权时保留说明，并可先复制文本走剪贴板。没有选区、来源应用不提供 `AXSelectedText`、应用退出或读取超时，也会尝试剪贴板；两处都没有可用文本时保留原文和译文。

选区来源在显示窗口前固定，跨进程读取在后台执行，每次 AX 消息设置 250 毫秒超时。Mac 的当前选区始终优先于剪贴板，不使用 Linux PRIMARY 的内容变化优先规则。已经聚焦 Yeravich 时再次按快捷键不会覆盖编辑内容；旧读取结果在编辑、收起或新的唤出后失效。程序不模拟 Cmd+C，不改变剪贴板，不提供鼠标悬停取词或 OCR。

兼容性取决于来源应用是否暴露可读的 Accessibility 选区；自绘控件、部分终端/浏览器页面可能不支持，不能保证所有应用都能划词。打包使用固定证书签名；从旧 ad-hoc 包升级、替换证书或改变路径后，系统可能要求重新授权，请授权实际运行的那份 `.app`。

如果权限开关已开启，日志仍显示 `primary_bytes: Err(AccessibilityRequired)`，需核对实际运行的应用及其签名。旧授权记录可能与重新构建后的程序不匹配，此时可在权限页面选中 Yeravich，点 `−` 移除旧条目，再用 `+` 添加当前 `/Applications/Yeravich.app` 并开启。系统要求的 Touch ID / 密码验证需在 Mac 本机完成。macOS 按代码签名要求识别获授权的程序，ad-hoc 构建更新后可能需要重新授权。[Apple 代码签名说明](https://developer.apple.com/documentation/technotes/tn3127-inside-code-signing-requirements)

验证选区时，先复制文本 B，再在不会自动复制选区的文本框中选中文本 A，按 Option+Q。应带入 A，且日志显示 `primary_bytes: Ok(...)`、`selected=Primary`。kitty 等应用若启用了“选择即复制”，选区和剪贴板会相同；看到译文并不能证明 AX 选区读取成功。

Apple Silicon macOS 的 release 构建已在原生 AppKit 文本框和 TextEdit 中验证未复制选区优先、剪贴板兜底、相同文本复用、聚焦时保留输入及窗口收起/退出；正常启动的 `.app` 已验证授权后的真实选区读取与翻译。SSH 启动与正常启动的信任来源可能不同，应分别检查。

## 配置翻译服务

首次启动时，如果默认配置不存在，程序会自动创建目录和 UTF-8 编码的 `config.toml`，默认使用 bilibili Index Translate；已有文件（包括无效配置）不会被覆盖。之后可以直接编辑文件，目前没有设置页面。默认文件路径由系统配置目录决定：

| 系统 | 路径 |
| --- | --- |
| Linux | `${XDG_CONFIG_HOME:-$HOME/.config}/yeravich/config.toml` |
| macOS | `~/Library/Application Support/yeravich/config.toml` |
| Windows | `%APPDATA%\yeravich\config.toml` |

也可通过非空环境变量 `YERAVICH_CONFIG` 指定文件路径；它优先于默认路径。显式指定的文件需要事先创建，路径写错或文件缺失时显示错误，不会改用默认服务。相对路径以启动时的工作目录为基准，桌面入口建议使用绝对路径。目录规则由 [`dirs::config_dir`](https://docs.rs/dirs/6.0.0/dirs/fn.config_dir.html) 提供。

默认配置如下；程序将 [config.example.toml](config.example.toml) 内嵌到可执行文件，首次使用不需要单独下载配置。接口与模型来自 [bilibili 官方说明](https://github.com/bilibili/Index-Translate#free-public-api)，默认不发送 API Key：

```toml
[shortcuts]
show_window = "Alt+Q"

[translation]
endpoint = "https://index-translate.bilibili.com/v1/chat/completions"
model = "Index-Translate-35B-A3B"
# 接口需要鉴权时填写；省略或留空时不发送 Authorization。
# api_key = "your-api-key"
```

`endpoint` 是完整的 HTTP(S) 请求地址，程序不会追加 `/v1/chat/completions`。`model` 不可为空。设置 `api_key` 时使用 `Authorization: Bearer <api_key>`。程序不会自动跟随 HTTP 重定向；如果服务返回 3xx，请填写最终接口地址。

本地开发直接启动即可生成默认配置：

```sh
cargo run --locked
```

开发时也可在仓库根目录使用单独的配置（该文件已加入 `.gitignore`）：

```sh
cp config.example.toml config.toml
YERAVICH_CONFIG="$PWD/config.toml" cargo run --locked
```

**配置在进程启动时读取，修改后需要点击“退出”并重新启动；仅关闭窗口仍然使用原来的配置。** 配置无法创建、读取或解析时，界面显示路径和修正提示，翻译按钮保持禁用，并保留窗口以便修正或退出。创建与读取配置在后台进行，不阻塞窗口打开；启动时不发送验证请求，提交翻译时才向配置中的服务发送原文。

配置成功后，输入文本并按 Enter 或点击“翻译”。Enter 不会修改原文；空白输入、配置尚未就绪或当前请求进行中时不提交。程序根据中英方向生成提示词，以非流式请求发送 `model`、`messages` 和 `stream = false`，从 `choices[0].message.content` 读取译文。连接超时为 10 秒，单次请求总超时为 60 秒（包括响应正文）。HTTP 错误、网络失败、格式不兼容和空译文分别提示，可以再次提交翻译重试。

输入编辑、清空或切换方向后，旧请求的成功和失败结果都会被忽略；底层请求仍可能运行至完成或超时。配置文件中若有密钥，请仅保存在自己的本地配置中。

## 自定义唤出快捷键

编辑上面的同一个 `config.toml`，添加或修改这一节；`[shortcuts]` 与 `[translation]` 是两个并列的 TOML 表：

```toml
[shortcuts]
show_window = "Alt+Q"
```

Mac 的 `Alt` 就是 `Option`，所以也可写成 `"Option+Q"`。例如改为 `"Ctrl+Shift+T"` 后，显式退出应用并重新启动，即可使用新组合键；目前不在运行中重新加载。旧配置省略 `[shortcuts]` 或 `show_window` 时自动采用 Alt+Q，不改写原文件。

修饰键可用 `Ctrl`、`Alt` / `Option`、`Shift`、`Super` / `Cmd`，也支持 `CmdOrCtrl`（Mac 为 Cmd，其他系统为 Ctrl）。使用 `+` 连接，修饰键在前，最后一个是主键。主键支持字母、数字、F1–F24、Space、Enter、Tab、Escape、Backspace、Delete、Insert、Home、End、PageUp、PageDown 和 ArrowUp / ArrowDown / ArrowLeft / ArrowRight；具体能否注册仍取决于系统和键盘，例如当前 macOS 后端不支持 F21–F24。

必须至少包含 Ctrl、Alt 或 Super / Cmd，避免拦截普通打字；应用退出组合键保留给 Ctrl+Q（Mac 为 Cmd+Q）。显式填写空值、拼错按键或字段会显示配置错误，不会悄悄改回默认键。系统报告注册失败时保持窗口可见，修改配置后退出重启即可重试。macOS 的原生 API 允许多个应用注册相同按键，因此注册成功不能证明没有冲突；遇到按键失效或同时唤出其他应用时，需要选择其他组合键。

Wayland 将此配置作为 Portal 的建议按键，桌面决定最终绑定；Hyprland 仍需修改下方的 `hl.bind`，仅改 TOML 不会改写桌面设置。修改前请确认所选组合键没有被桌面或其他应用使用。

macOS 已验证旧配置采用默认快捷键、自定义按键替代默认注册，以及无效配置或不受支持的按键不会使窗口无法恢复。Linux 也已验证常驻唤回、焦点、收起及退出。格式、Clippy、测试和编译检查通过；真实服务测试按设计默认忽略。

## Linux AppImage

[AppImage 工作流](.github/workflows/appimage.yml) 支持在 GitHub Actions 中选择 **AppImage → Run workflow** 手动构建，也会在推送 `v*` 标签时构建。成功后，从该次运行的 **Artifacts → Yeravich-linux-x86_64** 下载压缩包，内含 `.AppImage` 和 `.sha256`；当前上传到 Actions artifacts，不自动创建 GitHub Release。

解压后执行（将版本号替换为下载的文件名）：

```sh
sha256sum --check Yeravich-1.0.0-x86_64.AppImage.sha256
chmod +x Yeravich-1.0.0-x86_64.AppImage
./Yeravich-1.0.0-x86_64.AppImage
```

首次启动自动生成上面说明的 `config.toml`，无需管理员权限。替换 AppImage 升级不会覆盖用户配置。若系统不能挂载 AppImage，可运行 `./Yeravich-1.0.0-x86_64.AppImage --appimage-extract-and-run`。

AppImage 当前只打包 Linux x86_64，使用 Ubuntu 22.04 / glibc 2.35 作为构建基线；这不是所有发行版的兼容性保证。包内包含程序、桌面入口、图标、Wayland/X11 客户端库和 `wl-paste`。图形驱动、桌面服务、字体及 Wayland GlobalShortcuts Portal 仍由宿主系统提供，建议安装中文字体。AppImage 本身不会自动安装桌面入口；可以用支持 AppImage 的桌面集成工具，或复制包内的 `.desktop` 和图标到用户目录，将 `Exec` 改为 AppImage 的绝对路径。Wayland 快捷键配置见下文。

本地复现打包（Ubuntu 22.04 x86_64，已安装项目使用的 Rust 工具链）：

```sh
sudo apt-get update
sudo apt-get install -y build-essential pkg-config python3 curl file \
  desktop-file-utils squashfs-tools wl-clipboard \
  libwayland-dev libxkbcommon-dev libxkbcommon-x11-0 \
  libx11-6 libx11-xcb1 libxcursor1 libxi6 libxrandr2 \
  libxinerama1 libxrender1 libxfixes3 libxcb1 \
  xvfb xauth fonts-noto-cjk
bash scripts/build-appimage.sh
bash scripts/smoke-test-appimage.sh
```

产物在 `dist/`。打包脚本使用 release 构建，固定 linuxdeploy 版本并验证 SHA-256；冒烟检查在虚拟 X11 桌面启动解包后的程序，确认进程持续运行并生成正确的默认配置，不访问真实翻译接口。它不能代替真实桌面的快捷键、焦点、隐藏及唤出验证，也不是启动性能测试。

release AppImage 已通过构建和虚拟 X11 启动检查，并在 Wayland、Xwayland 路径验证窗口显示、默认配置生成及显式退出。包内携带配套 Compose 数据，避免旧版 xkbcommon 读取较新宿主系统的按键名时报错。GitHub 托管工作流尚未实际触发，其他发行版和输入法仍需验证。

## macOS DMG

[DMG 工作流](.github/workflows/dmg.yml) 支持在 GitHub Actions 中选择 **DMG → Run workflow** 手动构建，也会在推送 `v*` 标签时构建。两个架构分别在原生 macOS runner 上检查、构建和打包，成功后从该次运行的 **Artifacts** 下载对应压缩包：

| Mac 类型 | Artifact | 内含文件（以 1.0.0 为例） |
| --- | --- | --- |
| Apple Silicon（M 系列） | `Yeravich-macos-arm64` | `Yeravich-1.0.0-macos-arm64.dmg`、同名 `.sha256` |
| Intel | `Yeravich-macos-x86_64` | `Yeravich-1.0.0-macos-x86_64.dmg`、同名 `.sha256` |

在下载目录中用 `shasum -a 256 --check <文件名>.dmg.sha256` 校验，双击 DMG，把 `Yeravich.app` 拖到旁边的 **Applications**，再从应用程序目录打开。首次启动会在 `~/Library/Application Support/yeravich/config.toml` 自动写入 bilibili Index Translate 默认配置，保留已有配置；修改配置后需要显式退出再启动。DMG 不包含用户的配置或密钥，当前上传到 Actions artifacts，不自动创建 GitHub Release。

打包使用**固定自签名代码签名证书，尚未接入 Apple 公证**。每个版本及两个架构应使用同一证书和私钥；缺少签名身份会报错，不会静默回退到 ad-hoc。自签名不等于 Apple 对发布者的认证，下载后首次打开仍可能被 Gatekeeper 阻止。确认来源后可按 [Apple 的打开说明](https://support.apple.com/en-us/102445)，在系统设置的“隐私与安全性”中允许这次打开。签名也不代替辅助功能授权。

### 创建固定自签名证书

自签名不需要向 Apple 申请或购买开发者会员。只创建一次，后续版本复用；证书名称会随签名公开，可以使用项目名称。

1. 打开 macOS 的“钥匙串访问”，选择“证书助理 → 创建证书”。
2. 名称填写 `Yeravich Signing`，身份类型选择 **自签名根证书（Self Signed Root）**，证书类型选择 **代码签名（Code Signing）**。
3. 勾选“让我覆盖默认值”，设置适合长期维护的有效期（例如 3650 天）及 RSA 3072 或 4096 位密钥，将身份保存到登录钥匙串。
4. 在“我的证书”中找到该身份，确认可展开看到关联的私钥，再导出为带密码保护的 `.p12`。仅导出 `.cer` 不包含私钥，不能用于签名。
5. 将 `.p12` 和导出密码备份到自己的安全存储中。不要放进仓库、`private-docs/`、构建产物或日志；不要为每次构建重新生成证书。

可用 `security find-identity -p codesigning` 查看签名身份；自签名不在系统受信任身份列表中，因此这里不加仅列出有效受信任身份的 `-v`。签名时可以按证书名称选择，也可填写该身份的 SHA-1 指纹，后者用于区分同名证书。创建步骤参考 [Apple 证书助理说明](https://support.apple.com/guide/keychain-access/create-self-signed-certificates-kyca8916/mac) 和 [代码签名证书类型](https://developer.apple.com/library/archive/documentation/Security/Conceptual/CodeSigningGuide/Procedures/Procedures.html)。

### GitHub Actions 签名 Secrets

进入仓库 **Settings → Secrets and variables → Actions → Secrets → New repository secret**，添加以下两个 **Repository Secrets**，不要放到 Variables：

| Secret 名称 | 内容 |
| --- | --- |
| `MACOS_SIGNING_P12_BASE64` | 包含证书和私钥的 `.p12` 文件的 Base64 内容 |
| `MACOS_SIGNING_P12_PASSWORD` | 导出 `.p12` 时设置的密码 |

在 Mac 本机执行下面的命令，将编码结果直接复制到剪贴板，再粘贴到第一个 Secret 的值中；不要粘贴到聊天、终端输出或项目文档。路径仅为示例，替换成自己的安全存储位置。

```sh
base64 -i "$HOME/secure/yeravich-signing.p12" | pbcopy
```

Base64 是编码，不是加密。工作流从 `.p12` 自动识别唯一的代码签名身份，生成临时钥匙串密码，无需额外配置 Variables 或 Apple 账号。私钥仅导入 runner 的临时钥匙串并允许 `codesign` 使用，不修改系统根证书信任；打包后无论成功或失败都会执行清理，Artifacts 只上传 DMG 和校验和。参考 [GitHub 签名 Secrets 说明](https://docs.github.com/en/actions/how-tos/deploy/deploy-to-third-party-platforms/sign-xcode-applications)。

### 本地构建与签名

在 Mac 上本地打包，需要 Rust、Xcode Command Line Tools、Python 3 和 Homebrew 的 `librsvg`：

```sh
xcode-select --install  # 已安装时跳过
brew install librsvg
export MACOS_SIGNING_IDENTITY="Yeravich Signing"
bash scripts/build-dmg.sh
# 替换为上一步生成的单个文件路径；以下是 Apple Silicon 示例。
bash scripts/smoke-test-dmg.sh dist/Yeravich-1.0.0-macos-arm64.dmg
```

脚本默认使用当前 Mac 的架构，产物在 `dist/`；也接受 `aarch64-apple-darwin` 或 `x86_64-apple-darwin` 参数（需先安装对应 Rust target）。打包使用 release 构建，将程序、`Info.plist` 和由现有 SVG 生成的 `.icns` 图标组成标准 `.app`，再用系统 `hdiutil` 创建带 Applications 快捷方式的压缩 DMG。`MACOSX_DEPLOYMENT_TARGET` 默认设为 `11.0`，与 bundle 的最低系统版本声明保持一致；这不是对所有 macOS 版本的兼容性保证。

工作流会验证校验和、挂载后的 bundle 与签名、系统动态库依赖，再使用临时配置启动包内程序，确认进程持续运行且已有配置不被修改；不请求翻译服务。

自签名流程已在 Mac 使用临时证书验证：实际工作流中的身份导入、包签名与严格校验通过；两个不同版本的包具有不同代码哈希，但能满足相同的证书身份要求。缺少 Secret、非法 Base64、错误密码、缺少签名身份及请求 ad-hoc 的失败路径已检查，临时私钥和钥匙串已清理。该检查不代表跨版本 Accessibility 授权或 GitHub 托管工作流已通过实测。

固定自签名证书也已用于 Apple Silicon release DMG 的实际打包：镜像与 SHA-256 校验、包签名严格校验、包内证书与导入身份的一致性检查均通过；包内程序使用临时配置启动并持续运行 10 秒，配置未被修改。完成后临时钥匙串已删除，原有钥匙串搜索列表保持不变。此次检查未覆盖安装替换后的辅助功能授权或互联网下载后的 Gatekeeper 行为。

Apple Silicon macOS 的原有构建已通过格式、Clippy、编译及测试检查，并完成 arm64 release DMG 的构建、签名和启动验证。包中的应用已验证唤出、焦点、剪贴板自动翻译、关闭后常驻及显式退出。这些历史结果不代表新签名流程的所有分发行为均已验证。尚未进行性能测量或完整视觉检查；Intel Mac、其他 macOS 版本、Finder 拖拽安装和互联网下载后的 Gatekeeper 提示仍待验证。GitHub 托管工作流尚未实际触发。

将来切换 Developer ID 时，需要导入对应证书，并补齐 Hardened Runtime、安全时间戳、`notarytool` 公证和票据附加流程；仅替换 Secret 不会自动完成公证。自签名证书与 Developer ID 的代码身份不同，升级时可能需要用户重新授予辅助功能权限。当前 `MACOS_SIGNING_IDENTITY` / `MACOS_SIGNING_KEYCHAIN` 只控制选用哪个签名身份与钥匙串。

### macOS 权限与当前边界

当前 DMG 是非 App Sandbox 应用。权限要求取决于实际调用的系统 API，不是所有桌面能力都会在首次打开时集中弹框：

- 默认公网 HTTPS 翻译请求不需要通用的“允许联网”弹框。连接局域网翻译服务时可能触发“本地网络”授权；当前还未完成 Finder 启动下的局域网授权流程验证。Apple 还明确说明，从 Terminal 或 SSH 启动的命令行工具及其子进程有本地网络例外，因此 SSH 下的模拟请求测试不能代替这项验证。[本地网络隐私说明](https://developer.apple.com/documentation/technotes/tn3179-understanding-local-network-privacy)
- Option+Q 通过 `global-hotkey` 的 Carbon `RegisterEventHotKey` 注册，只接收这个组合键的激活事件；当前没有安装用于监听所有键盘输入的 event tap，不需要输入监控权限；选区读取的辅助功能授权与快捷键注册是两件事。
- Mac 的剪贴板兜底读取普通剪贴板。当前测试未出现剪贴板授权框，但不能据此断言所有系统配置都免授权：Apple 已提供程序化剪贴板访问的询问、允许、拒绝机制。是否提示取决于系统策略及访问方式；当前 Iced 接口只返回可选文本，尚不能把权限拒绝与无文本分开提示。[AppKit 剪贴板隐私说明](https://developer.apple.com/documentation/updates/appkit)
- 读取其他应用未复制的选区使用 Accessibility API，需要辅助功能授权。“选区权限”按钮触发系统授权引导并打开设置；程序不会自行授予权限。拒绝或撤销后仍可复制文本使用剪贴板兜底。截图/OCR 未实现，当前不需要屏幕录制权限。[辅助功能信任检查](https://developer.apple.com/documentation/applicationservices/1459186-axisprocesstrustedwithoptions)

启动后会读取一次剪贴板比较快照，不会因此发送翻译请求；从其他应用按快捷键唤出时，会读取文本并自动发送给配置的翻译服务。系统权限与发送行为是两件事，程序没有持续轮询剪贴板。

## 本地开发

项目使用 Rust 2024 edition，依赖版本由 [Cargo.lock](Cargo.lock) 记录。[mise.toml](mise.toml) 当前配置为使用 `latest` Rust 工具链；CI 使用的工具链和检查命令见 [ci.yml](.github/workflows/ci.yml)。

在具备图形桌面环境及所需系统库的环境中运行：

```sh
cargo run --locked
```

Ubuntu CI 安装了 `libwayland-dev` 和 `libxkbcommon-dev`。其他系统的环境准备应随平台验证补充。

评估运行时性能时先构建 release 版本，再单独启动可执行文件，避免把编译耗时计入启动耗时：

```sh
cargo build --release --locked
```

Linux 和 macOS 的可执行文件路径为 `target/release/yeravich`，Windows 为 `target/release/yeravich.exe`。

### Linux Wayland 桌面设置

需要提供 GlobalShortcuts 接口的 `xdg-desktop-portal` 及对应桌面后端。安装可执行文件与桌面入口后，Portal 才能识别 `com.zonowry.yeravich`：

```sh
install -Dm755 target/release/yeravich "$HOME/.local/bin/yeravich"
install -Dm644 data/com.zonowry.yeravich.desktop "${XDG_DATA_HOME:-$HOME/.local/share}/applications/com.zonowry.yeravich.desktop"
install -Dm644 data/com.zonowry.yeravich.svg "${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps/com.zonowry.yeravich.svg"
```

桌面会话的 `PATH` 需要包含 `~/.local/bin`，或将安装后的 `.desktop` 文件中的 `Exec` 改为可执行文件的绝对路径。调试时也可让 `Exec` 指向仓库内 `target/debug/yeravich` 的绝对路径，再使用 `cargo run --locked`。

Hyprland 需要把注册的动作绑定到按键。例如使用 `Alt+Q`，在 Lua 配置中添加以下内容；若已有 `Alt+Q` 绑定，请替换原绑定，避免同时触发其他操作：

```lua
hl.bind("ALT + Q", hl.dsp.global("com.zonowry.yeravich:show"))
```

保存后执行 `hyprctl reload config-only`。启动应用并完成快捷键注册后，即可用窗口关闭键（例如绑定了关闭窗口的 `Win+W`）收起，再用 `Alt+Q` 唤回，不需要先按一次 `Alt+Q`。界面不再常显注册成功提示；注册尚未完成时关闭窗口，会显示无法收起的原因并保持窗口可见。点击“退出”或按 `Ctrl+Q` 后，需重新启动应用才能继续使用唤出快捷键。

可用 `hyprctl globalshortcuts` 查看注册动作。旧 Hyprlang 配置的对应写法为 `bind = ALT, Q, global, com.zonowry.yeravich:show`。这里通知常驻进程，不执行新的 `yeravich` 程序。[Hyprland 全局快捷键说明](https://wiki.hypr.land/configuring/core/binds/globals/)

本地 Hyprland 0.56.2 验证中，显式退出后进程和 Portal 会话均已结束，但动作列表仍保留条目；不能仅凭该列表判断应用是否仍在运行。

### 代码检查

代码变更按 CI 约定检查：

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo check --workspace --all-targets --all-features
```

默认测试使用本地模拟 HTTP 服务，不访问真实翻译接口。需要单独验证配置时，可显式运行以下测试；它会向所选服务发送“你好，世界。”并打印译文：

```sh
YERAVICH_CONFIG="$PWD/config.example.toml" cargo test configured_service_smoke_test -- --ignored --nocapture
```

Linux 与 macOS 已通过使用示例配置的真实服务翻译检查。Linux Wayland 还已验证换行、Enter/数字键盘 Enter 提交、空输入与进行中重复提交保护，以及输入失焦时不提交。中文输入法候选词确认、Windows 及 macOS 的完整键盘交互仍需实机验证。

## 后续迭代方向

每次选择一个可以运行、验证和讲解的小功能推进。以下是当前建议的顺序，可随开发中的发现调整：

1. 验证更多来源应用的选区和剪贴板行为，按实际需求完善平台支持；学习外部输入与平台边界。
2. 根据实际使用完善翻译体验，例如取消底层请求或调整语言选择；学习异步任务生命周期。
3. 在其他目标平台验证翻译及常驻流程，测量并改善启动和唤出速度；学习条件编译、性能测量及取舍。

性能意识贯穿每一步。测量时分别记录进程启动到窗口可交互、快捷键触发到窗口可交互、提交翻译到显示结果的耗时，并记录系统、构建方式和测量条件。先建立基线，再确定数值目标。

## 阅读入口

- [Rust 阅读与实践指南](docs/rust-learning.md)：从现有代码理解数据流、所有权、消息和异步任务。
- [开发协作约定](AGENTS.md)：后续实现、注释、文档和验证遵循的项目要求。
