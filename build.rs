use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt::Write;
use std::fs;
use std::path::PathBuf;

type Messages = BTreeMap<String, String>;

fn main() {
    println!("cargo:rerun-if-changed=locales");

    let english = load_locale("en");
    let chinese = load_locale("zh-CN");

    let english_keys: BTreeSet<_> = english.keys().collect();
    let chinese_keys: BTreeSet<_> = chinese.keys().collect();
    assert_eq!(
        english_keys, chinese_keys,
        "English and Chinese must contain the same translation keys"
    );

    let mut generated = String::from(
        "// Generated from locales/en.json by build.rs. Do not edit.\n\
         pub struct Langs;\n\
         // Some messages are used only on specific target platforms.\n\
         #[allow(dead_code)]\n\
         impl Langs {\n",
    );

    for (key, value) in &english {
        assert!(
            key.starts_with(|c: char| c.is_ascii_lowercase())
                && key
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
            "Translation key {key:?} must use lowercase snake_case"
        );
        assert_eq!(
            placeholders(value),
            placeholders(&chinese[key]),
            "Translation parameters differ for {key:?}"
        );

        writeln!(
            generated,
            "    pub const {}: &'static str = {key:?};",
            key.to_ascii_uppercase(),
        )
        .expect("writing generated translation keys");
    }
    generated.push_str("}\n");

    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo provides OUT_DIR"));
    fs::write(output.join("langs.rs"), generated).expect("writing generated translation keys");
}

fn load_locale(locale: &str) -> Messages {
    let path = format!("locales/{locale}.json");
    let contents = fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path}: {error}"));
    let messages: Messages =
        serde_json::from_str(&contents).unwrap_or_else(|error| panic!("{path}: {error}"));

    assert!(!messages.is_empty(), "{path} must contain translations");
    for (key, value) in &messages {
        assert!(
            !value.trim().is_empty(),
            "{path}: empty translation for {key:?}"
        );
    }

    messages
}

fn placeholders(value: &str) -> BTreeSet<&str> {
    value
        .split("%{")
        .skip(1)
        .map(|part| {
            part.split_once('}')
                .expect("translation parameter is missing its closing brace")
                .0
        })
        .collect()
}
