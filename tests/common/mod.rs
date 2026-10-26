#![allow(dead_code)]

use mockito::{Matcher, ServerGuard};
use serde_json::{Value, json};

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

/// Starts a mock Twitch API that resolves user `user-1` and returns `streams`.
pub fn mock_api(streams: Vec<Value>) -> ServerGuard {
    let mut server = mockito::Server::new();
    server
        .mock("GET", "/users")
        .with_body(
            json!({"data": [{"id": "user-1", "login": "me", "display_name": "me"}]}).to_string(),
        )
        .create();
    server
        .mock("GET", "/streams/followed")
        .match_query(Matcher::UrlEncoded("user_id".into(), "user-1".into()))
        .with_body(json!({"data": streams, "pagination": {}}).to_string())
        .create();
    server
}
