use yeravich_core::{AppConfig, ConfigStore};
use yeravich_platform_wayland::XdgConfigStore;

#[test]
fn missing_file_loads_defaults_and_saved_config_round_trips() {
    let directory = tempfile::tempdir().unwrap();
    let store = XdgConfigStore::new(directory.path().join("yeravich/config.toml"));

    let mut config = futures_executor::block_on(store.load()).unwrap();
    assert_eq!(config, AppConfig::default());
    config.languages.target = "ja".into();

    futures_executor::block_on(store.save(config.clone())).unwrap();
    assert_eq!(futures_executor::block_on(store.load()).unwrap(), config);
    assert!(store.path().is_file());
}
