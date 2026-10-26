use std::io;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Parser;

use twitch_watch::cli::{self, Args};
use twitch_watch::client::{self, Client};

fn load_dotenv() -> Result<()> {
    match dotenvy::dotenv() {
        Ok(_) => Ok(()),
        Err(error) if error.not_found() => Ok(()),
        Err(error) => Err(error).context("failed to load .env"),
    }
}

fn try_main(args: &Args) -> Result<ExitCode> {
    load_dotenv()?;
    cli::install_interrupt_handler()?;
    let client_id = client::require_env("TWITCH_CLIENT_ID")?;
    let token = client::require_env("TWITCH_ACCESS_TOKEN")?;
    let client = Client::new(client_id, &token);
    cli::run(
        args,
        &client,
        &mut io::stdin().lock(),
        &mut io::stdout().lock(),
        cli::launch_streamlink,
    )
}

/// True when stdout was closed early, e.g. `twitch-watch --list | head -1`.
fn is_broken_pipe(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<io::Error>()
        .is_some_and(|error| error.kind() == io::ErrorKind::BrokenPipe)
}

fn main() -> ExitCode {
    let args = Args::parse();
    match try_main(&args) {
        Ok(code) => code,
        Err(error) if is_broken_pipe(&error) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}
