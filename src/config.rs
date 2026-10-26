//! Environment and `.env` lookup, locating the config directory, and reading
//! `config.toml`.

use std::collections::HashMap;
use std::fs::{self, DirBuilder};
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

/// Overrides the config directory (also used by the tests).
pub const CONFIG_DIR_VAR: &str = "TWITCH_WATCH_CONFIG_DIR";

/// The only variables read from a `.env` file.
const DOTENV_KEYS: [&str; 2] = ["TWITCH_CLIENT_ID", "TWITCH_ACCESS_TOKEN"];

/// Reads a process environment variable, treating blank values as unset.
#[must_use]
pub fn env_var(name: &str) -> Option<String> {
    non_blank(&std::env::var(name).ok()?)
}

fn non_blank(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

/// Credential variables from the process environment, falling back to a
/// `.env` file in the current directory or a parent.
///
/// Only [`DOTENV_KEYS`] are taken from `.env`, and nothing is exported to the
/// process environment. Otherwise a `.env` in an untrusted directory could
/// redirect where credentials are saved, or inject variables such as
/// `PYTHONPATH` into streamlink.
#[derive(Debug, Default)]
pub struct Env {
    dotenv: HashMap<String, String>,
}

impl Env {
    pub fn load() -> Result<Self> {
        let entries = match dotenvy::dotenv_iter() {
            Ok(entries) => entries,
            Err(error) if error.not_found() => return Ok(Self::default()),
            Err(error) => return Err(error).context("failed to load .env"),
        };
        let mut dotenv = HashMap::new();
        for entry in entries {
            let (key, value) = entry.context("failed to load .env")?;
            if DOTENV_KEYS.contains(&key.as_str()) {
                dotenv.insert(key, value);
            }
        }
        Ok(Self { dotenv })
    }

    /// Looks up a credential variable; blank values count as unset.
    #[must_use]
    pub fn var(&self, name: &str) -> Option<String> {
        env_var(name).or_else(|| self.dotenv.get(name).and_then(|value| non_blank(value)))
    }
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
