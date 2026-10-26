use std::process::ExitCode;

use twitch_watch::streamlink;

#[test]
fn missing_program_is_explained() {
    let error = streamlink::launch("twitch-watch-no-such-program", "url", "best").unwrap_err();
    assert_eq!(
        error.to_string(),
        "twitch-watch-no-such-program not found. Make sure it's installed and on your PATH."
    );
}

#[test]
fn exit_code_is_passed_through() {
    // `true`/`false` ignore their arguments.
    assert_eq!(
        streamlink::launch("true", "url", "best").unwrap(),
        ExitCode::SUCCESS
    );
    assert_eq!(
        streamlink::launch("false", "url", "best").unwrap(),
        ExitCode::FAILURE
    );
}
