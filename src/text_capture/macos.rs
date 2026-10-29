//! macOS Accessibility 读取只针对唤出前的应用，不模拟复制或修改剪贴板。
use std::ptr::NonNull;
use std::time::{Duration, Instant};

use iced::Task;
use objc2_app_kit::NSWorkspace;
use objc2_application_services::{
    AXError, AXIsProcessTrusted, AXIsProcessTrustedWithOptions, AXTextMarkerRange, AXUIElement,
    AXValue, AXValueType, kAXTrustedCheckOptionPrompt,
};
use objc2_core_foundation::{CFBoolean, CFDictionary, CFRange, CFRetained, CFString, CFType};
use objc2_foundation::{NSString, NSURL};

use super::{Error, Snapshot, usable_text};

const CAPTURE_TIMEOUT: Duration = Duration::from_millis(750);
const REQUEST_TIMEOUT: Duration = Duration::from_millis(250);

/// 在显示本应用窗口之前调用。跨线程只传 PID，不传 AppKit / AX 对象。
pub fn frontmost_application() -> Option<i32> {
    NSWorkspace::sharedWorkspace()
        .frontmostApplication()
        .map(|app| app.processIdentifier())
        .filter(|pid| *pid > 0 && *pid as u32 != std::process::id())
}

pub fn permission(request: bool) -> Task<Result<bool, Error>> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                if trusted() {
                    return Ok(true);
                }
                if request {
                    // 字典的键是系统提供的 CFString，值是 CFBoolean；均由字典保留。
                    let options = CFDictionary::from_slices(
                        &[unsafe { kAXTrustedCheckOptionPrompt }],
                        &[CFBoolean::new(true)],
                    );
                    unsafe { AXIsProcessTrustedWithOptions(Some(options.as_opaque())) };
                    let url = NSURL::URLWithString(&NSString::from_str(
                        "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
                    ))
                    .ok_or(Error::AccessibilitySettings)?;
                    if !NSWorkspace::sharedWorkspace().openURL(&url) {
                        return Err(Error::AccessibilitySettings);
                    }
                }
                // 系统提示是异步的，返回不代表用户已经授权。下次聚焦/唤出再检查。
                Ok(trusted())
            })
            .await
            .unwrap_or(Err(Error::AccessibilitySettings))
        },
        std::convert::identity,
    )
}

pub fn read(source_pid: Option<i32>) -> Task<Snapshot> {
    Task::perform(
        async move {
            // AX 会跨进程通信，不能阻塞 Iced 的事件线程。每次通信单独限制超时。
            tokio::task::spawn_blocking(move || selected_text(source_pid))
                .await
                .unwrap_or(Err(Error::SelectionUnavailable))
        },
        std::convert::identity,
    )
    .then(|mut selection| {
        iced::clipboard::read().map(move |clipboard| Snapshot {
            primary: std::mem::replace(&mut selection, Err(Error::NoText)),
            clipboard: usable_text(clipboard),
        })
    })
}

fn trusted() -> bool {
    // 此 API 无参数，只查询本进程的信任状态，不弹框或修改权限。
    unsafe { AXIsProcessTrusted() }
}

fn selected_text(source_pid: Option<i32>) -> Result<String, Error> {
    if !trusted() {
        return Err(Error::AccessibilityRequired);
    }
    let pid = source_pid
        .filter(|pid| *pid > 0 && *pid as u32 != std::process::id())
        .ok_or(Error::NoText)?;
    // 固定来源 PID，且所有 AX 请求共用截止时间；不查询后来取得焦点的其他应用。
    let application = unsafe { AXUIElement::new_application(pid) };
    let deadline = Instant::now() + CAPTURE_TIMEOUT;
    let focused = attribute(&application, "AXFocusedUIElement", deadline).map_err(capture_error)?;
    let focused = focused
        .downcast_ref::<AXUIElement>()
        .ok_or(Error::SelectionUnavailable)?;
    let direct =
        attribute(focused, "AXSelectedText", deadline).and_then(|value| selected_string(&value));
    let ranged = with_fallback(direct, || {
        let value = attribute(focused, "AXSelectedTextRange", deadline)?;
        let range = selected_character_range(&value)?;
        // 将系统范围原样交回 SDK，不把字符索引当成 Rust String 的 UTF-8 字节下标。
        parameterized_text(focused, "AXStringForRange", range, deadline)
    });
    with_fallback(ranged, || {
        // 保留已验证的 Zen / Firefox 兼容分支；标记只交回同一个元素解析。
        let value = attribute(focused, "AXSelectedTextMarkerRange", deadline)?;
        let range = value
            .downcast_ref::<AXTextMarkerRange>()
            .ok_or(AXError::IllegalArgument)?;
        parameterized_text(focused, "AXStringForTextMarkerRange", range, deadline)
    })
    .map_err(capture_error)
}

