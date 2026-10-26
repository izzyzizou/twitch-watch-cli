//! End-to-end checks of the compiled binary, run with a clean environment in an
//! empty directory so no real `.env` or credentials are picked up.

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
        .envs(envs.iter().copied())
        .output()
        .unwrap()
}

#[test]
fn missing_credentials_exit_with_error() {
    let dir = scratch_dir("missing");
    let output = run_in(&dir, &[], &[("TWITCH_CLIENT_ID", "cid")]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&output.stderr).trim(),
        "Missing required environment variable: TWITCH_ACCESS_TOKEN"
    );
}

#[test]
fn blank_credentials_count_as_missing() {
    let dir = scratch_dir("blank");
    let output = run_in(&dir, &[], &[("TWITCH_CLIENT_ID", "  ")]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("TWITCH_CLIENT_ID"));
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
