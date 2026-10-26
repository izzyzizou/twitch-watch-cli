//! Twitch HTTP APIs: the Helix [`client`] and the [`oauth`] service.

pub mod client;
pub mod models;
pub mod oauth;

use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::de::DeserializeOwned;
use ureq::http::Response;
use ureq::{Agent, Body};

pub use client::{Client, Validation};

const TIMEOUT: Duration = Duration::from_secs(15);

/// Base URLs of the Helix API and the OAuth service.
#[derive(Debug, Clone)]
pub struct Endpoints {
    pub api: String,
    pub auth: String,
}

impl Endpoints {
    #[must_use]
    pub fn twitch() -> Self {
        Self {
            api: "https://api.twitch.tv/helix".to_owned(),
            auth: "https://id.twitch.tv/oauth2".to_owned(),
        }
    }

    /// Both services under one origin, e.g. a mock server.
    #[must_use]
    pub fn at(origin: &str) -> Self {
        Self {
            api: format!("{origin}/helix"),
            auth: format!("{origin}/oauth2"),
        }
    }
}

/// An HTTP agent with a global timeout that leaves non-2xx statuses to
/// [`check_status`], so error bodies can be reported.
#[must_use]
pub fn agent() -> Agent {
    Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .http_status_as_error(false)
        .build()
        .into()
}

/// Turns a non-2xx response into an error that includes the response body.
pub(crate) fn check_status(mut response: Response<Body>) -> Result<Response<Body>> {
    let status = response.status().as_u16();
    if (200..300).contains(&status) {
        return Ok(response);
    }
    let body = response.body_mut().read_to_string().unwrap_or_default();
    if status == 401 {
        bail!("API error 401: {body}\nRun `twitch-watch login` to sign in again.");
    }
    bail!("API error {status}: {body}")
}

pub(crate) fn read_json<T: DeserializeOwned>(
    mut response: Response<Body>,
    what: &str,
) -> Result<T> {
    response
        .body_mut()
        .read_json()
        .with_context(|| format!("invalid JSON from {what}"))
}
