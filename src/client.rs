use std::cell::RefCell;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use ureq::http::Response;
use ureq::{Agent, Body};

use crate::auth::{self, Credentials, SCOPE};
use crate::models::{ApiResponse, Stream};

const TIMEOUT: Duration = Duration::from_secs(15);

/// Base URLs of the Helix API and the OAuth service.
#[derive(Debug, Clone)]
pub struct Endpoints {
    pub api: String,
    pub auth: String,
}

impl Endpoints {
    pub fn twitch() -> Self {
        Self {
            api: "https://api.twitch.tv/helix".to_owned(),
            auth: "https://id.twitch.tv/oauth2".to_owned(),
        }
    }

    /// Both services under one origin, e.g. a mock server.
    pub fn at(origin: &str) -> Self {
        Self {
            api: format!("{origin}/helix"),
            auth: format!("{origin}/oauth2"),
        }
    }
}

pub fn agent() -> Agent {
    Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        // Non-2xx responses are handled by `check_status` so the body can be reported.
        .http_status_as_error(false)
        .build()
        .into()
}

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

/// Response of the OAuth `/validate` endpoint.
#[derive(Debug, Deserialize)]
pub struct Validation {
    pub client_id: String,
    pub login: String,
    pub user_id: String,
    #[serde(default)]
    pub scopes: Vec<String>,
}

pub struct Client {
    agent: Agent,
    endpoints: Endpoints,
    credentials: RefCell<Credentials>,
    /// Where refreshed credentials are saved; `None` for environment tokens.
    store: Option<PathBuf>,
}

impl Client {
    pub fn new(endpoints: Endpoints, credentials: Credentials, store: Option<PathBuf>) -> Self {
        Self {
            agent: agent(),
            endpoints,
            credentials: RefCell::new(credentials),
            store,
        }
    }

    pub fn validate(&self) -> Result<Validation> {
        let url = format!("{}/validate", self.endpoints.auth);
        read_json(self.authorized_get(&url, &[], "OAuth")?, "/validate")
    }

    /// Validates the token (Twitch asks apps to do this on startup) and returns
    /// the user it belongs to.
    pub fn authenticated_user_id(&self) -> Result<String> {
        let validation = self.validate()?;
        let credentials = self.credentials.borrow();
        if validation.client_id != credentials.client_id {
            bail!(
                "The access token was issued to client ID {}, not {}. Check TWITCH_CLIENT_ID.",
                validation.client_id,
                credentials.client_id
            );
        }
        if !validation.scopes.iter().any(|scope| scope == SCOPE) {
            bail!(
                "The access token is missing the {SCOPE} scope. Run `twitch-watch login` to get a new one."
            );
        }
        Ok(validation.user_id)
    }

    pub fn get<T: DeserializeOwned>(&self, path: &str, params: &[(&str, &str)]) -> Result<T> {
        let url = format!("{}{path}", self.endpoints.api);
        read_json(self.authorized_get(&url, params, "Bearer")?, path)
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

    pub fn live_followed(&self, user_id: &str) -> Result<Vec<Stream>> {
        let mut streams: Vec<Stream> = self.get_all_pages(
            "/streams/followed",
            &[("user_id", user_id), ("first", "100")],
        )?;
        streams.sort_by_key(|stream| std::cmp::Reverse(stream.viewer_count));
        Ok(streams)
    }

    /// GETs `url`, refreshing the access token and retrying once on a 401.
    fn authorized_get(
        &self,
        url: &str,
        params: &[(&str, &str)],
        scheme: &str,
    ) -> Result<Response<Body>> {
        let mut refreshed = false;
        loop {
            let (client_id, token) = {
                let credentials = self.credentials.borrow();
                (
                    credentials.client_id.clone(),
                    credentials.access_token.clone(),
                )
            };
            let response = self
                .agent
                .get(url)
                .header("Client-Id", &client_id)
                .header("Authorization", &format!("{scheme} {token}"))
                .query_pairs(params.iter().copied())
                .call()
                .with_context(|| format!("request to {url} failed"))?;
            if response.status() == 401 && !refreshed && self.refresh()? {
                refreshed = true;
                continue;
            }
            return check_status(response);
        }
    }

    /// Swaps in a new access token using the refresh token, saving it if the
    /// credentials came from disk. Returns false if there's no refresh token.
    fn refresh(&self) -> Result<bool> {
        let (client_id, refresh_token) = {
            let credentials = self.credentials.borrow();
            match &credentials.refresh_token {
                Some(token) => (credentials.client_id.clone(), token.clone()),
                None => return Ok(false),
            }
        };
        let token = auth::refresh(
            &self.agent,
            &self.endpoints.auth,
            &client_id,
            &refresh_token,
        )?;
        let mut credentials = self.credentials.borrow_mut();
        credentials.access_token = token.access_token;
        // Twitch rotates refresh tokens; keep the old one if none was returned.
        if token.refresh_token.is_some() {
            credentials.refresh_token = token.refresh_token;
        }
        if let Some(path) = &self.store {
            credentials.save(path)?;
        }
        Ok(true)
    }
}
