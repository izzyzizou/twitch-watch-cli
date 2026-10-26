use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::de::DeserializeOwned;
use ureq::Agent;

use crate::models::{ApiResponse, Stream, User};

pub const API_BASE: &str = "https://api.twitch.tv/helix";

const TIMEOUT: Duration = Duration::from_secs(15);

/// Reads a required, non-blank environment variable.
pub fn require_env(name: &str) -> Result<String> {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => Ok(value.trim().to_owned()),
        _ => bail!("Missing required environment variable: {name}"),
    }
}

pub struct Client {
    agent: Agent,
    base_url: String,
    client_id: String,
    authorization: String,
}

impl Client {
    pub fn new(client_id: impl Into<String>, token: &str) -> Self {
        Self::with_base_url(API_BASE, client_id, token)
    }

    pub fn with_base_url(
        base_url: impl Into<String>,
        client_id: impl Into<String>,
        token: &str,
    ) -> Self {
        let agent = Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            // Non-2xx responses are handled in `get` so the body can be reported.
            .http_status_as_error(false)
            .build()
            .into();
        Self {
            agent,
            base_url: base_url.into(),
            client_id: client_id.into(),
            authorization: format!("Bearer {token}"),
        }
    }

    pub fn get<T: DeserializeOwned>(&self, path: &str, params: &[(&str, &str)]) -> Result<T> {
        let mut response = self
            .agent
            .get(format!("{}{path}", self.base_url))
            .header("Client-Id", &self.client_id)
            .header("Authorization", &self.authorization)
            .query_pairs(params.iter().copied())
            .call()
            .with_context(|| format!("request to {path} failed"))?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            let body = response.body_mut().read_to_string().unwrap_or_default();
            if status == 401 {
                bail!(
                    "API error 401: {body}\n\
                     The access token is invalid or expired, or doesn't match TWITCH_CLIENT_ID."
                );
            }
            bail!("API error {status}: {body}");
        }
        response
            .body_mut()
            .read_json()
            .with_context(|| format!("invalid JSON from {path}"))
    }

    pub fn get_all_pages<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<Vec<T>> {
        let mut results = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut page_params = params.to_vec();
            if let Some(after) = &cursor {
                page_params.push(("after", after));
            }
            let page: ApiResponse<T> = self.get(path, &page_params)?;
            let next = page
                .pagination
                .and_then(|pagination| pagination.cursor)
                .filter(|next| !next.is_empty());
            results.extend(page.data);
            // Stop on a missing cursor, and guard against an API that repeats one.
            if next.is_none() || next == cursor {
                return Ok(results);
            }
            cursor = next;
        }
    }

    pub fn own_user_id(&self) -> Result<String> {
        let payload: ApiResponse<User> = self.get("/users", &[])?;
        match payload.data.into_iter().next() {
            Some(user) => Ok(user.id),
            None => bail!("Could not resolve user ID from access token."),
        }
    }

    pub fn live_followed(&self, user_id: &str) -> Result<Vec<Stream>> {
        let mut streams: Vec<Stream> = self.get_all_pages(
            "/streams/followed",
            &[("user_id", user_id), ("first", "100")],
        )?;
        streams.sort_by_key(|stream| std::cmp::Reverse(stream.viewer_count));
        Ok(streams)
    }
}
