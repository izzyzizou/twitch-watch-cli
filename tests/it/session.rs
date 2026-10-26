use std::cell::RefCell;

use mockito::Matcher;

use crate::common::{creds, mock_validate};
use twitch_watch::commands::session;
use twitch_watch::credentials::Credentials;
use twitch_watch::twitch::Endpoints;

#[test]
fn login_runs_device_flow_and_saves_credentials() {
    let mut server = mockito::Server::new();
    server
        .mock("POST", "/oauth2/device")
        .with_body(
            r#"{"device_code": "d", "user_code": "ABCD-EFGH",
                "verification_uri": "https://www.twitch.tv/activate?device-code=ABCD-EFGH",
                "expires_in": 60, "interval": 0}"#,
        )
        .create();
    server
        .mock("POST", "/oauth2/token")
        .match_body(Matcher::UrlEncoded("device_code".into(), "d".into()))
        .with_body(r#"{"access_token": "fresh", "refresh_token": "ref"}"#)
        .create();
    mock_validate(&mut server, "fresh");

    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("credentials.json");
    let mut out = Vec::new();
    let opened = RefCell::new(None);
    session::login(
        &Endpoints::at(&server.url()),
        "cid",
        &store,
        &mut out,
        |url| {
            *opened.borrow_mut() = Some(url.to_owned());
        },
    )
    .unwrap();

    let out = String::from_utf8(out).unwrap();
    assert!(out.contains("ABCD-EFGH"));
    assert!(out.contains("Logged in as me."));
    assert_eq!(
        opened.into_inner().as_deref(),
        Some("https://www.twitch.tv/activate?device-code=ABCD-EFGH")
    );
    let saved = Credentials::load(&store).unwrap().unwrap();
    assert_eq!(saved.client_id, "cid");
    assert_eq!(saved.access_token, "fresh");
    assert_eq!(saved.refresh_token.as_deref(), Some("ref"));
}

#[test]
fn logout_revokes_and_deletes() {
    let mut server = mockito::Server::new();
    let revoke = server
        .mock("POST", "/oauth2/revoke")
        .match_body(Matcher::AllOf(vec![
            Matcher::UrlEncoded("client_id".into(), "cid".into()),
            Matcher::UrlEncoded("token".into(), "tok".into()),
        ]))
        .create();
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("credentials.json");
    creds("tok").save(&store).unwrap();

    let mut out = Vec::new();
    session::logout(&Endpoints::at(&server.url()), &store, &mut out).unwrap();
    revoke.assert();
    assert!(!store.exists());
    assert!(String::from_utf8(out).unwrap().starts_with("Logged out"));
}

#[test]
fn logout_deletes_even_if_revoke_fails() {
    let mut server = mockito::Server::new();
    server
        .mock("POST", "/oauth2/revoke")
        .with_status(500)
        .create();
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("credentials.json");
    creds("tok").save(&store).unwrap();

    let mut out = Vec::new();
    session::logout(&Endpoints::at(&server.url()), &store, &mut out).unwrap();
    assert!(!store.exists());
    assert!(String::from_utf8(out).unwrap().contains("could not revoke"));
}
