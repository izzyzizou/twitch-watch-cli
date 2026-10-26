mod common;

use std::collections::HashMap;

use mockito::Matcher;

use common::credentials;
use twitch_watch::auth::{self, Credentials, DeviceCode};
use twitch_watch::client;

fn env_from(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    move |name| map.get(name).cloned()
}

fn device(expires_in: u64) -> DeviceCode {
    DeviceCode {
        device_code: "dev-code".to_owned(),
        user_code: "ABCD-EFGH".to_owned(),
        verification_uri: "https://www.twitch.tv/activate".to_owned(),
        expires_in,
        interval: 0,
    }
}

#[test]
fn credentials_round_trip_privately() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join("credentials.json");
    let mut saved = credentials("tok");
    saved.refresh_token = Some("ref".to_owned());
    saved.save(&path).unwrap();

    let loaded = Credentials::load(&path).unwrap().unwrap();
    assert_eq!(loaded.access_token, "tok");
    assert_eq!(loaded.refresh_token.as_deref(), Some("ref"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&path), 0o600);
        assert_eq!(mode(path.parent().unwrap()), 0o700);
    }
}

#[test]
fn credentials_load_missing_is_none() {
    let dir = tempfile::tempdir().unwrap();
    assert!(
        Credentials::load(&dir.path().join("nope.json"))
            .unwrap()
            .is_none()
    );
}

#[test]
fn resolve_prefers_environment_token() {
    let dir = tempfile::tempdir().unwrap();
    let stored = dir.path().join("credentials.json");
    credentials("saved").save(&stored).unwrap();

    let env = env_from(&[
        ("TWITCH_CLIENT_ID", "env-cid"),
        ("TWITCH_ACCESS_TOKEN", "env-tok"),
    ]);
    let (creds, store) = auth::resolve(env, &stored).unwrap();
    assert_eq!(creds.client_id, "env-cid");
    assert_eq!(creds.access_token, "env-tok");
    assert!(store.is_none());
}

#[test]
fn resolve_env_token_requires_client_id() {
    let dir = tempfile::tempdir().unwrap();
    let env = env_from(&[("TWITCH_ACCESS_TOKEN", "env-tok")]);
    let error = auth::resolve(env, &dir.path().join("c.json"))
        .err()
        .unwrap();
    assert!(error.to_string().contains("TWITCH_CLIENT_ID must be set"));
}

#[test]
fn resolve_falls_back_to_saved_login() {
    let dir = tempfile::tempdir().unwrap();
    let stored = dir.path().join("credentials.json");
    credentials("saved").save(&stored).unwrap();

    // A client ID alone (e.g. kept around for `login`) doesn't override it.
    let env = env_from(&[("TWITCH_CLIENT_ID", "env-cid")]);
    let (creds, store) = auth::resolve(env, &stored).unwrap();
    assert_eq!(creds.access_token, "saved");
    assert_eq!(store.as_deref(), Some(stored.as_path()));
}

#[test]
fn resolve_without_credentials_says_not_logged_in() {
    let dir = tempfile::tempdir().unwrap();
    let error = auth::resolve(env_from(&[]), &dir.path().join("c.json"))
        .err()
        .unwrap();
    assert!(error.to_string().starts_with("Not logged in."));
}

#[test]
fn request_device_code_sends_client_and_scope() {
    let mut server = mockito::Server::new();
    let mock = server
        .mock("POST", "/oauth2/device")
        .match_body(Matcher::AllOf(vec![
            Matcher::UrlEncoded("client_id".into(), "cid".into()),
            Matcher::UrlEncoded("scopes".into(), "user:read:follows".into()),
        ]))
        .with_body(
            r#"{"device_code": "d", "user_code": "U", "verification_uri": "https://x",
                "expires_in": 1800, "interval": 5}"#,
        )
        .create();

    let code =
        auth::request_device_code(&client::agent(), &format!("{}/oauth2", server.url()), "cid")
            .unwrap();
    assert_eq!(code.user_code, "U");
    assert_eq!(code.interval, 5);
    mock.assert();
}

#[test]
fn request_device_code_explains_rejected_client() {
    let mut server = mockito::Server::new();
    server
        .mock("POST", "/oauth2/device")
        .with_status(400)
        .with_body(r#"{"message": "invalid client"}"#)
        .create();
    let error =
        auth::request_device_code(&client::agent(), &format!("{}/oauth2", server.url()), "cid")
            .unwrap_err();
    assert!(error.to_string().contains("Client Type is \"Public\""));
}

#[test]
fn poll_waits_while_authorization_is_pending() {
    let mut server = mockito::Server::new();
    let pending = server
        .mock("POST", "/oauth2/token")
        .with_status(400)
        .with_body(r#"{"status": 400, "message": "authorization_pending"}"#)
        .expect(1)
        .create();
    let granted = server
        .mock("POST", "/oauth2/token")
        .match_body(Matcher::UrlEncoded("device_code".into(), "dev-code".into()))
        .with_body(r#"{"access_token": "a", "refresh_token": "r", "expires_in": 100}"#)
        .create();

    let token = auth::poll_for_token(
        &client::agent(),
        &format!("{}/oauth2", server.url()),
        "cid",
        &device(60),
    )
    .unwrap();
    assert_eq!(token.access_token, "a");
    assert_eq!(token.refresh_token.as_deref(), Some("r"));
    pending.assert();
    granted.assert();
}

#[test]
fn poll_reports_denial() {
    let mut server = mockito::Server::new();
    server
        .mock("POST", "/oauth2/token")
        .with_status(400)
        .with_body(r#"{"status": 400, "message": "access_denied"}"#)
        .create();
    let error = auth::poll_for_token(
        &client::agent(),
        &format!("{}/oauth2", server.url()),
        "cid",
        &device(60),
    )
    .err()
    .unwrap();
    assert_eq!(error.to_string(), "Login was denied in the browser.");
}

#[test]
fn poll_times_out_when_code_expires() {
    let server = mockito::Server::new();
    let error = auth::poll_for_token(
        &client::agent(),
        &format!("{}/oauth2", server.url()),
        "cid",
        &device(0),
    )
    .err()
    .unwrap();
    assert!(error.to_string().starts_with("Login timed out."));
}
