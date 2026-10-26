mod common;

use std::cell::RefCell;
use std::io::Cursor;
use std::process::ExitCode;

use clap::Parser;

use common::{mock_api, stream_json, stream_json_with};
use twitch_watch::cli::{self, Args};
use twitch_watch::client::Client;

struct Outcome {
    result: anyhow::Result<ExitCode>,
    out: String,
    launches: Vec<(String, String)>,
}

fn run_with(argv: &[&str], streams: Vec<serde_json::Value>, input: &str) -> Outcome {
    let server = mock_api(streams);
    let client = Client::with_base_url(server.url(), "cid", "tok");
    let args = Args::parse_from(argv);
    let mut out = Vec::new();
    let launches = RefCell::new(Vec::new());
    let result = cli::run(
        &args,
        &client,
        &mut Cursor::new(input.as_bytes()),
        &mut out,
        |url, quality| {
            launches
                .borrow_mut()
                .push((url.to_owned(), quality.to_owned()));
            Ok(ExitCode::from(7))
        },
    );
    Outcome {
        result,
        out: String::from_utf8(out).unwrap(),
        launches: launches.into_inner(),
    }
}

fn two_streams() -> Vec<serde_json::Value> {
    vec![stream_json("alice", 10), stream_json("bob", 200)]
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
    let outcome = run_with(&["twitch-watch", "--list"], two_streams(), "1\n");
    outcome.result.unwrap();
    // Sorted by viewers, so bob comes first.
    assert!(outcome.out.contains("[1] bob"));
    assert!(outcome.out.contains("[2] alice"));
    assert!(!outcome.out.contains("Select a stream"));
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
