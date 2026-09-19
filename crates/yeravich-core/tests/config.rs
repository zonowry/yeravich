use std::collections::BTreeMap;

use yeravich_core::{AppConfig, CONFIG_VERSION, ProviderProfile, SecretReference};

#[test]
fn defaults_to_fixed_english_chinese_pair_and_current_version() {
    let config = AppConfig::default();
    assert_eq!(config.version, CONFIG_VERSION);
    assert_eq!(config.languages.source, "en");
    assert_eq!(config.languages.target, "zh-CN");
}

#[test]
fn public_configuration_round_trips_without_secret_material() {
    let config = AppConfig {
        active_provider: Some("work".into()),
        providers: vec![ProviderProfile {
            name: "work".into(),
            adapter_id: "openai".into(),
            public: BTreeMap::from([("model".into(), "example-model".into())]),
            secret_ref: Some(SecretReference("keyring:item/42".into())),
        }],
        ..AppConfig::default()
    };

    let encoded = config.to_toml().unwrap();
    assert!(encoded.contains("version = 1"));
    assert!(encoded.contains("keyring:item/42"));
    assert!(!encoded.contains("api_key"));
    assert!(!encoded.contains("secret_value"));
    assert_eq!(AppConfig::from_toml(&encoded).unwrap(), config);
}

#[test]
fn rejects_unknown_config_versions() {
    let text = AppConfig::default()
        .to_toml()
        .unwrap()
        .replace("version = 1", "version = 99");
    assert!(AppConfig::from_toml(&text).is_err());
}
