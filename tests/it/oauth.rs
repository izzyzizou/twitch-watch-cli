use mockito::Matcher;

use twitch_watch::twitch::{
    self,
    oauth::{self, DeviceCode},
};

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
        oauth::request_device_code(&twitch::agent(), &format!("{}/oauth2", server.url()), "cid")
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
        oauth::request_device_code(&twitch::agent(), &format!("{}/oauth2", server.url()), "cid")
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

    let token = oauth::poll_for_token(
        &twitch::agent(),
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
    let error = oauth::poll_for_token(
        &twitch::agent(),
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
    let error = oauth::poll_for_token(
        &twitch::agent(),
        &format!("{}/oauth2", server.url()),
        "cid",
        &device(0),
    )
    .err()
    .unwrap();
    assert!(error.to_string().starts_with("Login timed out."));
}
