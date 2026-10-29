use super::*;

fn ax_range(location: isize, length: isize) -> CFRetained<AXValue> {
    let range = CFRange { location, length };
    // 测试按 SDK 约定包装系统范围，输入指针在创建期间有效。
    unsafe { AXValue::new(AXValueType::CFRange, NonNull::from(&range).cast()) }.unwrap()
}

#[test]
fn success_permission_and_remote_failures_stop_further_reads() {
    for result in [
        Ok("selected".to_owned()),
        Err(AXError::APIDisabled),
        Err(AXError::CannotComplete),
        Err(AXError::InvalidUIElement),
        Err(AXError::IllegalArgument),
    ] {
        assert_eq!(
            with_fallback(result.clone(), || panic!("must not read another interface")),
            result
        );
    }
}

#[test]
fn only_empty_or_unsupported_attributes_try_another_selection_interface() {
    for error in [
        AXError::NoValue,
        AXError::AttributeUnsupported,
        AXError::ParameterizedAttributeUnsupported,
        AXError::NotImplemented,
    ] {
        assert_eq!(
            with_fallback(Err(error), || Ok("  网页 selection 🙂  ".to_owned())),
            Ok("  网页 selection 🙂  ".into())
        );
    }
}

#[test]
fn fallback_does_not_mask_permission_or_communication_failures() {
    assert_eq!(
        with_fallback::<String>(Err(AXError::NoValue), || Err(AXError::AttributeUnsupported)),
        Err(AXError::NoValue)
    );
    for error in [
        AXError::APIDisabled,
        AXError::CannotComplete,
        AXError::InvalidUIElement,
    ] {
        assert_eq!(
            with_fallback::<String>(Err(AXError::NoValue), || Err(error)),
            Err(error)
        );
    }
}

#[test]
fn selected_string_preserves_unicode_and_whitespace_and_checks_types() {
    assert_eq!(
        selected_string(&CFString::from_str("  选中的文本\n🙂  ")),
        Ok("  选中的文本\n🙂  ".into())
    );
    assert_eq!(
        selected_string(&CFString::from_str(" \n")),
        Err(AXError::NoValue)
    );
    assert_eq!(
        selected_string(CFBoolean::new(true)),
        Err(AXError::IllegalArgument)
    );
}

#[test]
fn character_range_checks_both_cf_type_and_ax_value_type() {
    let value = ax_range(2, 4);
    assert!(selected_character_range(&value).is_ok());
    assert!(selected_character_range(&CFString::from_str("not a range")).is_err());
    let status = AXError::NoValue;
    let value =
        unsafe { AXValue::new(AXValueType::AXError, NonNull::from(&status).cast()) }.unwrap();
    assert_eq!(
        selected_character_range(&value).err(),
        Some(AXError::IllegalArgument)
    );
}

#[test]
fn caret_negative_and_overflowing_ranges_are_not_sent_to_the_text_api() {
    assert_eq!(
        selected_character_range(&ax_range(3, 0)).err(),
        Some(AXError::NoValue)
    );
    for (location, length) in [(-1, 2), (0, -1), (isize::MAX, 1)] {
        assert_eq!(
            selected_character_range(&ax_range(location, length)).err(),
            Some(AXError::IllegalArgument)
        );
    }
}

#[test]
fn requests_share_one_deadline_instead_of_adding_a_full_timeout_for_each_method() {
    let now = Instant::now();
    let deadline = now + CAPTURE_TIMEOUT;
    assert_eq!(request_timeout(deadline, now), Ok(REQUEST_TIMEOUT));
    assert_eq!(
        request_timeout(deadline, deadline - Duration::from_millis(10)),
        Ok(Duration::from_millis(10))
    );
    assert_eq!(
        request_timeout(deadline, deadline),
        Err(AXError::CannotComplete)
    );
    assert_eq!(
        request_timeout(deadline, deadline + Duration::from_millis(1)),
        Err(AXError::CannotComplete)
    );
}

#[test]
fn permission_empty_selection_and_remote_failures_have_distinct_user_messages() {
    assert_eq!(check(AXError::Success), Ok(()));
    assert_eq!(capture_error(AXError::NoValue), Error::NoText);
    assert_eq!(
        capture_error(AXError::APIDisabled),
        Error::AccessibilityRequired
    );
    for error in [
        AXError::CannotComplete,
        AXError::AttributeUnsupported,
        AXError::InvalidUIElement,
        AXError::IllegalArgument,
    ] {
        assert_eq!(check(error), Err(error));
        assert_eq!(capture_error(error), Error::SelectionUnavailable);
    }
}

fn running_selection() -> (i32, String) {
    let pid = std::env::var("YERAVICH_TEST_SOURCE_PID")
        .expect("set the source application's PID")
        .parse()
        .expect("source PID must be an integer");
    let expected =
        std::env::var("YERAVICH_TEST_SELECTED_TEXT").expect("set the expected fixture selection");
    (pid, expected)
}

fn assert_selection(actual: Result<String, Error>, expected: &str) {
    // 不将来源应用实际返回的正文写入断言或日志。
    assert!(
        actual.as_deref() == Ok(expected),
        "selection mismatch: {:?}",
        actual.as_ref().map(String::len)
    );
}

#[test]
#[ignore = "requires Accessibility access and a known selection in a running source app"]
fn selected_text_from_running_application() {
    let (pid, expected) = running_selection();
    assert_selection(selected_text(Some(pid)), &expected);
}

#[test]
#[ignore = "requires Accessibility access and a known selection in a native text control"]
fn selected_range_from_running_application() {
    let (pid, expected) = running_selection();
    assert!(trusted(), "test process requires Accessibility access");
    let deadline = Instant::now() + CAPTURE_TIMEOUT;
    let application = unsafe { AXUIElement::new_application(pid) };
    let focused = attribute(&application, "AXFocusedUIElement", deadline).unwrap();
    let focused = focused.downcast_ref::<AXUIElement>().unwrap();
    let value = attribute(focused, "AXSelectedTextRange", deadline).unwrap();
    let range = selected_character_range(&value).unwrap();
    assert_selection(
        parameterized_text(focused, "AXStringForRange", range, deadline).map_err(capture_error),
        &expected,
    );
}
