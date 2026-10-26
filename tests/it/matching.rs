use twitch_watch::matching;
use twitch_watch::twitch::models::Stream;

const NONE: [&str; 0] = [];

fn stream(login: &str, game: &str, title: &str) -> Stream {
    serde_json::from_value(serde_json::json!({
        "user_login": login,
        "user_name": login,
        "game_name": game,
        "title": title,
    }))
    .unwrap()
}

fn filtered<'a>(streams: &[&'a Stream], query: &str) -> Vec<&'a str> {
    matching::filter(streams, query)
        .into_iter()
        .map(|i| streams[i].user_login.as_str())
        .collect()
}

#[test]
fn empty_query_keeps_everything_in_order() {
    let (a, b) = (stream("a", "", ""), stream("b", "", ""));
    assert_eq!(filtered(&[&a, &b], "  "), ["a", "b"]);
}

#[test]
fn scattered_letters_do_not_match() {
    // A subsequence matcher would match "bs" against "bob" + "Chess".
    let bob = stream("bob", "Chess", "");
    assert_eq!(filtered(&[&bob], "bs"), NONE);
    assert_eq!(filtered(&[&bob], "bc"), NONE);
}

#[test]
fn titles_are_not_searched() {
    let alice = stream("alice", "Chess", "bonanza speedrun");
    assert_eq!(filtered(&[&alice], "bon"), NONE);
    assert_eq!(filtered(&[&alice], "speedrun"), NONE);
}

#[test]
fn matching_is_case_insensitive() {
    let alice = stream("Alice", "Chess", "");
    assert_eq!(filtered(&[&alice], "aLi"), ["Alice"]);
    assert_eq!(filtered(&[&alice], "CHESS"), ["Alice"]);
}

#[test]
fn name_prefix_beats_name_substring_beats_game() {
    // Input order is by viewers; ranking must override it.
    let by_game = stream("carl", "Bob Ross painting", "");
    let substring = stream("xbox_bob", "", "");
    let prefix = stream("bobby", "", "");
    assert_eq!(
        filtered(&[&by_game, &substring, &prefix], "bob"),
        ["bobby", "xbox_bob", "carl"]
    );
}

#[test]
fn ties_keep_viewer_order() {
    let (first, second) = (stream("bob_a", "", ""), stream("bob_b", "", ""));
    assert_eq!(filtered(&[&first, &second], "bob"), ["bob_a", "bob_b"]);
}

#[test]
fn every_term_must_match() {
    let alice_chess = stream("alice", "Chess", "");
    let alice_go = stream("alicia", "Go", "");
    assert_eq!(filtered(&[&alice_chess, &alice_go], "ali chess"), ["alice"]);
    assert_eq!(filtered(&[&alice_chess], "ali poker"), NONE);
}
