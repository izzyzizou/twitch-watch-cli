use clap::Parser;

use twitch_watch::cli::{Args, Cmd};

#[test]
fn channel_and_quality() {
    let args = Args::parse_from(["twitch-watch", "shroud", "-q", "720p"]);
    assert_eq!(args.channel.as_deref(), Some("shroud"));
    assert_eq!(args.quality.as_deref(), Some("720p"));
    assert!(args.command.is_none());
}

#[test]
fn login_subcommand() {
    let args = Args::parse_from(["twitch-watch", "login", "--client-id", "abc"]);
    assert!(matches!(args.command, Some(Cmd::Login { client_id: Some(ref id) }) if id == "abc"));
}

#[test]
fn watch_flags_conflict_with_subcommands() {
    assert!(Args::try_parse_from(["twitch-watch", "login", "--list"]).is_err());
}
