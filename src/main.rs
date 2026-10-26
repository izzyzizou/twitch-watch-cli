use std::io::{self, IsTerminal};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Parser;

use twitch_watch::auth::{self, Credentials};
use twitch_watch::cli::{self, Args, Cmd};
use twitch_watch::client::{Client, Endpoints};
use twitch_watch::config::{self, Config, Paths};

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
    let paths = Paths::in_dir(&config::dir(config::env_var)?);
    let endpoints = Endpoints::twitch();
    let mut stdout = io::stdout().lock();

    match &args.command {
        Some(Cmd::Login { client_id }) => {
            let client_id = client_id
                .clone()
                .or_else(|| config::env_var("TWITCH_CLIENT_ID"))
                .or_else(|| {
                    Credentials::load(&paths.credentials)
                        .ok()
                        .flatten()
                        .map(|credentials| credentials.client_id)
                })
                .context(
                    "Pass your Twitch app's client ID: `twitch-watch login --client-id <id>` (see README).",
                )?;
            let interactive = io::stdout().is_terminal();
            cli::login(
                &endpoints,
                &client_id,
                &paths.credentials,
                &mut stdout,
                |url| {
                    if interactive {
                        cli::open_in_browser(url);
                    }
                },
            )?;
            if config::env_var("TWITCH_ACCESS_TOKEN").is_some() {
                eprintln!(
                    "Note: TWITCH_ACCESS_TOKEN is set (in your environment or .env) and takes precedence over this login."
                );
            }
            Ok(ExitCode::SUCCESS)
        }
        Some(Cmd::Logout) => {
            cli::logout(&endpoints, &paths.credentials, &mut stdout)?;
            Ok(ExitCode::SUCCESS)
        }
        None => {
            let config = Config::load(&paths.config)?;
            let (credentials, store) = auth::resolve(config::env_var, &paths.credentials)?;
            let client = Client::new(endpoints, credentials, store);
            let quality = args
                .quality
                .clone()
                .or(config.quality)
                .unwrap_or_else(|| "best".to_owned());
            if io::stdin().is_terminal() && io::stderr().is_terminal() {
                cli::run(
                    args,
                    &quality,
                    &client,
                    &mut stdout,
                    cli::fuzzy_picker,
                    cli::launch_streamlink,
                )
            } else {
                let picker = cli::numbered_picker(io::stdin().lock());
                cli::run(
                    args,
                    &quality,
                    &client,
                    &mut stdout,
                    picker,
                    cli::launch_streamlink,
                )
            }
        }
    }
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
