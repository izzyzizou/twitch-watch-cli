use std::collections::HashMap;
use std::path::PathBuf;

use twitch_watch::config::{self, Config};

fn dir_with(pairs: &[(&str, &str)]) -> anyhow::Result<PathBuf> {
    let map: HashMap<&str, &str> = pairs.iter().copied().collect();
    config::dir(|name| map.get(name).map(|v| (*v).to_owned()))
}

#[test]
fn dir_prefers_explicit_override() {
    let dir = dir_with(&[
        ("TWITCH_WATCH_CONFIG_DIR", "/custom"),
        ("XDG_CONFIG_HOME", "/xdg"),
        ("HOME", "/home/me"),
    ]);
    assert_eq!(dir.unwrap(), PathBuf::from("/custom"));
}

#[test]
fn dir_uses_xdg_then_home() {
    let xdg = dir_with(&[("XDG_CONFIG_HOME", "/xdg"), ("HOME", "/home/me")]).unwrap();
    assert_eq!(xdg, PathBuf::from("/xdg/twitch-watch"));
    if !cfg!(windows) {
        let home = dir_with(&[("HOME", "/home/me")]).unwrap();
        assert_eq!(home, PathBuf::from("/home/me/.config/twitch-watch"));
    }
}

#[test]
fn dir_without_home_errors() {
    assert!(dir_with(&[]).is_err());
}

#[test]
fn config_missing_file_uses_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let config = Config::load(&dir.path().join("config.toml")).unwrap();
    assert!(config.quality.is_none());
}

#[test]
fn config_reads_quality() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "quality = \"720p60\"\n").unwrap();
    assert_eq!(
        Config::load(&path).unwrap().quality.as_deref(),
        Some("720p60")
    );
}

#[test]
fn config_rejects_unknown_keys() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "qualty = \"720p\"\n").unwrap();
    let error = format!("{:#}", Config::load(&path).unwrap_err());
    assert!(error.starts_with("invalid config file"));
    assert!(error.contains("qualty"));
}