fn with_fallback<T>(
    first: Result<T, AXError>,
    read_next: impl FnOnce() -> Result<T, AXError>,
) -> Result<T, AXError> {
    match first {
        Err(error) if missing_attribute(error) => match read_next() {
            // 已读到空值时，后续接口不支持仍表示没有可用选区。
            Err(next) if missing_attribute(next) && error == AXError::NoValue => Err(error),
            result => result,
        },
        result => result,
    }
}

fn missing_attribute(error: AXError) -> bool {
    matches!(
        error,
        AXError::NoValue
            | AXError::AttributeUnsupported
            | AXError::ParameterizedAttributeUnsupported
            | AXError::NotImplemented
    )
}

fn selected_character_range(value: &CFType) -> Result<&AXValue, AXError> {
    let value = value
        .downcast_ref::<AXValue>()
        .ok_or(AXError::IllegalArgument)?;
    let mut range = CFRange {
        location: 0,
        length: 0,
    };
    // AXValueGetValue 检查内部类型；输出指针指向完整、可写的 CFRange。
    if !unsafe { value.value(AXValueType::CFRange, NonNull::from(&mut range).cast()) }
        || range.location < 0
        || range.length < 0
        || range.location.checked_add(range.length).is_none()
    {
        return Err(AXError::IllegalArgument);
    }
    if range.length == 0 {
        return Err(AXError::NoValue);
    }
    Ok(value)
}

fn request_timeout(deadline: Instant, now: Instant) -> Result<Duration, AXError> {
    deadline
        .checked_duration_since(now)
        .filter(|remaining| !remaining.is_zero())
        .map(|remaining| remaining.min(REQUEST_TIMEOUT))
        .ok_or(AXError::CannotComplete)
}

fn prepare(element: &AXUIElement, deadline: Instant) -> Result<(), AXError> {
    let timeout = request_timeout(deadline, Instant::now())?;
    // 每次最多 250 毫秒，所有兼容路径共享 750 毫秒预算，避免逐项累加等待。
    check(unsafe { element.set_messaging_timeout(timeout.as_secs_f32()) })
}

fn parameterized_text(
    element: &AXUIElement,
    name: &str,
    range: &CFType,
    deadline: Instant,
) -> Result<String, AXError> {
    prepare(element, deadline)?;
    let mut value = std::ptr::null();
    // 两处调用分别传经过类型/范围检查的 AXValue 和 AXTextMarkerRange，
    // 与各自属性名匹配。范围来自同一个元素，输出槽有效。
    let status = unsafe {
        element.copy_parameterized_attribute_value(
            &CFString::from_str(name),
            range,
            NonNull::from(&mut value),
        )
    };
    if status != AXError::Success {
        eprintln!("[capture macOS] {name}: {status:?}");
    }
    check(status)?;
    let value = NonNull::new(value.cast_mut()).ok_or(AXError::NoValue)?;
    // 与普通 Copy 属性相同，成功后由 CFRetained 接管 +1 引用。
    let value = unsafe { CFRetained::from_raw(value) };
    selected_string(&value)
}

fn attribute(
    element: &AXUIElement,
    name: &str,
    deadline: Instant,
) -> Result<CFRetained<CFType>, AXError> {
    prepare(element, deadline)?;
    let mut value = std::ptr::null();
    // value 是有效输出槽。Copy API 成功后转移一个 +1 引用给调用方。
    let status = unsafe {
        element.copy_attribute_value(&CFString::from_str(name), NonNull::from(&mut value))
    };
    if status != AXError::Success {
        eprintln!("[capture macOS] {name}: {status:?}");
    }
    check(status)?;
    let value = NonNull::new(value.cast_mut()).ok_or(AXError::NoValue)?;
    // CFRetained 接管 Copy 返回的引用，所有退出路径都会释放它。
    Ok(unsafe { CFRetained::from_raw(value) })
}

fn selected_string(value: &CFType) -> Result<String, AXError> {
    let text = value
        .downcast_ref::<CFString>()
        .ok_or(AXError::IllegalArgument)?;
    usable_text(Some(text.to_string())).map_err(|_| AXError::NoValue)
}

fn check(status: AXError) -> Result<(), AXError> {
    match status {
        AXError::Success => Ok(()),
        error => Err(error),
    }
}

fn capture_error(status: AXError) -> Error {
    match status {
        AXError::NoValue => Error::NoText,
        AXError::APIDisabled => Error::AccessibilityRequired,
        _ => Error::SelectionUnavailable,
    }
}

#[cfg(test)]
mod tests;
