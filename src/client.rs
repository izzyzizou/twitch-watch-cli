use anyhow::{Context, Result, bail};
use serde::de::DeserializeOwned;

use crate::models::{ApiResponse, Stream, User};

pub const API_BASE: &str = "https://api.twitch.tv/helix";

pub fn require_env(name: &str) -> Result<String> {
    match std::env::var(name) {
        Ok(value) if !value.is_empty() => Ok(value),
        _ => bail!("Missing required environment variable: {name}"),
    }
}

pub struct Client {
    http: reqwest::blocking::Client,
    base_url: String,
    client_id: String,
    token: String,
}

impl Client {
    pub fn new(client_id: impl Into<String>, token: impl Into<String>) -> Self {
        Self::with_base_url(API_BASE, client_id, token)
    }

    pub fn with_base_url(
        base_url: impl Into<String>,
        client_id: impl Into<String>,
        token: impl Into<String>,
    ) -> Self {
        Self {
            http: reqwest::blocking::Client::new(),
            base_url: base_url.into(),
            client_id: client_id.into(),
            token: token.into(),
        }
    }

    pub fn get<T: DeserializeOwned>(&self, path: &str, params: &[(&str, &str)]) -> Result<T> {
        let response = self
            .http
            .get(format!("{}{path}", self.base_url))
            .header("Client-Id", &self.client_id)
            .bearer_auth(&self.token)
            .query(params)
            .send()
            .with_context(|| format!("request to {path} failed"))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().unwrap_or_default();
            bail!("API error {}: {body}", status.as_u16());
        }
        response
            .json()
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
            results.extend(page.data);
            cursor = page
                .pagination
                .and_then(|pagination| pagination.cursor)
                .filter(|cursor| !cursor.is_empty());
            if cursor.is_none() {
                return Ok(results);
            }
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
