mod common;

use std::cell::RefCell;
use std::io::{Cursor, Write};
use std::process::ExitCode;

use clap::Parser;
use mockito::Matcher;

use common::{credentials, mock_api, mock_validate, stream_json, stream_json_with};
use twitch_watch::auth::Credentials;
use twitch_watch::cli::{self, Args, Cmd};
use twitch_watch::client::{Client, Endpoints};
use twitch_watch::models::Stream;

struct Outcome {
    result: anyhow::Result<ExitCode>,
    out: String,
    launches: Vec<(String, String)>,
}

fn run_with_picker(
    argv: &[&str],
    streams: Vec<serde_json::Value>,
    pick: impl FnOnce(&[&Stream], &mut dyn Write) -> anyhow::Result<Option<usize>>,
) -> Outcome {
    let server = mock_api(streams);
    let client = Client::new(Endpoints::at(&server.url()), credentials("tok"), None);
    let args = Args::parse_from(argv);
    let quality = args.quality.clone().unwrap_or_else(|| "best".to_owned());
    let mut out = Vec::new();
    let launches = RefCell::new(Vec::new());
    let result = cli::run(&args, &quality, &client, &mut out, pick, |url, quality| {
        launches
            .borrow_mut()
            .push((url.to_owned(), quality.to_owned()));
        Ok(ExitCode::from(7))
    });
    Outcome {
        result,
        out: String::from_utf8(out).unwrap(),
        launches: launches.into_inner(),
    }
}

fn run_with(argv: &[&str], streams: Vec<serde_json::Value>, input: &str) -> Outcome {
    run_with_picker(
        argv,
        streams,
        cli::numbered_picker(Cursor::new(input.to_owned())),
    )
}

fn never_pick(_: &[&Stream], _: &mut dyn Write) -> anyhow::Result<Option<usize>> {
    panic!("picker should not be shown");
}

fn two_streams() -> Vec<serde_json::Value> {
    vec![stream_json("alice", 10), stream_json("bob", 200)]
}

fn launched(launches: &[(String, String)]) -> Vec<&str> {
    launches.iter().map(|(url, _)| url.as_str()).collect()
}

#[test]
fn args_parse_channel_and_subcommands() {
    let args = Args::parse_from(["twitch-watch", "shroud", "-q", "720p"]);
    assert_eq!(args.channel.as_deref(), Some("shroud"));
    assert_eq!(args.quality.as_deref(), Some("720p"));
    assert!(args.command.is_none());

    let args = Args::parse_from(["twitch-watch", "login", "--client-id", "abc"]);
    assert!(matches!(args.command, Some(Cmd::Login { client_id: Some(ref id) }) if id == "abc"));
    assert!(Args::try_parse_from(["twitch-watch", "login", "--list"]).is_err());
}

#[test]
fn no_live_streams() {
    let outcome = run_with(&["twitch-watch"], vec![], "");
    outcome.result.unwrap();
    assert!(
        outcome
            .out
            .contains("No followed channels are live right now.")
    );
}

#[test]
fn list_flag_does_not_prompt() {
    let outcome = run_with_picker(&["twitch-watch", "--list"], two_streams(), never_pick);
    outcome.result.unwrap();
    // Sorted by viewers, so bob comes first.
    assert!(outcome.out.contains("[1] bob"));
    assert!(outcome.out.contains("[2] alice"));
    assert!(outcome.launches.is_empty());
}

#[test]
fn launches_streamlink() {
    let outcome = run_with(&["twitch-watch", "-q", "720p"], two_streams(), "1\n");
    // The launcher's exit code is passed through.
    assert_eq!(outcome.result.unwrap(), ExitCode::from(7));
    assert_eq!(
        outcome.launches,
        [("https://twitch.tv/bob".to_owned(), "720p".to_owned())]
    );
    assert!(outcome.out.contains("https://twitch.tv/bob [720p]"));
}

#[test]
fn quit_does_not_launch() {
    for input in ["q\n", "QUIT\n", "\n", ""] {
        let outcome = run_with(&["twitch-watch"], two_streams(), input);
        outcome.result.unwrap();
        assert!(outcome.launches.is_empty(), "input {input:?} launched");
    }
}

#[test]
fn invalid_selection_errors() {
    for input in ["99\n", "0\n", "abc\n"] {
        let outcome = run_with(&["twitch-watch"], two_streams(), input);
        assert_eq!(
            outcome.result.unwrap_err().to_string(),
            "Invalid selection."
        );
        assert!(outcome.launches.is_empty());
    }
}

