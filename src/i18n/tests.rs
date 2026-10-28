use super::*;
use rust_i18n::t;

#[test]
fn system_locales_resolve_to_supported_languages() {
    for locale in ["zh", "zh-CN", "zh_TW.UTF-8", "ZH-Hans", "zh_HK@variant"] {
        assert_eq!(Language::from_locale(locale), Language::Chinese);
    }
    for locale in ["en", "en_US.UTF-8", "fr-FR", "C", ""] {
        assert_eq!(Language::from_locale(locale), Language::English);
    }
}

#[test]
fn generated_keys_resolve_and_interpolate_with_english_fallback() {
    assert_eq!(t!(Langs::TRANSLATE, locale = "zh-CN"), "翻译");
    assert_eq!(t!(Langs::TRANSLATE, locale = "en"), "Translate");
    assert_eq!(t!(Langs::TRANSLATE, locale = "fr"), "Translate");
    assert_eq!(
        t!(
            Langs::EXIT_HINT_SHORTCUT,
            locale = "zh-CN",
            shortcut = "Alt+Q"
        ),
        "退出应用。关闭窗口则继续运行，可用 Alt+Q 唤出。"
    );
    assert_eq!(
        t!(Langs::EXIT_HINT_SHORTCUT, locale = "en", shortcut = "Alt+Q"),
        "Quit the app. Close to keep running; Alt+Q to show again."
    );
}
