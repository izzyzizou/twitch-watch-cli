use std::io::{self, IsTerminal};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Parser;

use twitch_watch::cli::{Args, Cmd};
use twitch_watch::commands::session;
use twitch_watch::commands::watch::{self, WatchOptions};
use twitch_watch::config::{self, Config, Env, Paths};
use twitch_watch::credentials::{self, Credentials};
use twitch_watch::streamlink;
use twitch_watch::twitch::{Client, Endpoints};
use twitch_watch::ui::{list, picker};

const DEFAULT_QUALITY: &str = "best";

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

fn try_main(args: &Args) -> Result<ExitCode> {
    let env = Env::load()?;
    streamlink::install_interrupt_handler()?;
    // The config location comes from the real environment only, never `.env`.
    let paths = Paths::in_dir(&config::dir(config::env_var)?);
    match &args.command {
        Some(Cmd::Login { client_id }) => login(client_id.as_deref(), &env, &paths),
        Some(Cmd::Logout) => {
            session::logout(
                &Endpoints::twitch(),
                &paths.credentials,
                &mut io::stdout().lock(),
            )?;
            Ok(ExitCode::SUCCESS)
        }
        None => watch(args, &env, &paths),
    }
}

fn login(client_id: Option<&str>, env: &Env, paths: &Paths) -> Result<ExitCode> {
    let client_id = client_id
        .map(str::to_owned)
        .or_else(|| env.var("TWITCH_CLIENT_ID"))
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
    session::login(
        &Endpoints::twitch(),
        &client_id,
        &paths.credentials,
        &mut io::stdout().lock(),
        |url| {
            if interactive {
                session::open_in_browser(url);
            }
        },
    )?;
    if env.var("TWITCH_ACCESS_TOKEN").is_some() {
        eprintln!(
            "Note: TWITCH_ACCESS_TOKEN is set (in your environment or .env) and takes precedence over this login."
        );
    }
    Ok(ExitCode::SUCCESS)
}

fn watch(args: &Args, env: &Env, paths: &Paths) -> Result<ExitCode> {
    let config = Config::load(&paths.config)?;
    let (credentials, store) = credentials::resolve(|name| env.var(name), &paths.credentials)?;
    let client = Client::new(Endpoints::twitch(), credentials, store);
    let quality = args
        .quality
        .as_deref()
        .or(config.quality.as_deref())
        .unwrap_or(DEFAULT_QUALITY);
    let options = WatchOptions {
        channel: args.channel.as_deref(),
        list: args.list,
        quality,
    };
    let mut stdout = io::stdout().lock();
    if io::stdin().is_terminal() && io::stderr().is_terminal() {
        watch::run(
            &options,
            &client,
            &mut stdout,
            picker::pick,
            streamlink::watch,
        )
    } else {
        let numbered = list::numbered_picker(io::stdin().lock());
        watch::run(&options, &client, &mut stdout, numbered, streamlink::watch)
    }
}

/// True when stdout was closed early, e.g. `twitch-watch --list | head -1`.
fn is_broken_pipe(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<io::Error>()
        .is_some_and(|error| error.kind() == io::ErrorKind::BrokenPipe)
}
