//! Twitch OAuth: device-code login, token refresh and revocation, and the
//! on-disk credentials file.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use ureq::Agent;

use crate::client::{check_status, read_json};
use crate::config;

/// The only scope the CLI needs: reading the channels you follow.
pub const SCOPE: &str = "user:read:follows";

const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";

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
/// from the environment if set, else the saved login. Returns where refreshed
/// tokens should be saved (`None` for environment tokens, which can't refresh).
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

#[derive(Debug, Deserialize)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Deserialize)]
pub struct Token {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
}

#[derive(Deserialize)]
struct OAuthError {
    #[serde(default)]
    message: String,
}

pub fn request_device_code(agent: &Agent, auth_base: &str, client_id: &str) -> Result<DeviceCode> {
    let mut response = agent
        .post(format!("{auth_base}/device"))
        .send_form([("client_id", client_id), ("scopes", SCOPE)])
        .context("request to Twitch failed")?;
    if response.status() == 400 {
        let body = response.body_mut().read_to_string().unwrap_or_default();
        bail!(
            "Twitch rejected the client ID: {body}\n\
             Check it, and that the app's Client Type is \"Public\" in the Twitch developer console."
        );
    }
    read_json(check_status(response)?, "/device")
}

/// Polls until the user approves the device code in their browser.
pub fn poll_for_token(
    agent: &Agent,
    auth_base: &str,
    client_id: &str,
    device: &DeviceCode,
) -> Result<Token> {
    let deadline = Instant::now() + Duration::from_secs(device.expires_in);
    let mut interval = Duration::from_secs(device.interval);
    loop {
        std::thread::sleep(interval);
        if Instant::now() >= deadline {
            bail!("Login timed out. Run `twitch-watch login` to try again.");
        }
        let mut response = agent
            .post(format!("{auth_base}/token"))
            .send_form([
                ("client_id", client_id),
                ("scopes", SCOPE),
                ("device_code", device.device_code.as_str()),
                ("grant_type", DEVICE_GRANT),
            ])
            .context("request to Twitch failed")?;
        let status = response.status().as_u16();
        if status == 200 {
            return read_json(response, "/token");
        }
        let body = response.body_mut().read_to_string().unwrap_or_default();
        let message = serde_json::from_str::<OAuthError>(&body)
            .map(|error| error.message)
            .unwrap_or_default();
        match message.as_str() {
            "authorization_pending" => {}
            "slow_down" => interval += Duration::from_secs(5),
            "access_denied" => bail!("Login was denied in the browser."),
            _ => bail!("Login failed ({status}): {body}"),
        }
    }
}

pub fn refresh(
    agent: &Agent,
    auth_base: &str,
    client_id: &str,
    refresh_token: &str,
) -> Result<Token> {
    let response = agent
        .post(format!("{auth_base}/token"))
        .send_form([
            ("client_id", client_id),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ])
        .context("request to Twitch failed")?;
    if !response.status().is_success() {
        bail!("Your Twitch session has expired. Run `twitch-watch login` to sign in again.");
    }
    read_json(response, "/token")
}

pub fn revoke(agent: &Agent, auth_base: &str, credentials: &Credentials) -> Result<()> {
    let response = agent
        .post(format!("{auth_base}/revoke"))
        .send_form([
            ("client_id", credentials.client_id.as_str()),
            ("token", credentials.access_token.as_str()),
        ])
        .context("request to Twitch failed")?;
    check_status(response).map(drop)
}
