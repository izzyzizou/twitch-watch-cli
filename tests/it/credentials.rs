use std::collections::HashMap;

use crate::common::creds;
use twitch_watch::credentials::{self, Credentials};

fn env_from(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    move |name| map.get(name).cloned()
}

#[test]
fn credentials_round_trip_privately() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join("credentials.json");
    let mut saved = creds("tok");
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
    creds("saved").save(&stored).unwrap();

    let env = env_from(&[
        ("TWITCH_CLIENT_ID", "env-cid"),
        ("TWITCH_ACCESS_TOKEN", "env-tok"),
    ]);
    let (creds, store) = credentials::resolve(env, &stored).unwrap();
    assert_eq!(creds.client_id, "env-cid");
    assert_eq!(creds.access_token, "env-tok");
    assert!(store.is_none());
}

#[test]
fn resolve_env_token_requires_client_id() {
    let dir = tempfile::tempdir().unwrap();
    let env = env_from(&[("TWITCH_ACCESS_TOKEN", "env-tok")]);
    let error = credentials::resolve(env, &dir.path().join("c.json"))
        .err()
        .unwrap();
    assert!(error.to_string().contains("TWITCH_CLIENT_ID must be set"));
}

#[test]
fn resolve_falls_back_to_saved_login() {
    let dir = tempfile::tempdir().unwrap();
    let stored = dir.path().join("credentials.json");
    creds("saved").save(&stored).unwrap();

    // A client ID alone (e.g. kept around for `login`) doesn't override it.
    let env = env_from(&[("TWITCH_CLIENT_ID", "env-cid")]);
    let (creds, store) = credentials::resolve(env, &stored).unwrap();
    assert_eq!(creds.access_token, "saved");
    assert_eq!(store.as_deref(), Some(stored.as_path()));
}

#[test]
fn resolve_without_credentials_says_not_logged_in() {
    let dir = tempfile::tempdir().unwrap();
    let error = credentials::resolve(env_from(&[]), &dir.path().join("c.json"))
        .err()
        .unwrap();
    assert!(error.to_string().starts_with("Not logged in."));
}
