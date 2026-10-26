//! Twitch OAuth requests: device-code sign-in, refresh and revocation.

use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use ureq::Agent;

use super::{check_status, read_json};

/// The only scope the CLI needs: reading the channels you follow.
pub const SCOPE: &str = "user:read:follows";

const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
const SLOW_DOWN_BACKOFF: Duration = Duration::from_secs(5);

#[derive(Debug, Deserialize)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

/// Deliberately not `Debug`, so tokens can't end up in logs or error output.
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
            "slow_down" => interval += SLOW_DOWN_BACKOFF,
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

pub fn revoke(agent: &Agent, auth_base: &str, client_id: &str, token: &str) -> Result<()> {
    let response = agent
        .post(format!("{auth_base}/revoke"))
        .send_form([("client_id", client_id), ("token", token)])
        .context("request to Twitch failed")?;
    check_status(response).map(drop)
}
