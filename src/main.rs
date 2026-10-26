use std::io;
use std::process::ExitCode;

use anyhow::Result;
use clap::Parser;

use twitch_watch::cli::{self, Args};
use twitch_watch::client::{self, Client};

fn try_main(args: &Args) -> Result<()> {
    let client_id = client::require_env("TWITCH_CLIENT_ID")?;
    let token = client::require_env("TWITCH_ACCESS_TOKEN")?;
    let client = Client::new(client_id, token);
    cli::run(
        args,
        &client,
        &mut io::stdin().lock(),
        &mut io::stdout(),
        cli::launch_streamlink,
    )
}

fn main() -> ExitCode {
    let _ = dotenvy::dotenv();
    let args = Args::parse();
    match try_main(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}
