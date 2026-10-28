//! macOS Accessibility 读取只针对唤出前的应用，不模拟复制或修改剪贴板。
use std::ptr::NonNull;

use iced::Task;
use objc2_app_kit::NSWorkspace;
use objc2_application_services::{
    AXError, AXIsProcessTrusted, AXIsProcessTrustedWithOptions, AXUIElement,
    kAXTrustedCheckOptionPrompt,
};
use objc2_core_foundation::{CFBoolean, CFDictionary, CFRetained, CFString, CFType};
use objc2_foundation::{NSString, NSURL};

use super::{Error, Snapshot, usable_text};

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
    // PID 在唤出前固定；应用退出时 AX 返回错误，不转而读取当前前台的 Yeravich。
    let application = unsafe { AXUIElement::new_application(pid) };
    let focused = attribute(&application, "AXFocusedUIElement")?;
    let focused = focused
        .downcast_ref::<AXUIElement>()
        .ok_or(Error::SelectionUnavailable)?;
    let value = attribute(focused, "AXSelectedText")?;
    selected_string(&value)
}

fn attribute(element: &AXUIElement, name: &str) -> Result<CFRetained<CFType>, Error> {
    // 超时只作用于这个有效的 AX 对象；两次读取最多各等待 250 毫秒。
    check(unsafe { element.set_messaging_timeout(0.25) })?;
    let mut value = std::ptr::null();
    // value 指向本栈帧中的有效输出槽。Copy API 成功后转移一个 +1 引用给调用方。
    check(unsafe {
        element.copy_attribute_value(&CFString::from_str(name), NonNull::from(&mut value))
    })?;
    let value = NonNull::new(value.cast_mut()).ok_or(Error::NoText)?;
    // CFRetained 接管 Copy 返回的引用，所有退出路径都会释放它。
    Ok(unsafe { CFRetained::from_raw(value) })
}

fn selected_string(value: &CFType) -> Result<String, Error> {
    // 来源应用可能返回不符合约定的类型；检查 CFTypeID 后才把它当作字符串。
    let text = value
        .downcast_ref::<CFString>()
        .ok_or(Error::SelectionUnavailable)?;
    usable_text(Some(text.to_string()))
}

fn check(status: AXError) -> Result<(), Error> {
    match status {
        AXError::Success => Ok(()),
        AXError::NoValue => Err(Error::NoText),
        AXError::APIDisabled => Err(Error::AccessibilityRequired),
        _ => Err(Error::SelectionUnavailable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_text_requires_a_string_and_preserves_unicode_and_whitespace() {
        assert_eq!(
            selected_string(&CFString::from_str("  选中的文本\n🙂  ")),
            Ok("  选中的文本\n🙂  ".into())
        );
        assert_eq!(
            selected_string(&CFString::from_str(" \n")),
            Err(Error::NoText)
        );
        assert_eq!(
            selected_string(CFBoolean::new(true)),
            Err(Error::SelectionUnavailable)
        );
    }

    #[test]
    fn permission_empty_selection_and_remote_failures_remain_distinct() {
        assert_eq!(check(AXError::NoValue), Err(Error::NoText));
        assert_eq!(
            check(AXError::APIDisabled),
            Err(Error::AccessibilityRequired)
        );
        for error in [
            AXError::CannotComplete,
            AXError::AttributeUnsupported,
            AXError::InvalidUIElement,
        ] {
            assert_eq!(check(error), Err(Error::SelectionUnavailable));
        }
    }
}
