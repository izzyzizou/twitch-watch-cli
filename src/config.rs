//! Locating the config directory and reading `config.toml`.

use std::fs::{self, DirBuilder};
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

/// Overrides the config directory (also used by the tests).
pub const CONFIG_DIR_VAR: &str = "TWITCH_WATCH_CONFIG_DIR";

/// Reads an environment variable, treating blank values as unset.
#[must_use]
pub fn env_var(name: &str) -> Option<String> {
    let value = std::env::var(name).ok()?;
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

/// `$TWITCH_WATCH_CONFIG_DIR`, else `$XDG_CONFIG_HOME/twitch-watch`, else
/// `%APPDATA%\twitch-watch` on Windows, else `~/.config/twitch-watch`.
pub fn dir(env: impl Fn(&str) -> Option<String>) -> Result<PathBuf> {
    if let Some(dir) = env(CONFIG_DIR_VAR) {
        return Ok(dir.into());
    }
    if let Some(xdg) = env("XDG_CONFIG_HOME") {
        return Ok(Path::new(&xdg).join("twitch-watch"));
    }
    if cfg!(windows)
        && let Some(appdata) = env("APPDATA")
    {
        return Ok(Path::new(&appdata).join("twitch-watch"));
    }
    if let Some(home) = env("HOME") {
        return Ok(Path::new(&home).join(".config").join("twitch-watch"));
    }
    bail!("Could not determine a config directory; set HOME or {CONFIG_DIR_VAR}.")
}

pub struct Paths {
    pub config: PathBuf,
    pub credentials: PathBuf,
}

impl Paths {
    #[must_use]
    pub fn in_dir(dir: &Path) -> Self {
        Self {
            config: dir.join("config.toml"),
            credentials: dir.join("credentials.json"),
        }
    }
}

/// User preferences from `config.toml`. Every key is optional.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Default streamlink quality, overridden by `--quality`.
    pub quality: Option<String>,
}

impl Config {
    /// Loads the config, or the defaults if the file doesn't exist.
    pub fn load(path: &Path) -> Result<Self> {
        match fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text)
                .with_context(|| format!("invalid config file {}", path.display())),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
        }
    }
}

/// Creates `dir` (and parents), accessible only by the current user on Unix.
pub fn create_private_dir(dir: &Path) -> io::Result<()> {
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir)
}
