mod common;

use mockito::Matcher;
use serde_json::{Value, json};

use common::stream_json;
use twitch_watch::client::{Client, require_env};

fn client_for(server: &mockito::ServerGuard) -> Client {
    Client::with_base_url(server.url(), "cid", "tok")
}

#[test]
fn require_env_returns_value() {
    // SAFETY: each env test uses its own variable name.
    unsafe { std::env::set_var("TW_TEST_PRESENT", "abc") };
    assert_eq!(require_env("TW_TEST_PRESENT").unwrap(), "abc");
}

#[test]
fn require_env_missing_errors() {
    unsafe { std::env::remove_var("TW_TEST_MISSING") };
    let error = require_env("TW_TEST_MISSING").unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Missing required environment variable")
    );
}

#[test]
fn get_parses_json_and_sends_query_and_headers() {
    let mut server = mockito::Server::new();
    let mock = server
        .mock("GET", "/users")
        .match_header("client-id", "cid")
        .match_header("authorization", "Bearer tok")
        .match_query(Matcher::Exact("a=b&c=d".into()))
        .with_body(r#"{"data": [{"id": "1"}]}"#)
        .create();

    let result: Value = client_for(&server)
        .get("/users", &[("a", "b"), ("c", "d")])
        .unwrap();
    assert_eq!(result, json!({"data": [{"id": "1"}]}));
    mock.assert();
}

#[test]
fn get_errors_on_http_error() {
    let mut server = mockito::Server::new();
    server
        .mock("GET", "/x")
        .with_status(404)
        .with_body(r#"{"error": "nope"}"#)
        .create();

    let error = client_for(&server).get::<Value>("/x", &[]).unwrap_err();
    assert_eq!(error.to_string(), r#"API error 404: {"error": "nope"}"#);
}

#[test]
fn get_all_pages_follows_cursor() {
    let mut server = mockito::Server::new();
    let first = server
        .mock("GET", "/streams/followed")
        .match_query(Matcher::Exact("user_id=u&first=100".into()))
        .with_body(r#"{"data": [{"n": 1}], "pagination": {"cursor": "next"}}"#)
        .create();
    let second = server
        .mock("GET", "/streams/followed")
        .match_query(Matcher::Exact("user_id=u&first=100&after=next".into()))
        .with_body(r#"{"data": [{"n": 2}], "pagination": {}}"#)
        .create();

    let result: Vec<Value> = client_for(&server)
        .get_all_pages("/streams/followed", &[("user_id", "u"), ("first", "100")])
        .unwrap();
    assert_eq!(result, vec![json!({"n": 1}), json!({"n": 2})]);
    first.assert();
    second.assert();
}

#[test]
fn own_user_id_returns_id() {
    let mut server = mockito::Server::new();
    server
        .mock("GET", "/users")
        .with_body(r#"{"data": [{"id": "42", "login": "me", "display_name": "me"}]}"#)
        .create();
    assert_eq!(client_for(&server).own_user_id().unwrap(), "42");
}

#[test]
fn own_user_id_empty_errors() {
    let mut server = mockito::Server::new();
    server
        .mock("GET", "/users")
        .with_body(r#"{"data": []}"#)
        .create();
    let error = client_for(&server).own_user_id().unwrap_err();
    assert!(error.to_string().contains("Could not resolve user ID"));
}

#[test]
fn live_followed_sorts_by_viewers() {
    let mut server = mockito::Server::new();
    server
        .mock("GET", "/streams/followed")
        .match_query(Matcher::Any)
        .with_body(json!({"data": [stream_json("low", 5), stream_json("high", 500)]}).to_string())
        .create();

    let streams = client_for(&server).live_followed("u").unwrap();
    let logins: Vec<_> = streams.iter().map(|s| s.user_login.as_str()).collect();
    assert_eq!(logins, ["high", "low"]);
}