#[test]
fn long_multibyte_title_is_truncated_by_chars() {
    let title = "é".repeat(100);
    let streams = vec![stream_json_with("carol", 1, "", &title)];
    let outcome = run_with(&["twitch-watch", "--list"], streams, "");
    outcome.result.unwrap();
    assert!(outcome.out.contains(&format!("\"{}\"", "é".repeat(80))));
    assert!(!outcome.out.contains("()"), "empty game should be omitted");
}

#[test]
fn title_control_characters_are_stripped() {
    let streams = vec![stream_json_with("dave", 1, "Game", "hi\u{1b}[2Jthere")];
    let outcome = run_with(&["twitch-watch", "--list"], streams, "");
    outcome.result.unwrap();
    assert!(!outcome.out.contains('\u{1b}'));
    assert!(outcome.out.contains("\"hi[2Jthere\""));
}

#[test]
fn exact_channel_launches_without_prompt() {
    for query in [
        "bob",
        "BOB",
        "@bob",
        "twitch.tv/bob",
        "https://www.twitch.tv/Bob/",
    ] {
        let outcome = run_with_picker(&["twitch-watch", query], two_streams(), never_pick);
        outcome.result.unwrap();
        assert_eq!(
            launched(&outcome.launches),
            ["https://twitch.tv/bob"],
            "query {query:?}"
        );
    }
}

#[test]
fn exact_match_wins_over_partial_matches() {
    let streams = vec![stream_json("bobby", 500), stream_json("bob", 1)];
    let outcome = run_with_picker(&["twitch-watch", "bob"], streams, never_pick);
    outcome.result.unwrap();
    assert_eq!(launched(&outcome.launches), ["https://twitch.tv/bob"]);
}

#[test]
fn single_partial_match_launches() {
    let outcome = run_with_picker(&["twitch-watch", "ob"], two_streams(), never_pick);
    outcome.result.unwrap();
    assert_eq!(launched(&outcome.launches), ["https://twitch.tv/bob"]);
}

#[test]
fn several_partial_matches_are_offered_in_the_picker() {
    let streams = vec![
        stream_json("alice", 10),
        stream_json("bob", 200),
        stream_json("alicia", 50),
    ];
    let offered = RefCell::new(Vec::new());
    let outcome = run_with_picker(&["twitch-watch", "ali"], streams, |candidates, _| {
        *offered.borrow_mut() = candidates.iter().map(|s| s.user_login.clone()).collect();
        Ok(Some(1))
    });
    outcome.result.unwrap();
    assert_eq!(offered.into_inner(), ["alicia", "alice"]);
    assert_eq!(launched(&outcome.launches), ["https://twitch.tv/alice"]);
}

#[test]
fn unmatched_channel_errors() {
    let outcome = run_with_picker(&["twitch-watch", "zed"], two_streams(), never_pick);
    assert_eq!(
        outcome.result.unwrap_err().to_string(),
        "None of your live followed channels match \"zed\"."
    );
}

#[test]
fn channel_with_list_prints_only_matches() {
    let outcome = run_with_picker(
        &["twitch-watch", "ali", "--list"],
        two_streams(),
        never_pick,
    );
    outcome.result.unwrap();
    assert!(outcome.out.contains("[1] alice"));
    assert!(!outcome.out.contains("bob"));
}

#[test]
fn launch_reports_missing_program() {
    let error = cli::launch("twitch-watch-no-such-program", "url", "best").unwrap_err();
    assert_eq!(
        error.to_string(),
        "twitch-watch-no-such-program not found. Make sure it's installed and on your PATH."
    );
}

#[test]
fn launch_passes_through_exit_code() {
    // `true`/`false` ignore their arguments.
    assert_eq!(
        cli::launch("true", "url", "best").unwrap(),
        ExitCode::SUCCESS
    );
    assert_eq!(
        cli::launch("false", "url", "best").unwrap(),
        ExitCode::FAILURE
    );
}

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
    cli::login(
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
    credentials("tok").save(&store).unwrap();

    let mut out = Vec::new();
    cli::logout(&Endpoints::at(&server.url()), &store, &mut out).unwrap();
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
    credentials("tok").save(&store).unwrap();

    let mut out = Vec::new();
    cli::logout(&Endpoints::at(&server.url()), &store, &mut out).unwrap();
    assert!(!store.exists());
    assert!(String::from_utf8(out).unwrap().contains("could not revoke"));
}
