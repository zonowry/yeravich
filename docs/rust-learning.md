# 通过 Yeravich 学习 Rust

学习从一个具体问题开始：用户修改原文、发起翻译后，数据如何进入任务，结果又如何回到界面？沿着这条路径阅读现有代码，可以逐步理解模块、所有权、枚举和异步处理。

当前已经接入通过 TOML 配置的 Chat Completions 兼容服务。下面沿着配置加载、异步请求和界面状态更新介绍实际代码。

## 阅读顺序

| 文件 | 重点问题 | Rust 概念 |
| --- | --- | --- |
| [src/main.rs](../src/main.rs) | 程序如何组装并运行？ | 模块声明、函数返回值、条件编译 |
| [src/app/mod.rs](../src/app/mod.rs) | 谁持有应用状态，事件如何分发？ | `struct`、`enum`、`impl`、`match`、`&mut self` |
| [src/app/window.rs](../src/app/window.rs) | 窗口关闭后，哪些数据继续存在？ | 状态枚举、异步事件顺序 |
| [src/app/capture.rs](../src/app/capture.rs) | 唤出时如何带入文本，又如何防止覆盖后续输入？ | 任务句柄、所有权、版本检查 |
| [src/text_capture.rs](../src/text_capture.rs) | 不同平台如何取得外部文本？ | 条件编译、`Result`、任务组合 |
| [src/shortcut/mod.rs](../src/shortcut/mod.rs) | 系统快捷键如何进入应用？ | 资源所有权、条件编译、通道 |
| [src/app/translation_panel/mod.rs](../src/app/translation_panel/mod.rs) | 面板需要保存哪些数据？ | 字段所有权、`Option`、模块可见性 |
| [src/app/translation_panel/update.rs](../src/app/translation_panel/update.rs) | 编辑、翻译和返回结果如何改变状态？ | 模式匹配、所有权转移、闭包、异步任务 |
| [src/app/translation_panel/view.rs](../src/app/translation_panel/view.rs) | 状态如何变成界面？ | `&self`、返回类型中的生命周期 |
| [src/config.rs](../src/config.rs) | 如何定位文件、解析并校验配置？ | Serde、`Option`、`Result`、`?`、字段所有权 |
| [src/translation.rs](../src/translation.rs) | 翻译实现与界面如何衔接？ | `async fn`、借用、`Result`、错误枚举 |
| [src/i18n/mod.rs](../src/i18n/mod.rs) | 语言如何表示和转换？ | `Copy`、`Default`、`Display`、迭代与转换 |
| [build.rs](../build.rs) | 文案键如何在编译时生成？ | 构建脚本、集合、文件 I/O、生成代码 |

先读入口、面板状态和 `update`，再读视图。样式数值可以暂时跳过，避免打断对行为的理解。

## 状态由谁持有

`App` 持有界面语言和 `TranslationPanel`。面板持有原文、译文、翻译方向、请求版本、等待标记、可选请求错误，以及 `ServiceState`。后者区分 `Loading`、`Ready(Service)` 和 `Failed(Error)`，所以输入编辑不会清除配置加载错误。数据放在需要管理它的结构体里，因此可以从字段找到状态的负责人。

`view(&self)` 借用状态来构建界面；`update(&mut self, message)` 通过可变借用修改状态。阅读这两个签名时，先区分“查看已有数据”和“改变已有数据”。

`view` 返回的 `Element<'_, Message>` 带有生命周期标记。这里的 `'_` 让编译器推断生命周期；返回的界面元素可能借用 `self` 中的内容，因此不能比被借用的数据活得更久。遇到生命周期问题时，先问数据由谁持有、谁借用了它、借用需要持续多久。

## 用消息表达行为

面板的 `Message` 枚举包含 `Clear`、`SwapLanguages`、`Translate` 等行为，也包含带数据的 `Translated { revision, result }`。一个消息的类型就说明它代表什么，以及处理它需要哪些数据。

`update` 使用 `match` 处理各个变体。新增消息时，穷尽匹配会帮助检查处理分支是否齐全。多数分支只改变状态并返回 `Task::none()`；复制或翻译等分支则返回需要执行的任务。

面板消息还有一层应用消息包装：`Message::Panel(...)`。应用通过 `self.panel.update(message).map(Message::Panel)` 将面板任务的结果转换为应用消息，让面板管理自己的行为。

原文编辑器在 [view.rs](../src/app/translation_panel/view.rs) 用 `.key_binding(source_key_binding)` 绑定按键。`source_key_binding` 把 `KeyPress` 的所有权交给 Iced 的 `Binding::from_key_press`，获得 `Option<Binding<Message>>`；未聚焦时为 `None`。函数提前保存修饰键是否为空，再通过带条件的 `match` 将无修饰键的 `Binding::Enter` 转为 `Binding::Custom(Message::Translate)`；Shift+Enter、复制、粘贴和光标移动沿用默认绑定。

键盘与按钮都发送 `Message::Translate`，统一经过 `can_translate` 检查配置、空白输入和进行中的请求。Enter 不经过 `InputAction`，不会插入换行或使正在等待的结果失效。对应的按键与状态边界见 [面板测试](../src/app/translation_panel/tests.rs)。Hyprland Wayland 上已用本地模拟服务验证换行、提交、焦点与数字键盘 Enter；中文输入法候选词确认及其他平台仍需验证。

## 多行编辑器的状态与可见空间

`TranslationPanel` 持有 `text_editor::Content`，视图中的 `text_editor(&self.input)` 只借用它；Shift+Enter 和粘贴产生的 `InputAction` 在 `update` 中通过 `perform` 修改内容。换行同样属于编辑，会让旧翻译结果失效。

这次布局解决的问题是：上下堆叠的原文不断增高，把译文挤到整页滚动区外。[面板视图](../src/app/translation_panel/view.rs) 改用 `row![source_editor, translation_output]`，两个面板的宽度都为 `Fill`，因此等分可用宽度。应用列、面板列、文本行、面板容器和编辑器逐层设置 `.height(Fill)`，共同把工具栏和底部操作之外的剩余高度分配给文本。只给最内层编辑器设置 `Fill`，不能保证外层也为它保留空间。

不再按文本行数调整面板高度，也不再把两个面板一起放进外层 `scrollable`。自动折行和内部滚动由 Iced `TextEditor` 处理，缩小窗口或输入长文时两栏仍同时可见。底部配置/请求错误使用单独的限高滚动区，以免长路径占满界面。

