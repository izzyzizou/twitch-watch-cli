//! Library behind the `twitch-watch` binary. It is split out so the
//! integration tests can drive each layer; it is not a stable public API.
//!
//! - [`cli`]: command-line definitions
//! - [`commands`]: what each subcommand does
//! - [`twitch`]: the Helix and OAuth HTTP clients
//! - [`credentials`], [`config`]: on-disk state
//! - [`matching`], [`ui`]: choosing a stream
//! - [`streamlink`]: launching the player

pub mod cli;
pub mod commands;
pub mod config;
pub mod credentials;
pub mod matching;
pub mod streamlink;
pub mod twitch;
pub mod ui;
