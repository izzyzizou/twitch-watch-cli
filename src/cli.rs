//! Command-line definitions.

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    version,
    about = "Pick a live followed Twitch channel and watch it via streamlink.",
    args_conflicts_with_subcommands = true
)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Cmd>,

    /// Channel to watch; a partial name narrows the list to matching channels
    pub channel: Option<String>,

    /// streamlink quality [default: `quality` from config.toml, else best]
    #[arg(short, long)]
    pub quality: Option<String>,

    /// Only list live channels, don't prompt or launch
    #[arg(long)]
    pub list: bool,
}

#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Sign in with your Twitch account and save the credentials
    Login {
        /// Client ID of your Twitch app [default: `TWITCH_CLIENT_ID`, else the saved login's]
        #[arg(long)]
        client_id: Option<String>,
    },
    /// Revoke and delete the saved credentials
    Logout,
}
