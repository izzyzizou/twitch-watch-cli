use mockito::Matcher;
use serde_json::{Value, json};

use crate::common::{client_for, creds, mock_validate, stream_json, validation_json};
use twitch_watch::credentials::Credentials;
use twitch_watch::twitch::{Client, Endpoints};

#[test]
fn get_parses_json_and_sends_query_and_headers() {
    let mut server = mockito::Server::new();
    let mock = server
        .mock("GET", "/helix/users")
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
        .mock("GET", "/helix/x")
        .with_status(404)
        .with_body(r#"{"error": "nope"}"#)
        .create();

    let error = client_for(&server).get::<Value>("/x", &[]).unwrap_err();
    assert_eq!(error.to_string(), r#"API error 404: {"error": "nope"}"#);
}

#[test]
fn unauthorized_without_refresh_token_suggests_login() {
    let mut server = mockito::Server::new();
    server
        .mock("GET", "/oauth2/validate")
        .with_status(401)
        .with_body(r#"{"message": "invalid access token"}"#)
        .create();
    let refresh = server.mock("POST", "/oauth2/token").expect(0).create();

    let error = client_for(&server).validate().unwrap_err().to_string();
    assert!(error.starts_with("API error 401"));
    assert!(error.contains("twitch-watch login"));
    refresh.assert();
}

#[test]
fn get_all_pages_follows_cursor() {
    let mut server = mockito::Server::new();
    let first = server
        .mock("GET", "/helix/streams/followed")
        .match_query(Matcher::Exact("user_id=u&first=100".into()))
        .with_body(r#"{"data": [{"n": 1}], "pagination": {"cursor": "next"}}"#)
        .create();
    let second = server
        .mock("GET", "/helix/streams/followed")
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
fn get_all_pages_stops_on_repeated_cursor() {
    let mut server = mockito::Server::new();
    let mock = server
        .mock("GET", "/helix/streams/followed")
        .match_query(Matcher::Any)
        .with_body(r#"{"data": [{"n": 1}], "pagination": {"cursor": "same"}}"#)
        .expect(2)
        .create();

    let result: Vec<Value> = client_for(&server)
        .get_all_pages("/streams/followed", &[])
        .unwrap();
    assert_eq!(result.len(), 2);
    mock.assert();
}

#[test]
fn live_followed_sorts_by_viewers() {
    let mut server = mockito::Server::new();
    server
        .mock("GET", "/helix/streams/followed")
        .match_query(Matcher::Any)
        .with_body(json!({"data": [stream_json("low", 5), stream_json("high", 500)]}).to_string())
        .create();

    let streams = client_for(&server).live_followed("u").unwrap();
    let logins: Vec<_> = streams.iter().map(|s| s.user_login.as_str()).collect();
    assert_eq!(logins, ["high", "low"]);
}

#[test]
fn authenticated_user_id_validates_token() {
    let mut server = mockito::Server::new();
    let mock = mock_validate(&mut server, "tok");
    assert_eq!(
        client_for(&server).authenticated_user_id().unwrap(),
        "user-1"
    );
    mock.assert();
}

#[test]
fn authenticated_user_id_rejects_other_client_id() {
    let mut server = mockito::Server::new();
    server
        .mock("GET", "/oauth2/validate")
        .with_body(validation_json("other", &["user:read:follows"]))
        .create();
    let error = client_for(&server).authenticated_user_id().unwrap_err();
    assert!(
        error
            .to_string()
            .contains("issued to client ID other, not cid")
    );
}

#[test]
fn authenticated_user_id_requires_follows_scope() {
    let mut server = mockito::Server::new();
    server
        .mock("GET", "/oauth2/validate")
        .with_body(validation_json("cid", &["chat:read"]))
        .create();
    let error = client_for(&server).authenticated_user_id().unwrap_err();
    assert!(
        error
            .to_string()
            .contains("missing the user:read:follows scope")
    );
}

#[test]
fn expired_token_is_refreshed_retried_and_saved() {
    let mut server = mockito::Server::new();
    server
        .mock("GET", "/oauth2/validate")
        .match_header("authorization", "OAuth old")
        .with_status(401)
        .create();
    let refresh = server
        .mock("POST", "/oauth2/token")
        .match_body(Matcher::AllOf(vec![
            Matcher::UrlEncoded("grant_type".into(), "refresh_token".into()),
            Matcher::UrlEncoded("refresh_token".into(), "r1".into()),
            Matcher::UrlEncoded("client_id".into(), "cid".into()),
        ]))
        .with_body(r#"{"access_token": "new", "refresh_token": "r2"}"#)
        .create();
    mock_validate(&mut server, "new");

    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("credentials.json");
    let mut old = creds("old");
    old.refresh_token = Some("r1".to_owned());
    let client = Client::new(Endpoints::at(&server.url()), old, Some(store.clone()));

    assert_eq!(client.authenticated_user_id().unwrap(), "user-1");
    refresh.assert();
    let saved = Credentials::load(&store).unwrap().unwrap();
    assert_eq!(saved.access_token, "new");
    assert_eq!(saved.refresh_token.as_deref(), Some("r2"));
}

#[test]
fn failed_refresh_asks_to_log_in_again() {
    let mut server = mockito::Server::new();
    server
        .mock("GET", "/oauth2/validate")
        .with_status(401)
        .create();
    server
        .mock("POST", "/oauth2/token")
        .with_status(400)
        .with_body(r#"{"message": "Invalid refresh token"}"#)
        .create();

    let mut expired = creds("old");
    expired.refresh_token = Some("r1".to_owned());
    let client = Client::new(Endpoints::at(&server.url()), expired, None);
    let error = client.validate().unwrap_err();
    assert!(error.to_string().contains("session has expired"));
}
