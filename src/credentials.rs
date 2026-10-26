//! The saved-login file and choosing between it and environment tokens.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::config;

/// Deliberately not `Debug`, so tokens can't end up in logs or error output.
#[derive(Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub client_id: String,
    pub access_token: String,
    /// Present for logins via `twitch-watch login`; absent for env tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
}

impl Credentials {
    /// Loads saved credentials, or `None` if the file doesn't exist.
    pub fn load(path: &Path) -> Result<Option<Self>> {
        match fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text)
                .map(Some)
                .with_context(|| format!("invalid credentials file {}", path.display())),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
        }
    }

    /// Writes the file atomically, readable only by the current user on Unix.
    pub fn save(&self, path: &Path) -> Result<()> {
        let dir = path
            .parent()
            .context("credentials path has no parent directory")?;
        config::create_private_dir(dir)
            .with_context(|| format!("failed to create {}", dir.display()))?;
        let json = serde_json::to_vec_pretty(self)?;
        let tmp = path.with_extension("json.tmp");
        write_private(&tmp, &json)
            .and_then(|()| fs::rename(&tmp, path))
            .with_context(|| format!("failed to write {}", path.display()))
    }
}

fn write_private(path: &Path, contents: &[u8]) -> io::Result<()> {
    // Remove any leftover file so the restrictive mode applies on creation.
    let _ = fs::remove_file(path);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)?.write_all(contents)
}

/// Picks this run's credentials: `TWITCH_ACCESS_TOKEN` + `TWITCH_CLIENT_ID`
/// from the environment if set, else the saved login. Also returns where
/// refreshed tokens should be saved: `None` for environment tokens, which
/// can't be refreshed.
pub fn resolve(
    env: impl Fn(&str) -> Option<String>,
    stored: &Path,
) -> Result<(Credentials, Option<PathBuf>)> {
    if let Some(access_token) = env("TWITCH_ACCESS_TOKEN") {
        let client_id = env("TWITCH_CLIENT_ID")
            .context("TWITCH_ACCESS_TOKEN is set, so TWITCH_CLIENT_ID must be set too.")?;
        let credentials = Credentials {
            client_id,
            access_token,
            refresh_token: None,
        };
        return Ok((credentials, None));
    }
    match Credentials::load(stored)? {
        Some(credentials) => Ok((credentials, Some(stored.to_owned()))),
        None => bail!(
            "Not logged in. Run `twitch-watch login --client-id <client id>` (see README),\n\
             or set TWITCH_CLIENT_ID and TWITCH_ACCESS_TOKEN."
        ),
    }
}
