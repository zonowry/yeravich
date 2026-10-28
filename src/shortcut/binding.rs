use global_hotkey::hotkey::{Code, HotKey, Modifiers};

/// 只保存已校验的按键；显示名称和 Portal 格式都从同一个值生成。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding(HotKey);

impl Default for Binding {
    fn default() -> Self {
        Self(HotKey::new(Some(Modifiers::ALT), Code::KeyQ))
    }
}

impl Binding {
    pub fn parse(value: &str) -> Result<Self, &'static str> {
        let hotkey = value
            .trim()
            .parse::<HotKey>()
            .map_err(|_| "shortcuts.show_window")?;
        // 普通输入不能被全局拦截，也不能覆盖应用自己的退出组合键。
        let command = if cfg!(target_os = "macos") {
            Modifiers::META
        } else {
            Modifiers::CONTROL
        };
        if !hotkey
            .mods
            .intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::META)
            || (hotkey.key == Code::KeyQ && hotkey.mods.contains(command))
            || key_names(hotkey.key).is_none()
        {
            return Err("shortcuts.show_window");
        }
        Ok(Self(hotkey))
    }

    pub(super) fn hotkey(self) -> HotKey {
        self.0
    }

    pub fn description(self) -> String {
        let mut parts = Vec::new();
        for (modifier, name) in [
            (Modifiers::CONTROL, "Ctrl"),
            (
                Modifiers::ALT,
                if cfg!(target_os = "macos") {
                    "Option"
                } else {
                    "Alt"
                },
            ),
            (Modifiers::SHIFT, "Shift"),
            (
                Modifiers::META,
                if cfg!(target_os = "macos") {
                    "Cmd"
                } else {
                    "Super"
                },
            ),
        ] {
            if self.0.mods.contains(modifier) {
                parts.push(name.to_owned());
            }
        }
        if let Some((name, _)) = key_names(self.0.key) {
            parts.push(name);
        }
        parts.join("+")
    }

    #[cfg(any(target_os = "linux", test))]
    pub(super) fn preferred_trigger(self) -> String {
        let mut parts = Vec::new();
        for (modifier, name) in [
            (Modifiers::CONTROL, "CTRL"),
            (Modifiers::ALT, "ALT"),
            (Modifiers::SHIFT, "SHIFT"),
            (Modifiers::META, "LOGO"),
        ] {
            if self.0.mods.contains(modifier) {
                parts.push(name.to_owned());
            }
        }
        if let Some((_, keysym)) = key_names(self.0.key) {
            parts.push(keysym);
        }
        parts.join("+")
    }
}

fn key_names(key: Code) -> Option<(String, String)> {
    let code = key.to_string();
    if let Some(letter) = code.strip_prefix("Key") {
        return Some((letter.to_owned(), letter.to_ascii_lowercase()));
    }
    if let Some(digit) = code.strip_prefix("Digit") {
        return Some((digit.to_owned(), digit.to_owned()));
    }
    if code
        .strip_prefix('F')
        .and_then(|number| number.parse::<u8>().ok())
        .is_some_and(|number| (1..=24).contains(&number))
    {
        return Some((code.clone(), code));
    }
    let keysym = match key {
        Code::Space => "space",
        Code::Enter => "Return",
        Code::Tab => "Tab",
        Code::Escape => "Escape",
        Code::Backspace => "BackSpace",
        Code::Delete => "Delete",
        Code::Insert => "Insert",
        Code::Home => "Home",
        Code::End => "End",
        Code::PageUp => "Prior",
        Code::PageDown => "Next",
        Code::ArrowUp => "Up",
        Code::ArrowDown => "Down",
        Code::ArrowLeft => "Left",
        Code::ArrowRight => "Right",
        _ => return None,
    };
    Some((code, keysym.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_and_option_alias_use_the_same_alt_q_binding() {
        for value in ["Alt+Q", "Option+Q", " alt + KeyQ "] {
            assert_eq!(Binding::parse(value).unwrap(), Binding::default());
        }
        assert_eq!(Binding::default().preferred_trigger(), "ALT+q");
        assert_eq!(
            Binding::default().description(),
            if cfg!(target_os = "macos") {
                "Option+Q"
            } else {
                "Alt+Q"
            }
        );
    }

    #[test]
    fn portal_uses_xdg_modifier_and_keysym_names() {
        for (value, expected) in [
            ("Ctrl+Alt+Space", "CTRL+ALT+space"),
            ("Super+Shift+Enter", "SHIFT+LOGO+Return"),
            ("Alt+PageUp", "ALT+Prior"),
            ("Ctrl+F12", "CTRL+F12"),
            ("Alt+1", "ALT+1"),
        ] {
            assert_eq!(Binding::parse(value).unwrap().preferred_trigger(), expected);
        }
    }

    #[test]
    fn invalid_unmodified_and_exit_shortcuts_are_rejected() {
        for value in [
            "",
            " ",
            "Q",
            "Shift+Q",
            "Alt+",
            "Alt++Q",
            "Alt+Q+W",
            "Alt+NotAKey",
            "Alt+MediaPlayPause",
        ] {
            assert_eq!(Binding::parse(value), Err("shortcuts.show_window"));
        }
        let exit = if cfg!(target_os = "macos") {
            "Cmd+Q"
        } else {
            "Ctrl+Q"
        };
        assert!(Binding::parse(exit).is_err());
    }
}
