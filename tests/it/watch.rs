use std::cell::RefCell;
use std::io::{Cursor, Write};
use std::process::ExitCode;

use serde_json::Value;

use crate::common::{client_for, mock_api, stream_json, stream_json_with};
use twitch_watch::commands::watch::{self, WatchOptions};
use twitch_watch::twitch::models::Stream;
use twitch_watch::ui::list;

const NO_URLS: [&str; 0] = [];

const ALL: WatchOptions = WatchOptions {
    channel: None,
    list: false,
    quality: "best",
};

struct Outcome {
    result: anyhow::Result<ExitCode>,
    out: String,
    launches: Vec<(String, String)>,
}

impl Outcome {
    fn urls(&self) -> Vec<&str> {
        self.launches.iter().map(|(url, _)| url.as_str()).collect()
    }
}

fn run_with_picker(
    options: &WatchOptions,
    streams: &[Value],
    pick: impl FnOnce(&[&Stream], &mut dyn Write) -> anyhow::Result<Option<usize>>,
) -> Outcome {
    let server = mock_api(streams);
    let mut out = Vec::new();
    let launches = RefCell::new(Vec::new());
    let result = watch::run(
        options,
        &client_for(&server),
        &mut out,
        pick,
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

fn run_with_input(options: &WatchOptions, streams: &[Value], input: &str) -> Outcome {
    run_with_picker(
        options,
        streams,
        list::numbered_picker(Cursor::new(input.to_owned())),
    )
}

fn never_pick(_: &[&Stream], _: &mut dyn Write) -> anyhow::Result<Option<usize>> {
    panic!("picker should not be shown");
}

fn listing() -> WatchOptions<'static> {
    WatchOptions { list: true, ..ALL }
}

fn channel(query: &str) -> WatchOptions<'_> {
    WatchOptions {
        channel: Some(query),
        ..ALL
    }
}

fn two_streams() -> Vec<Value> {
    vec![stream_json("alice", 10), stream_json("bob", 200)]
}

#[test]
fn no_live_streams() {
    let outcome = run_with_input(&ALL, &[], "");
    outcome.result.unwrap();
    assert!(
        outcome
            .out
            .contains("No followed channels are live right now.")
    );
}

#[test]
fn list_does_not_prompt() {
    let outcome = run_with_picker(&listing(), &two_streams(), never_pick);
    outcome.result.as_ref().unwrap();
    // Sorted by viewers, so bob comes first.
    assert!(outcome.out.contains("[1] bob"));
    assert!(outcome.out.contains("[2] alice"));
    assert_eq!(outcome.urls(), NO_URLS);
}

#[test]
fn launches_choice_and_passes_through_exit_code() {
    let options = WatchOptions {
        quality: "720p",
        ..ALL
    };
    let outcome = run_with_input(&options, &two_streams(), "1\n");
    assert_eq!(outcome.result.as_ref().unwrap(), &ExitCode::from(7));
    assert_eq!(
        outcome.launches,
        [("https://twitch.tv/bob".to_owned(), "720p".to_owned())]
    );
    assert!(outcome.out.contains("https://twitch.tv/bob [720p]"));
}

#[test]
fn quit_does_not_launch() {
    for input in ["q\n", "QUIT\n", "\n", ""] {
        let outcome = run_with_input(&ALL, &two_streams(), input);
        outcome.result.as_ref().unwrap();
        assert_eq!(outcome.urls(), NO_URLS, "input {input:?} launched");
    }
}

#[test]
fn invalid_selection_errors() {
    for input in ["99\n", "0\n", "abc\n"] {
        let outcome = run_with_input(&ALL, &two_streams(), input);
        assert_eq!(
            outcome.result.as_ref().unwrap_err().to_string(),
            "Invalid selection."
        );
        assert_eq!(outcome.urls(), NO_URLS);
    }
}

#[test]
fn long_multibyte_title_is_truncated_by_chars() {
    let title = "é".repeat(100);
    let streams = [stream_json_with("carol", 1, "", &title)];
    let outcome = run_with_picker(&listing(), &streams, never_pick);
    outcome.result.as_ref().unwrap();
    assert!(outcome.out.contains(&format!("\"{}\"", "é".repeat(80))));
    assert!(!outcome.out.contains("()"), "empty game should be omitted");
}

#[test]
fn title_control_characters_are_stripped() {
    let streams = [stream_json_with("dave", 1, "Game", "hi\u{1b}[2Jthere")];
    let outcome = run_with_picker(&listing(), &streams, never_pick);
    outcome.result.as_ref().unwrap();
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
        let outcome = run_with_picker(&channel(query), &two_streams(), never_pick);
        outcome.result.as_ref().unwrap();
        assert_eq!(outcome.urls(), ["https://twitch.tv/bob"], "query {query:?}");
    }
}

#[test]
fn exact_match_wins_over_partial_matches() {
    let streams = [stream_json("bobby", 500), stream_json("bob", 1)];
    let outcome = run_with_picker(&channel("bob"), &streams, never_pick);
    outcome.result.as_ref().unwrap();
    assert_eq!(outcome.urls(), ["https://twitch.tv/bob"]);
}

#[test]
fn single_partial_match_launches() {
    let outcome = run_with_picker(&channel("ob"), &two_streams(), never_pick);
    outcome.result.as_ref().unwrap();
    assert_eq!(outcome.urls(), ["https://twitch.tv/bob"]);
}

#[test]
fn several_partial_matches_are_offered_in_the_picker() {
    let streams = [
        stream_json("alice", 10),
        stream_json("bob", 200),
        stream_json("alicia", 50),
    ];
    let offered = RefCell::new(Vec::new());
    let outcome = run_with_picker(&channel("ali"), &streams, |candidates, _| {
        *offered.borrow_mut() = candidates.iter().map(|s| s.user_login.clone()).collect();
        Ok(Some(1))
    });
    outcome.result.as_ref().unwrap();
    assert_eq!(offered.into_inner(), ["alicia", "alice"]);
    assert_eq!(outcome.urls(), ["https://twitch.tv/alice"]);
}

#[test]
fn unmatched_channel_errors() {
    let outcome = run_with_picker(&channel("zed"), &two_streams(), never_pick);
    assert_eq!(
        outcome.result.unwrap_err().to_string(),
        "None of your live followed channels match \"zed\"."
    );
}

#[test]
fn channel_with_list_prints_only_matches() {
    let options = WatchOptions {
        list: true,
        ..channel("ali")
    };
    let outcome = run_with_picker(&options, &two_streams(), never_pick);
    outcome.result.as_ref().unwrap();
    assert!(outcome.out.contains("[1] alice"));
    assert!(!outcome.out.contains("bob"));
}

#[test]
fn out_of_range_pick_is_an_error_not_a_panic() {
    let outcome = run_with_picker(&ALL, &two_streams(), |_, _| Ok(Some(5)));
    assert!(outcome.result.is_err());
    assert_eq!(outcome.urls(), NO_URLS);
}