[widgets::editor](../src/widgets.rs) 只统一字号、行高、折行和外观。它接收 `TextEditor<'a, H, Message>` 的所有权，配置后转成 `Element<'a, Message>`；内部仍借用面板持有的 `Content`，没有复制文本状态。泛型 `H: text::Highlighter` 沿用传入编辑器的类型；调用方先设置 `id` 和消息处理，再把编辑器移入辅助函数。这里不再需要自定义 `Widget`、测量缓存和 Iced 的 `advanced` feature，也没有新增依赖；未测量启动、内存或体积变化。

常显文案与按需提示也由视图决定：`App::header` 复用 `TranslationPanel::direction`，通过 `.map(Message::Panel)` 把面板消息接回应用；`widgets::hint` 用悬停提示容纳按键和退出说明。`ServiceState`、`translating` 和 `error` 继续决定操作是否可用以及显示什么反馈，隐藏“就绪”文案不会改变翻译状态机。

布局数值不需要新增机械单元测试；原有状态和键盘测试继续验证行为边界。桌面检查应覆盖最小窗口、多行文本、复制、方向切换、中文翻译与错误反馈，当前验证范围见本文的[验证范围](#验证范围)。

## 让异步任务持有输入

发起翻译时，代码把 `self.input.text()` 得到的 `String` 移入 `async move`，再传给 `Service::translate`。`String` 拥有文本数据；这份文本进入异步任务后，编辑器仍然可以继续处理用户输入，任务使用的是发起时取得的文本。

任务还持有一个 `Service` 副本：其中配置是只读快照，`reqwest::Client` 的克隆共享原有连接池。任务需要在 `update` 返回后继续运行，不能借用面板里的服务；这里的克隆用于同时保留面板状态和任务所需的数据，没有引入共享可变配置。请求体 `ChatRequest<'a>` 则只在函数内部借用模型名和提示词，序列化后无需复制完整字符串到另一个结构体。

任务通过 `Task::perform` 返回结果，由 `move |result| Message::Translated { revision, result }` 转成消息。`move` 让闭包按值捕获需要的变量；这里的 `revision` 是实现了 `Copy` 的 `u64`，因此捕获的是版本号副本。

`async fn` 调用产生 future，具体执行由任务机制驱动。Iced 开启 `tokio` feature，使 `Task::perform` 中的 `reqwest` 网络请求拥有需要的 Tokio 执行环境。`async` 本身不会让阻塞操作自动变成非阻塞操作；这里文件读取使用 `tokio::fs::read_to_string`，由后台线程执行文件 I/O，HTTP 请求通过异步客户端等待网络事件。

## 快捷键带入文本与取消任务

阅读路径是 `App::update(Event::Activated)` → [show_with_text](../src/app/capture.rs) → [text_capture::read](../src/text_capture.rs) → `Message::TextCaptured` → `TranslationPanel::import_and_translate` → `Message::Translate`。只有从应用外唤出才开始读取；窗口已聚焦时保留当前编辑内容。`Task::batch` 同时驱动窗口显示与读取，外部文本不会阻塞 Wayland 窗口重建。

Linux 读取 PRIMARY selection 和普通剪贴板，Windows 读取普通剪贴板，macOS 从来源应用读取 AX 选区并以普通剪贴板兜底。`usable_text` 接收 `Option<String>` 的所有权，借用 `trim()` 判断是否全为空白，但返回原来的 `String`，保留用户的换行和缩进。`Snapshot` 分别持有两个 `Result<String, Error>`，这样一个来源失败时另一个仍可使用。任务把快照所有权交给消息；`text_captured` 通过 `snapshot.text(source)` 临时借用 `&str` 传给面板，再把整个快照移入 `App` 作为下次比较的依据，无需克隆正文。Iced 的原生剪贴板任务使用 `Option::take()` 把第一次读取移入最终快照，适应任务回调的 `FnMut` 签名。

旧实现只要 PRIMARY 非空就返回，CLI 只更新普通剪贴板时便会反复取得旧选区、复用旧译文。Linux 的 `Snapshot::select` 比较两次采样：有效内容变化的来源优先，两处都变化时 PRIMARY 优先；都未变化时保留上次来源，没有上次来源才按 PRIMARY、剪贴板的顺序兜底。首次窗口就绪时用 `CaptureBaseline` 保存比较快照，不提交翻译；`capture_revision == 0` 保证迟到的启动快照不能改写热键读取之后的状态。这里比较的是文本变化，不是剪贴板事件的时间戳，也不等于持续监听剪贴板。

Wayland 最后一个窗口关闭后，Iced 的剪贴板连接也会断开。[wayland::read_selection](../src/text_capture/wayland.rs) 因此通过 `tokio::process::Command` 异步运行 `wl-paste`，读取不依赖窗口焦点与重建进度。固定参数分别传给 `args`，不使用 shell；`--type text` 只请求文本，`--no-newline` 避免工具追加换行。每次读取用 `timeout` 限制在 500 毫秒内，并设置 `kill_on_drop(true)`，以便超时或任务被取消时结束子进程。这只在 Linux 编译，需安装 `wl-clipboard`；其他平台复用 Iced，不新增剪贴板 crate。当前没有测量这些选择对体积或耗时的影响。

`App` 的 `capture: Option<Capture>` 保存请求版本和可选的 `iced::task::Handle`。没有句柄表示还在等待焦点：使用 Iced 原生剪贴板时，`read_pending_capture` 等实际 `WindowFocused` 消息后才读取，不能把发出聚焦请求当成已经获得焦点；Wayland 的独立读取不需要等待。句柄存在后，重复焦点事件不会启动第二次读取。

开始新读取前丢弃旧句柄；因为句柄设置了 `abort_on_drop()`，编辑、提交、收起或退出时 `cancel_capture` 把 `capture` 设为 `None` 就会请求取消。这里利用所有权释放任务资源，无需 `Arc<Mutex<_>>`。已入队的消息仍可能在取消后到达，因此接收时还要比较版本；“取消底层工作”和“拒绝过期结果”分别处理。Wayland 取消能结束读取子进程；Iced 已发出的原生剪贴板操作无法靠丢弃句柄撤回，但其过期结果不会更新面板。

`import_and_translate` 先借用文本调用 `import_text`：只有文本非空且不同于原文时才替换编辑器，并复用 `invalidate_translation` 清除旧译文、使旧翻译结果失效。之后自动走已有的 `Message::Translate` 分支，沿用配置就绪、空输入和请求进行中的检查，不改变翻译方向。同样的文本已有译文时直接复用；上一次翻译失败时再次捕获可重试。两个来源均不可用时，界面保留原文并显示本地化提示。

`Snapshot` 手写 `Debug`，只展示两个来源的长度或错误，避免调试消息时展开剪贴板正文。关键节点用 `eprintln!` 向标准错误（stderr）输出捕获版本、来源选择和翻译版本，网络耗时用 `Instant` 测量；应用和打包检查脚本都不写日志文件，无需轮转或清理。这些诊断日志可用于关联事件，不代替 release 性能测量，也没有引入新的日志依赖。

这里 `text_captured` 返回 `Task<Message>`，通过 `.map(Message::Panel)` 把面板翻译任务交回应用事件循环；只更新状态而丢弃返回任务，会造成界面显示“翻译中”却没有实际网络请求。`import_and_translate` 用 `pub(in crate::app)` 把入口限制在应用模块内，纯导入辅助函数继续保持私有。`auto_translate_pending: bool` 只记录配置加载期间一次已捕获文本的自动提交意图，`ServiceLoaded` 消费它后再调用同一翻译分支。编辑、清空或切换方向会经 `invalidate_translation` 清除该标记，避免配置就绪后把用户修改过的内容自动发出；普通手动输入不会设置它。

[应用测试](../src/app/tests.rs) 和 [面板测试](../src/app/translation_panel/tests.rs) 检查聚焦时不覆盖、重复唤出、编辑或提交后拒绝旧结果、失败保留内容以及旧翻译失效。[Wayland 测试](../src/text_capture/wayland/tests.rs) 使用临时可执行文件模拟选区、空白、非法 UTF-8、工具缺失和卡住的读取，不接触真实剪贴板；超时测试还确认子进程确实结束。这些测试不能替代真实桌面上的选区、焦点和唤出验证。

关于超时取消的细节可查 [Tokio Command::kill_on_drop](https://docs.rs/tokio/latest/tokio/process/struct.Command.html#method.kill_on_drop)。

## macOS 选区、权限与 FFI 所有权

入口在 [`macos.rs`](../src/text_capture/macos.rs)。`App::show_with_text` 在 `show_window` 前通过 `NSWorkspace` 取得来源 PID，保存在 `Capture::source_pid: Option<i32>`。只把整数移入后台任务，不把 AppKit 对象或借用的指针跨线程传递。后台通过 `AXUIElement::new_application(pid)` 访问固定的来源，再取 `AXFocusedUIElement` 的 `AXSelectedText`；不会在 Yeravich 获得焦点后查询系统当前的焦点控件，也不会退而读取整个控件的 `AXValue`。

AX 属性读取会与其他进程通信，因此放在 `tokio::task::spawn_blocking`，并给应用对象和焦点元素分别设置 250 毫秒消息超时。丢弃 Iced 的任务句柄不能强行终止已开始的阻塞 AX 调用；超时限制等待，捕获版本检查拒绝迟到结果。这是“取消等待”和“取消底层工作”必须区分的另一个例子。

`attribute` 把 `&mut value` 转成有效的 `NonNull` 输出槽交给 C API；只有返回成功且指针非空，才用 `CFRetained::from_raw` 接管 Copy API 交出的 +1 引用。包装值离开作用域会释放引用。`downcast_ref` 检查 Core Foundation 类型 ID 后才把返回值借用为 `AXUIElement` 或 `CFString`，避免把其他类型当作字符串。少量 `unsafe` 局限在这些 FFI 边界，并在注释里说明指针有效性和所有权依据。

`macos::read` 将拥有的 `Result<String, Error>` 移入剪贴板读取回调。回调要求 `FnMut`，所以用 `mem::replace` 将结果移出并留下 `NoText`，无需克隆选区正文。最终 `Snapshot.primary` 在 Mac 表示本次真实 AX 选区，因此只要有效就优先使用；Linux 的 PRIMARY 可能残留旧内容，继续采用变更优先规则。

`permission(false)` 只检查信任状态；用户点击“选区权限”才调用带 `kAXTrustedCheckOptionPrompt` 的检查并打开系统设置。返回 `false` 不表示用户已经拒绝，也不表示授权弹框已经处理完，重新聚焦或唤出时再次查询。`AccessibilityRequired`、`SelectionUnavailable` 与 `NoText` 分别表达缺少授权、来源不支持/失败和没有选区；即使剪贴板兜底成功，也保留必要的选区失败说明。权限检查不代替输入版本检查，旧读取不能因随后授权而重新生效。

这里的 `Result` 也帮助定位部署问题：`selected_text` 在 `trusted()` 为 `false` 时就返回 `AccessibilityRequired`，此时尚未查询来源应用的焦点元素。即使权限开关已开启，TCC 也可能因旧签名记录拒绝新构建，不能把它当作 `AXSelectedText` 不受支持，更不能把剪贴板兜底成功当作权限已生效。先核对实际运行的 `.app` 和系统授权，再用选区 A、剪贴板 B 不同的测试检查 `Snapshot.primary` 与 `selected=Primary`。重建 ad-hoc 签名应用会改变代码身份，替换已授权的二进制可能让前一次授权失效；处理步骤见 [中文 README](../README.zh-CN.md#macos)。

仅在 macOS 引入 `objc2-application-services`，它提供系统 AX 的 Rust 绑定；直接声明已在依赖树中的 `objc2-app-kit`、`objc2-foundation` 和 `objc2-core-foundation`，只开启用到的接口。没有增加外部命令或读取服务，最终仍链接 macOS 自带框架；这些依赖增加构建与 FFI 维护成本，未测量体积、启动或内存变化。参考 [Apple AX 超时说明](https://developer.apple.com/documentation/applicationservices/1459345-axuielementsetmessagingtimeout) 和 [objc2 AX 绑定](https://docs.rs/objc2-application-services/0.3.2/objc2_application_services/)。

测试覆盖 Unicode/空白保留、错误的 CF 类型、权限/空选区/通信失败分类、Mac 选区优先、剪贴板兜底提示和过期结果。实机需将选区 A 与剪贴板 B 区分，并分别用 SSH 和普通 `.app` 启动验证，避免继承的开发环境信任掩盖用户需要的授权步骤。

## 配置从文件进入服务

`App::boot` 用 `Task::batch` 同时发起窗口打开和 `App::load_configuration()`，因此不会等配置读取完成才开窗口。后台任务只读取一次配置，得到包含 `TranslationConfig` 和 `shortcut::Binding` 的 `Config`，再创建网络客户端，通过 `ConfigurationLoaded` 把两个结果移交应用。`App::update` 将服务转为面板的 `ServiceLoaded` 消息，并保存待注册的快捷键。加载成功前不允许提交翻译，也不会访问远端接口；配置错误时仍可通过托盘恢复窗口，托盘也不可用时才禁止收起。

`config::load` 先读取 `YERAVICH_CONFIG`，否则使用系统配置目录。`load_with_paths` 区分默认路径与显式指定的路径：仅默认路径缺失时创建配置；读取失败、语法错误和自定义路径缺失都返回错误。这个函数接收路径参数，测试无需修改进程全局环境变量，可以并行执行。

默认内容通过 `include_str!("../config.example.toml")` 在编译时成为 `&'static str`，所以 AppImage 不需要在运行时寻找仓库文件。`create_default(&Path)` 先在目标目录创建 `NamedTempFile`，完整写入后交给 `persist_noclobber` 保存；它消费临时文件的所有权。目标已存在时保留原文件，再由 `load_from` 读取。这既保护用户的配置，也避免同时启动的进程读到另一个进程尚未写完的默认内容。其他创建错误转为 `Error::Create(PathBuf)`，界面提示权限、磁盘空间及替代配置路径。

临时文件 API 是阻塞的，因此通过 `tokio::task::spawn_blocking` 执行。闭包需要满足 `'static`，不能借用当前函数栈上的 `&Path`；代码把一份 `PathBuf` 移入 `move` 闭包，同时让外层继续持有原路径，用于错误提示和重新读取。这里的 `clone` 有明确的双重持有需求。等待结果产生两层 `Result`：外层表示后台任务是否正常结束，内层表示文件操作是否成功，分别使用 `map_err(...)?` 处理。

`load_from` 读取文件后交给 `parse`：Serde 将 TOML 转成拥有 `String` 的 `FileConfig`，然后校验快捷键、URL、模型名和可选 API Key，生成 `Config`。`deny_unknown_fields` 让拼错的配置项成为错误，避免被静默忽略。相关边界在 [配置测试](../src/config/tests.rs) 中覆盖，包括首次创建、保留有效/无效文件、显式路径缺失、并发初始化及创建失败。

API Key 从 `Option<String>` 经 `as_deref()` 暂时借用为 `Option<&str>`，空白值过滤为 `None`。构建请求头得到 `Option<Result<HeaderValue, _>>`，`transpose()` 把它转换成 `Result<Option<HeaderValue>, _>`，就可以用 `?` 提前返回非法请求头错误。请求头标为敏感值，配置不派生 `Debug`，界面也不显示可能包含密钥的 TOML 原始错误或服务端错误正文。

初版不监听文件变化。服务及其配置由常驻进程持有，修改后需显式退出并重新启动；仅关闭窗口不会重新加载。运行步骤见 [中文 README](../README.zh-CN.md#修改配置)。

新增依赖中，`reqwest` 负责 HTTP，关闭默认 feature，仅开启 JSON 与 rustls TLS；`serde`/`toml` 负责数据格式，`dirs` 统一 Windows、macOS 和 Linux 的配置目录，`tokio` 提供网络运行时和后台文件读取。`tempfile` 从测试依赖移到运行依赖，用于跨平台的临时文件清理和不覆盖目标的保存操作，没有新增锁文件中的版本；`serde_json` 仍用于测试和构建脚本。它们增加了依赖、构建和维护成本；当前未测量依赖带来的二进制体积、启动耗时和常驻内存变化。平台验证范围见 README，编译通过不能代替桌面实机验证。相关机制可参考 [Reqwest 客户端](https://docs.rs/reqwest/0.12.28/reqwest/struct.ClientBuilder.html)、[Tokio 文件 I/O](https://docs.rs/tokio/1.48.0/tokio/fs/index.html) 和 [NamedTempFile::persist_noclobber](https://docs.rs/tempfile/latest/tempfile/struct.NamedTempFile.html#method.persist_noclobber)。

AppImage 的边界位于 [打包脚本](../scripts/build-appimage.sh)：Rust 代码继续负责配置行为，脚本负责组装二进制、运行库、图标与桌面入口。`ldd` 只能发现链接时记录的依赖，winit 运行时用 `dlopen` 加载的 X11/Wayland 库需要显式列出。linuxdeploy 仅参与 Linux 打包，不成为 Rust 的运行依赖。工作流先做原有代码检查，再构建 release 产物并执行包内程序的启动检查。

macOS 的同一条边界位于 [DMG 打包脚本](../scripts/build-dmg.sh) 和 [Info.plist 模板](../data/macos/Info.plist)：`cargo build --release --target ...` 产生对应架构的可执行文件，脚本再把它放进 `Yeravich.app/Contents/MacOS`。`CFBundleExecutable` 必须与文件名一致，`CFBundleIdentifier` 使用现有的 `com.zonowry.yeravich`；版本来自 Cargo 元数据，最低系统版本与 `MACOSX_DEPLOYMENT_TARGET` 一致。`.dmg` 是承载 `.app` 与 Applications 快捷方式的磁盘镜像，配置仍由 `config::load` 在用户目录管理。

这次没有增加 Rust 运行依赖。`librsvg` 只在打包时把现有 SVG 渲染为不同尺寸的 PNG，macOS 的 `iconutil` 再合成 `.icns`；`codesign` 和 `hdiutil` 分别负责签名与镜像生成。[sign-macos-app.sh](../scripts/sign-macos-app.sh) 使用显式指定的固定证书签名，并检查包完整性及签名中确实包含证书；缺少身份时失败，不回退到 ad-hoc。证书签名不等于 Apple 公证。

签名身份属于发布流程，不应进入 Rust 应用的配置或二进制。CI 从两个 GitHub Secrets 中读取 `.p12` 和密码，将身份导入临时钥匙串，自动解析唯一证书的指纹，再通过环境变量交给打包脚本；应用运行时不持有签名私钥。自签名证书不受系统默认信任，导入阶段不能用 `security find-identity -v` 来判断它是否存在。持续复用证书、应用标识符和兼容的 designated requirement，可以建立版本间的代码身份；是否保留 Accessibility 权限仍须实际升级验证，不能由签名校验推断。证书创建、Secrets 配置及公证边界见本文的[构建与验证](#构建与验证)。

`aarch64-apple-darwin` 与 `x86_64-apple-darwin` 会让 `#[cfg(target_os = "linux")]` 分支在编译时被排除。例如 [shortcut/mod.rs](../src/shortcut/mod.rs) 的 Portal 注册和 [text_capture.rs](../src/text_capture.rs) 的 `wl-paste` 调用不会进入 Mac 程序，Mac 使用原生快捷键、Accessibility 选区和 Iced 剪贴板兜底。因此 DMG 不需要 Linux 的运行库或外部剪贴板工具；两个原生 runner 分别执行检查，可以暴露仅在对应平台出现的编译问题。

平台边界验证应让来源窗口选中 A、剪贴板持有 B，再触发快捷键：已授权时应读取未复制的 A，未授权时应使用 B 并提示授权。通用函数名不意味着所有平台读取方式相同，必须用来源不同的文本验证实际取值。打包脚本还要求把 `lipo` 的输入文件放在 `-verify_arch` 的架构列表之前；这类原生工具行为需在目标系统实际执行，ShellCheck 或 Linux 上的语法检查无法覆盖。验证范围见本文的[验证范围](#验证范围)。

## 为什么要检查请求版本

用户在请求返回之前可能清空原文、修改原文或切换方向。如果直接显示旧结果，译文就会与当前输入不对应。

`invalidate_translation` 增加 `revision` 并清理旧的输出状态。请求携带发起时的版本号；结果返回时，只有版本仍一致且面板还在等待翻译，才接收它。

可以按以下事件顺序理解：

1. 请求 A 发起，记录当前版本。
2. 用户修改输入，面板版本增加。
3. 请求 A 返回，版本不一致，因此忽略结果。

这是结果有效性检查。底层请求是否仍在运行是另一个问题：当前版本检查并没有取消它，请求仍可能在后台运行至完成或 60 秒超时。超时会结束网络等待，版本检查则决定对应结果能否更新界面。

## 用类型区分成功和失败

`Service::translate` 返回 `Result<String, translation::Error>`：`Ok` 包含译文，`Err` 包含失败原因。面板通过匹配结果决定更新输出还是保存错误。

面板的 `error: Option<translation::Error>` 表达另一层含义：当前可能有错误，也可能没有。`Option` 处理“有或无”，`Result` 处理一次操作的“成功或失败”。

`Error` 区分配置、客户端初始化、超时、网络、HTTP 状态、响应格式和空译文。`map_err` 将底层错误转换为这些可处理的类别，`?` 在失败时提前返回，不以 panic 结束进程。服务加载失败时按钮保持禁用；一次请求失败则保留服务，用户可以重试。错误在 `view` 时按当前界面语言转换为文案，切换语言不会丢失错误状态。

## 从现有测试验证理解

[面板测试](../src/app/translation_panel/tests.rs) 直接构造状态并发送消息，不需要先操作窗口。它们检查了切换界面语言保留文本和翻译方向、译文只读，以及清空或切换方向后拒绝旧结果等行为。

适合随功能推进的练习是：先用自己的话预测消息处理后的状态，再阅读对应测试和 `update` 验证预测。修改状态逻辑时，为真正需要保证的行为补充测试。

现有测试覆盖编辑、清空、切换方向后拒绝旧结果，以及重试期间忽略旧错误。可以练习先预测“请求 A 失败、重试 B、A 的迟到消息到达”时的版本号和等待状态，再读测试验证。

[配置测试](../src/config/tests.rs) 覆盖示例文件、路径优先级、可选鉴权、非法字段和读取失败。[HTTP 测试](../src/translation/tests.rs) 使用本地 TCP 服务捕获真实请求，验证地址、方向、JSON 和鉴权，以及 HTTP 错误、空内容、格式错误、连接失败和正文读取超时；CI 不依赖真实服务。实际桌面仍需验证输入、等待、译文显示、配置错误提示，以及进行中的请求不会阻塞窗口操作。

## 进程常驻与窗口生命周期

入口使用 [`iced::daemon`](https://docs.rs/iced/0.14.0/iced/daemon/fn.daemon.html)，没有窗口时也继续运行。`App::boot` 返回应用状态和打开窗口的任务；后续关闭窗口不会销毁 `App`，其中的 `TranslationPanel` 因此保留原文、译文及进行中的翻译状态。

`App::subscription` 通过 `event::listen_with(App::keyboard_shortcut)` 监听窗口按键。`keyboard_shortcut` 用 `let ... else` 只取按下事件，再借助 `key.as_ref()` 匹配按键而不复制字符数据。未被控件消费、没有修饰键的 Esc 返回 `Some(Message::CloseRequested(window))`，把事件携带的窗口 ID 交给既有 `hide_window`；不相关事件返回 `None`。因此 Esc 复用关闭保护及平台策略，不会移走快捷键注册，也不会销毁 `App`。Ctrl+Q / Cmd+Q 则继续产生 `Message::Exit`，负责真正退出。按键测试覆盖两种窗口策略、不可唤回时的关闭保护，以及被消费或带修饰键的 Esc。

`WindowState` 明确区分 `Absent`、`Opening`、`Visible`、`Hidden` 和 `Closing`。这是对实际生命周期的建模：`Opening` 提前保存窗口 ID，避免重复热键创建多个窗口；`Closing { reopen: true, .. }` 记录关闭期间收到的唤出请求，等旧窗口的 `Closed` 事件到达后再创建新窗口。

平台差异由实际窗口句柄决定。Windows、macOS、X11 可以隐藏并复用原窗口；Wayland 路径销毁窗口后再创建，但保持 `App` 的所有权不变。保留应用状态和保留图形资源是两个层次，当前 Iced 在最后一个窗口销毁后会释放 compositor，所以 Wayland 下还不能复用所有图形资源。

## 将配置变成可注册的快捷键

[`Binding`](../src/shortcut/binding.rs) 包装现有 `global-hotkey` 的 `HotKey`。私有字段让调用方只能通过 `parse` 或 `Default` 构造它：默认值是 Alt+Q，解析时校验修饰键和主键，并保留应用的退出组合键。这个值只有键码和修饰键等小数据，派生 `Copy` 后可在注册和显示之间按值传递，无需共享可变状态。本次配置功能复用现有依赖，没有新增 crate。

Serde 的 `#[serde(default)]` 让旧文件缺少 `[shortcuts]` 时得到空的 `RawShortcuts`。`show_window.as_deref()` 借用可选字符串，`map(Binding::parse).transpose()?` 区分未填写与填写错误；只有未填写时才用 `unwrap_or_default()`，因此拼错按键不会被默认值掩盖。[配置测试](../src/config/tests.rs) 还验证旧文件字节不变，升级不会重写用户配置。

窗口与配置异步完成，顺序不确定。`App` 用 `pending_shortcut: Option<Binding>` 暂存配置；`WindowReady` 和 `ConfigurationLoaded` 都调用 `register_shortcut`，只有窗口就绪、配置已到且尚未注册时才执行。`take()` 消费待注册值，保证后续唤回窗口不会重复注册。这里把读取文件和创建客户端放在后台，而原生注册仍在窗口事件线程完成；这两个异步边界不能混为一谈。[应用测试](../src/app/tests.rs) 覆盖两种完成顺序、错误时的关闭保护以及退出后忽略迟到配置。

`Binding::description` 生成界面名称（Mac 显示 Option），`preferred_trigger` 生成 Portal 使用的 XDG 字符串，例如 `Alt+Q` 对应 `ALT+q`，Super 对应 `LOGO`，Enter 对应 `Return`。这是两种协议的格式转换，不是把一个显示字符串直接交给所有平台。Portal 仅接收建议，Hyprland 的真实绑定仍由桌面配置决定。

验证快捷键时，先确认旧配置采用默认值，再检查自定义值替代默认注册。格式错误与系统无法注册是两条错误路径：前者在 `Binding::parse` 返回 `Err`，后者由原生后端返回错误，均应保留窗口。macOS 的非独占注册允许多个应用使用同一按键，因此不能用另开一个同配置进程可靠模拟注册失败；应检查实际的系统返回值。

## 快捷键注册由谁持有

原生路径在 [native.rs](../src/shortcut/native.rs) 中创建 `GlobalHotKeyManager`，由 `App` 间接持有 `Registration`。注册必须在窗口事件循环所在的线程进行，因此从 `update` 调用。系统回调通过 `mpsc` 通道发送 `Activated`，`Task::run` 把接收到的事件转成应用消息；空闲时等待消息，不反复检查队列。

`Registration` 实现 `Drop`，离开作用域时注销热键。应用退出时用 `Option::take()` 移走注册对象，既明确转移所有权，也避免后续重复使用。这是利用所有权控制系统资源生命周期的一个实际例子。

Wayland 会话使用 [portal.rs](../src/shortcut/portal.rs) 中的异步任务持有 Portal 会话。`select!` 等待激活、绑定变化、服务断开或退出通知；`oneshot` 通道只负责发送一次退出通知。收到退出通知后先关闭会话，再发送 `Stopped`，应用等待快捷键和托盘均确认停止后结束进程。等待清理期间不再处理新的唤出请求。

`ShortcutStatus::Ready(String)` 的枚举变体表示注册成功，其中的 `String` 只用于展示按键描述。[`App::update`](../src/app/mod.rs) 收到 `Event::Ready` 就进入这个状态；空字符串仍然是成功，不能把缺少描述推断成不可用。[Hyprland 的 Portal 会返回空描述](https://github.com/hyprwm/xdg-desktop-portal-hyprland/issues/312)，过去将它设为 `NeedsBinding` 会使 `hide_window` 拦截 `Win+W`，直到用户先触发一次唤出。现在关闭判断只区分注册状态，后续空描述更新也不会再把窗口恢复或阻止关闭；真正的注册或监听错误仍由 `Failed` 表达。

新增依赖各有明确职责：`global-hotkey` 封装 Windows、macOS 和 X11 的原生注册；仅在 Linux 编译的 `ashpd` 封装 Portal，关闭默认功能并只开启 `async-io` 和 `global_shortcuts`。它们增加了平台适配与依赖维护成本，当前尚未测量对二进制体积、启动速度和常驻内存的影响。

[窗口状态测试](../src/app/tests.rs) 覆盖重复唤出、关闭与唤出的交错、快捷键和托盘均不可用时保持窗口可见，以及退出期间忽略新的激活事件。[面板测试](../src/app/translation_panel/tests.rs) 验证两个窗口策略均保留文本与翻译方向。桌面验证还要确认实际窗口消失、恢复后 PID 不变、焦点行为和显式退出，不能用状态测试替代。

## 托盘的所有权与异步边界

阅读入口是 [tray/mod.rs](../src/tray/mod.rs)、[Linux 后端](../src/tray/linux.rs)、[原生后端](../src/tray/native.rs) 和 `App::update`。`App::boot` 在窗口打开任务后发送 `StartTray`，确保原生托盘在已运行的窗口事件线程创建。`App` 持有 `Option<tray::Registration>`，收起窗口不会释放它；关闭窗口与移除托盘是两个独立的生命周期。

Windows/macOS 的图标和菜单不是 `Send`，因此不能移入 `Task::perform`；`native::Registration` 留在 `App` 中，菜单回调仅把 `Show` / `Exit` 发进 `mpsc` 通道。`Task::run` 将事件送回 Iced，空闲时等待事件。Linux 的 `ksni::Tray::spawn` 在异步任务中连接 D-Bus，初始化等待与 `oneshot` 退出信号竞争；语言更新也通过通道送给异步任务，避免在界面更新中等待 D-Bus。

`Event::Show` 调用既有 `show_window`，不走快捷键的 `show_with_text`，并取消尚未完成的文本捕获，因此托盘恢复不会覆盖原文。`capture_baseline_started` 单独记录启动快照是否已经发起，不能用“快捷键是否已注册”代替：配置失败时注册为空，但 Wayland 重建窗口也不应该再次读取剪贴板。`can_hide` 统一判断快捷键和托盘是否至少有一个可用；Linux 的 `watcher_offline` / `watcher_online` 将面板离线和恢复转换为状态变化。只有最后一个恢复入口失效才自动打开窗口。

退出时 `Option::take()` 移走两项注册资源，再各自调用 `stop`。原生图标随 `Drop` 移除；Linux 先等待 `handle.shutdown().await`。`shortcut_stopped` 和 `tray_stopped` 记录两个独立的确认，任意到达顺序都必须等齐，避免 Portal 关闭时托盘还留在后台，或托盘先消失就跳过快捷键清理。退出期间迟到的托盘点击、初始化和语言消息都被忽略。[应用测试](../src/app/tests.rs) 覆盖仅托盘可用、两个恢复入口依次失效、Wayland 关闭期间恢复，以及退出确认顺序。

`data/tray-icon.rgba` 是现有 `data/com.zonowry.yeravich.svg` 的 32×32 RGBA 导出，使用 `include_bytes!` 嵌入。Linux 转为协议要求的 ARGB 字节序，Windows/macOS 直接传给 `Icon::from_rgba`，不引入运行时图片解码依赖。更新 SVG 后可用 `rsvg-convert -w 32 -h 32 data/com.zonowry.yeravich.svg -o /tmp/yeravich-tray.png`，再用 ImageMagick 的 `magick /tmp/yeravich-tray.png -depth 8 rgba:data/tray-icon.rgba` 重新导出。

依赖边界见 [ksni 的异步接口](https://docs.rs/ksni/0.3.7/ksni/) 与 [tray-icon 的平台要求](https://docs.rs/tray-icon/0.26.1/tray_icon/)。仅 Linux 引入 `ksni`；仅 Windows/macOS 引入关闭默认 feature 的 `tray-icon`，平台模块由 `#[cfg]` 选择。没有引入轮询或为托盘另开应用进程，性能影响仍需 release 测量。

## 构建与验证

日常安装、快捷键与权限设置见 [中文 README](../README.zh-CN.md)。以下内容面向修改代码或自行打包的开发者；打包命令均在仓库根目录执行。

### 本地运行与检查

项目使用 Rust 2024 edition，依赖由 [Cargo.lock](../Cargo.lock) 锁定，工具链与检查要求见 [CI](../.github/workflows/ci.yml)。在图形桌面中运行：

```sh
cargo run --locked
```

代码变更按 CI 约定检查：

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo check --workspace --all-targets --all-features
```

默认测试使用本地模拟 HTTP 服务。下面的测试会向指定服务发送“你好，世界。”并打印译文，需要手动运行：

```sh
YERAVICH_CONFIG="$PWD/config.example.toml" cargo test configured_service_smoke_test -- --ignored --nocapture
```

测量性能时使用 `cargo build --release --locked`，再单独运行 `target/release/yeravich`。分别记录冷启动、常驻进程唤出和翻译请求延迟，避免将编译耗时混入测量。

### Linux AppImage 构建

在 Ubuntu 22.04 x86_64、已安装项目 Rust 工具链的环境中执行：

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

### macOS DMG 构建

打包使用固定自签名证书，尚未接入 Apple 公证。[DMG 工作流](../.github/workflows/dmg.yml) 和 [AppImage 工作流](../.github/workflows/appimage.yml) 支持手动运行，产物上传到 Actions artifacts；推送 `v*` 标签时，由 [Release 工作流](../.github/workflows/release.yml) 统一调用并发布，见下文。

#### 创建固定自签名证书

自签名不需要向 Apple 申请或购买开发者会员。只创建一次，后续版本复用；证书名称会随签名公开，可以使用项目名称。

1. 打开 macOS 的“钥匙串访问”，选择“证书助理 → 创建证书”。
2. 名称填写 `Yeravich Signing`，身份类型选择 **自签名根证书（Self Signed Root）**，证书类型选择 **代码签名（Code Signing）**。
3. 勾选“让我覆盖默认值”，设置适合长期维护的有效期（例如 3650 天）及 RSA 3072 或 4096 位密钥，将身份保存到登录钥匙串。
4. 在“我的证书”中找到该身份，确认可展开看到关联的私钥，再导出为带密码保护的 `.p12`。仅导出 `.cer` 不包含私钥，不能用于签名。
5. 将 `.p12` 和导出密码备份到自己的安全存储中。不要放进仓库、`private-docs/`、构建产物或日志；不要为每次构建重新生成证书。

可用 `security find-identity -p codesigning` 查看签名身份；自签名不在系统受信任身份列表中，因此这里不加仅列出有效受信任身份的 `-v`。签名时可以按证书名称选择，也可填写该身份的 SHA-1 指纹，后者用于区分同名证书。创建步骤参考 [Apple 证书助理说明](https://support.apple.com/guide/keychain-access/create-self-signed-certificates-kyca8916/mac) 和 [代码签名证书类型](https://developer.apple.com/library/archive/documentation/Security/Conceptual/CodeSigningGuide/Procedures/Procedures.html)。

#### GitHub Actions 签名 Secrets

进入仓库 **Settings → Environments**，创建或打开名称为 **MACOS** 的环境，在 **Environment secrets** 中添加以下两项，不要放到 Variables：

| Secret 名称 | 内容 |
| --- | --- |
| `MACOS_SIGNING_P12_BASE64` | 包含证书和私钥的 `.p12` 文件的 Base64 内容 |
| `MACOS_SIGNING_P12_PASSWORD` | 导出 `.p12` 时设置的密码 |

DMG 构建任务通过 `environment: MACOS` 读取该环境的 Secrets，手动构建和标签发布都使用同一配置。若给环境设置了审批或分支/标签限制，发布时也需要满足这些规则。

在 Mac 本机执行下面的命令，将编码结果直接复制到剪贴板，再粘贴到第一个 Secret 的值中；不要粘贴到聊天、终端输出或项目文档。路径仅为示例，替换成自己的安全存储位置。

```sh
base64 -i "$HOME/secure/yeravich-signing.p12" | pbcopy
```

Base64 是编码，不是加密。工作流从 `.p12` 自动识别唯一的代码签名身份，生成临时钥匙串密码，无需额外配置 Variables 或 Apple 账号。私钥仅导入 runner 的临时钥匙串并允许 `codesign` 使用，不修改系统根证书信任；打包后无论成功或失败都会执行清理，Artifacts 只上传 DMG 和校验和。参考 [GitHub 签名 Secrets 说明](https://docs.github.com/en/actions/how-tos/deploy/deploy-to-third-party-platforms/sign-xcode-applications)。

#### 本地构建与签名

在 Mac 上本地打包，需要 Rust、Xcode Command Line Tools、Python 3 和 Homebrew 的 `librsvg`：

```sh
xcode-select --install  # 已安装时跳过
brew install librsvg
export MACOS_SIGNING_IDENTITY="Yeravich Signing"
bash scripts/build-dmg.sh
# 替换为上一步生成的单个文件路径；以下是 Apple Silicon 示例。
bash scripts/smoke-test-dmg.sh dist/Yeravich-0.0.1-macos-arm64.dmg
```

脚本默认使用当前 Mac 的架构，产物在 `dist/`；也接受 `aarch64-apple-darwin` 或 `x86_64-apple-darwin` 参数（需先安装对应 Rust target）。打包使用 release 构建，将程序、`Info.plist` 和由现有 SVG 生成的 `.icns` 图标组成标准 `.app`，再用系统 `hdiutil` 创建带 Applications 快捷方式的压缩 DMG。`MACOSX_DEPLOYMENT_TARGET` 默认设为 `11.0`，与 bundle 的最低系统版本声明保持一致；这不是对所有 macOS 版本的兼容性保证。

工作流会验证校验和、挂载后的 bundle 与签名、系统动态库依赖，再使用临时配置启动包内程序，确认进程持续运行且已有配置不被修改；不请求翻译服务。

将来切换 Developer ID 时，需要导入对应证书，并补齐 Hardened Runtime、安全时间戳、`notarytool` 公证和票据附加流程；仅替换 Secret 不会自动完成公证。自签名证书与 Developer ID 的代码身份不同，升级时可能需要用户重新授予辅助功能权限。当前 `MACOS_SIGNING_IDENTITY` / `MACOS_SIGNING_KEYCHAIN` 只控制选用哪个签名身份与钥匙串。

### 自动发布 Release

版本以 `Cargo.toml` 的 `package.version` 为唯一来源。窗口标题在 [app/mod.rs](../src/app/mod.rs) 中通过 `env!("CARGO_PKG_VERSION")` 获取 Cargo 在编译时提供的版本；两个打包脚本通过 `cargo metadata` 获取同一版本，生成文件名与 macOS bundle 信息。`Cargo.lock` 中的项目版本由 Cargo 同步。当前不另设 `VERSION` 文件，以免引入第二处需要同步的版本号。Cargo 的版本字段说明见 [官方文档](https://doc.rust-lang.org/cargo/reference/manifest.html#the-version-field)。

版本号描述这份代码的版本，Git 标签固定对应提交，GitHub Release 则提供该标签的发布说明和安装包。当前由维护者选择发布时机，创建附注标签，再由 CI 自动构建和发布；修改版本号或只在本地打标签都不会触发发布。

[release.yml](../.github/workflows/release.yml) 是标签发布入口。先配置上面的两个 macOS 签名 Secrets；发布使用 GitHub 自动提供的 `GITHUB_TOKEN`，无需额外的个人访问令牌。只有 `publish` 任务具有 `contents: write`，构建任务保持只读仓库权限。

1. 更新 `Cargo.toml` 的 `package.version`，运行 `cargo check --locked` 检查锁文件是否同步；如提示需要更新，先运行 `cargo check` 更新 `Cargo.lock`，再完成 CI 检查。
2. 提交并推送版本、锁文件与工作流改动。
3. 确认待发布改动已提交，再创建并推送与版本完全一致的标签。例如 `package.version = "0.0.1"` 时执行：

   ```sh
   git tag -a v0.0.1 -m "Yeravich v0.0.1"
   git push origin v0.0.1
   ```

在 **Actions → Release** 查看运行。标签不匹配时会在构建前失败；检查、打包和冒烟验证任一失败，都不会进入发布任务。成功后，Release 包含一个 AppImage、两个架构的 DMG 及各自的 `.sha256`，共六个附件，发布说明由 GitHub 自动生成。`v0.0.2-rc.1` 这样的标签会标为预发布，Cargo 版本也必须是 `0.0.2-rc.1`。

`workflow_call` 让两个原有工作流同时支持独立手动打包与被 Release 调用。`needs: [appimage, dmg]` 表达构建完成的依赖，发布任务下载本次运行的 artifacts，检查文件齐全及校验和，再创建草稿、上传附件，最后公开。上传中断会保留草稿；在同一次运行中选择 **Re-run failed jobs** 可以继续上传和发布。重跑时会覆盖草稿中的同名附件，已经公开的 Release 保持不变。普通分支推送仍只触发 CI。

若以后增加自动打标签，可以在发布工作流中读取 Cargo 版本、创建标签，并继续调用打包与发布任务。不能只用默认 `GITHUB_TOKEN` 推送标签后等待另一条 `push.tags` 工作流启动：该令牌产生的推送不会触发新的工作流运行，见 [GitHub 触发规则](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/trigger-a-workflow#triggering-a-workflow-from-a-workflow)。

这里没有新增 Rust 依赖或所有权逻辑。版本仍来自 `Cargo.toml`；Rust 与发布流程的边界是打包脚本生成的文件，工作流只编排检查、构建和分发。参考 [GitHub 可复用工作流](https://docs.github.com/en/actions/how-tos/reuse-automations/reuse-workflows) 与 [GitHub CLI 发布命令](https://cli.github.com/manual/gh_release_create)。

本地已通过 actionlint 与 Bash 语法检查，并使用模拟 GitHub CLI 验证版本不匹配、附件缺失或损坏、预发布标记、创建或上传失败、草稿重试及已发布版本保持不变。Rust 格式、Clippy、测试和编译检查也已通过。这些检查没有实际创建 Release；自动发布流程尚待 GitHub 托管环境实际运行验证。

### 验证范围

以下范围来自已有实机与打包验证，不代表所有桌面、架构和系统版本均已验证。

- Hyprland 的 Wayland、Xwayland 路径已验证常驻、收起、唤回、焦点和退出；原生 Wayland 已验证选区或剪贴板带入、自动翻译和相同文本复用。强制移除 `WAYLAND_DISPLAY` 的 Xwayland 自动读取仍待验证。
- Hyprland 已检查双栏布局、多行输入、滚动、选择、复制、Enter 提交和错误显示；中文输入法候选确认仍待验证。macOS 新布局及完整键盘交互尚未完成实机验证。
- Apple Silicon macOS 已在原生文本框和 TextEdit 中验证授权后的未复制选区读取、剪贴板兜底、自动翻译、相同文本复用、聚焦时保留输入、收起和退出。
- Linux release AppImage 已通过构建和虚拟 X11 启动检查；Apple Silicon release DMG 已通过构建、固定证书签名、校验和及启动检查。Finder 拖拽安装、互联网下载后的 Gatekeeper 提示与升级后的辅助功能授权仍待验证。
- Linux 托盘已验证收起、恢复、退出及初始化失败时的关闭保护；macOS 托盘实机交互仍待补充。GitHub 托管打包工作流尚未实际触发。
- 尚未建立启动、唤出和常驻资源占用的性能基线；其他桌面、架构与系统版本不在上述实机验证范围内。

## 后续如何积累学习记录

每次涉及新概念的功能迭代，在对应章节补充简短说明：解决了什么问题、为何采用这种写法、数据由谁持有、如何验证。函数路径随代码调整更新，过时的描述及时替换。

学习顺序跟随实际需求：先掌握所有权与消息驱动的状态变化，随着常驻和全局快捷键学习资源释放与平台差异，再随着翻译服务学习异步和错误处理，并用实际测量指导性能调整。
