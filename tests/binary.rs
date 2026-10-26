//! End-to-end checks of the compiled binary, run with a clean environment, an
//! empty working directory and an empty config directory, so no real `.env`,
//! config or credentials are picked up and nothing touches the network.

use std::path::PathBuf;
use std::process::{Command, Output};

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("twitch-watch-test-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run_in(dir: &PathBuf, args: &[&str], envs: &[(&str, &str)]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_twitch-watch"))
        .args(args)
        .current_dir(dir)
        .env_clear()
        .env("TWITCH_WATCH_CONFIG_DIR", dir.join("config"))
        .envs(envs.iter().copied())
        .output()
        .unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).trim().to_owned()
}

#[test]
fn not_logged_in_exits_with_error() {
    let dir = scratch_dir("not-logged-in");
    let output = run_in(&dir, &[], &[("TWITCH_CLIENT_ID", "cid")]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).starts_with("Not logged in."));
}

#[test]
fn env_token_without_client_id_exits_with_error() {
    let dir = scratch_dir("token-only");
    let output = run_in(
        &dir,
        &[],
        &[("TWITCH_ACCESS_TOKEN", "tok"), ("TWITCH_CLIENT_ID", "  ")],
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("TWITCH_CLIENT_ID must be set"));
}

#[test]
fn invalid_config_is_reported() {
    let dir = scratch_dir("bad-config");
    std::fs::create_dir_all(dir.join("config")).unwrap();
    std::fs::write(dir.join("config").join("config.toml"), "quality = 720\n").unwrap();
    let output = run_in(&dir, &[], &[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).starts_with("invalid config file"));
}

#[test]
fn login_without_client_id_explains() {
    let dir = scratch_dir("login");
    let output = run_in(&dir, &["login"], &[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("--client-id"));
}

#[test]
fn logout_when_not_logged_in() {
    let dir = scratch_dir("logout");
    let output = run_in(&dir, &["logout"], &[]);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "Not logged in."
    );
}

#[test]
fn malformed_dotenv_is_reported() {
    let dir = scratch_dir("dotenv");
    std::fs::write(dir.join(".env"), "TWITCH_CLIENT_ID='unterminated\n").unwrap();
    let output = run_in(&dir, &[], &[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).starts_with("failed to load .env"));
}

#[test]
fn version_flag() {
    let dir = scratch_dir("version");
    let output = run_in(&dir, &["--version"], &[]);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        format!("twitch-watch {}", env!("CARGO_PKG_VERSION"))
    );
}
