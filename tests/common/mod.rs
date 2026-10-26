#![allow(dead_code)]

use mockito::{Matcher, Mock, ServerGuard};
use serde_json::{Value, json};

use twitch_watch::auth::Credentials;
use twitch_watch::client::{Client, Endpoints};

pub fn stream_json(login: &str, viewers: u64) -> Value {
    stream_json_with(login, viewers, "Game", "Some title")
}

pub fn stream_json_with(login: &str, viewers: u64, game: &str, title: &str) -> Value {
    json!({
        "id": "1",
        "user_id": "10",
        "user_login": login,
        "user_name": login,
        "game_id": "1",
        "game_name": game,
        "type": "live",
        "title": title,
        "viewer_count": viewers,
        "started_at": "2026-01-01T00:00:00Z",
        "language": "en",
        "thumbnail_url": "https://example.com/thumb.jpg",
        "tags": ["English"],
        "is_mature": false,
    })
}

pub fn credentials(token: &str) -> Credentials {
    Credentials {
        client_id: "cid".to_owned(),
        access_token: token.to_owned(),
        refresh_token: None,
    }
}

pub fn client_for(server: &ServerGuard) -> Client {
    Client::new(Endpoints::at(&server.url()), credentials("tok"), None)
}

pub fn validation_json(client_id: &str, scopes: &[&str]) -> String {
    json!({
        "client_id": client_id,
        "login": "me",
        "user_id": "user-1",
        "scopes": scopes,
        "expires_in": 5000,
    })
    .to_string()
}

/// Mocks a successful `/validate` for `token`.
pub fn mock_validate(server: &mut ServerGuard, token: &str) -> Mock {
    server
        .mock("GET", "/oauth2/validate")
        .match_header("authorization", format!("OAuth {token}").as_str())
        .with_body(validation_json("cid", &["user:read:follows"]))
        .create()
}

/// Starts a mock Twitch that validates token `tok` as user `user-1`, who
/// follows the given live `streams`.
pub fn mock_api(streams: Vec<Value>) -> ServerGuard {
    let mut server = mockito::Server::new();
    mock_validate(&mut server, "tok");
    server
        .mock("GET", "/helix/streams/followed")
        .match_query(Matcher::UrlEncoded("user_id".into(), "user-1".into()))
        .with_body(json!({"data": streams, "pagination": {}}).to_string())
        .create();
    server
}
