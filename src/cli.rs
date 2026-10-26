use std::io::{self, BufRead, Write};
use std::process::{Command, ExitCode, ExitStatus};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result, bail};
use clap::Parser;

use crate::client::Client;
use crate::models::Stream;

const TITLE_MAX_CHARS: usize = 80;

/// Set while streamlink is running, so Ctrl-C is left for it to handle.
static CHILD_RUNNING: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Parser)]
#[command(
    version,
    about = "Pick a live followed Twitch channel and watch it via streamlink."
)]
pub struct Args {
    /// streamlink quality
    #[arg(short, long, default_value = "best")]
    pub quality: String,

    /// only list live channels, don't prompt or launch
    #[arg(long)]
    pub list: bool,
}

/// Ctrl-C exits immediately, except while streamlink is running: it receives
/// the signal too, and we wait for it to shut down and report its exit code.
pub fn install_interrupt_handler() -> Result<()> {
    ctrlc::set_handler(|| {
        if !CHILD_RUNNING.load(Ordering::SeqCst) {
            eprintln!("\nStopped.");
            std::process::exit(130);
        }
    })
    .context("failed to install Ctrl-C handler")
}

pub fn print_streams(out: &mut impl Write, streams: &[Stream]) -> io::Result<()> {
    writeln!(out, "\nLive now:\n")?;
    for (i, stream) in streams.iter().enumerate() {
        write!(out, "  [{}] {}", i + 1, stream.user_name)?;
        if !stream.game_name.is_empty() {
            write!(out, "  ({})", stream.game_name)?;
        }
        writeln!(out, "  - {} viewers", stream.viewer_count)?;
        // Titles are streamer-controlled: drop control characters so they can't
        // inject terminal escape sequences, and truncate by chars, not bytes.
        let title: String = stream
            .title
            .trim()
            .chars()
            .filter(|c| !c.is_control())
            .take(TITLE_MAX_CHARS)
            .collect();
        if !title.is_empty() {
            writeln!(out, "        \"{title}\"")?;
        }
    }
    writeln!(out)
}

/// Prompts for a 1-based stream index. Returns `None` if the user quits.
pub fn select_stream(
    input: &mut impl BufRead,
    out: &mut impl Write,
    count: usize,
) -> Result<Option<usize>> {
    write!(out, "Select a stream to watch [1-{count}] (or q to quit): ")?;
    out.flush()?;
    let mut choice = String::new();
    if input.read_line(&mut choice)? == 0 {
        return Ok(None);
    }
    let choice = choice.trim();
    if choice.is_empty() || choice.eq_ignore_ascii_case("q") || choice.eq_ignore_ascii_case("quit")
    {
        return Ok(None);
    }
    match choice.parse::<usize>() {
        Ok(index) if (1..=count).contains(&index) => Ok(Some(index)),
        _ => bail!("Invalid selection."),
    }
}

pub fn launch_streamlink(url: &str, quality: &str) -> Result<ExitCode> {
    launch("streamlink", url, quality)
}

/// Runs `program url quality`, waiting for it and passing through its exit code.
pub fn launch(program: &str, url: &str, quality: &str) -> Result<ExitCode> {
    CHILD_RUNNING.store(true, Ordering::SeqCst);
    let status = Command::new(program).arg(url).arg(quality).status();
    CHILD_RUNNING.store(false, Ordering::SeqCst);
    match status {
        Ok(status) => Ok(exit_code(status)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            bail!("{program} not found. Make sure it's installed and on your PATH.")
        }
        Err(error) => Err(error).with_context(|| format!("failed to launch {program}")),
    }
}

fn exit_code(status: ExitStatus) -> ExitCode {
    match status.code() {
        Some(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        // Terminated by a signal.
        None => ExitCode::FAILURE,
    }
}

pub fn run(
    args: &Args,
    client: &Client,
    input: &mut impl BufRead,
    out: &mut impl Write,
    launch: impl FnOnce(&str, &str) -> Result<ExitCode>,
) -> Result<ExitCode> {
    let user_id = client.own_user_id()?;
    let streams = client.live_followed(&user_id)?;

    if streams.is_empty() {
        writeln!(out, "No followed channels are live right now.")?;
        return Ok(ExitCode::SUCCESS);
    }

    print_streams(out, &streams)?;

    if args.list {
        return Ok(ExitCode::SUCCESS);
    }

    let Some(index) = select_stream(input, out, streams.len())? else {
        return Ok(ExitCode::SUCCESS);
    };

    let url = format!("https://twitch.tv/{}", streams[index - 1].user_login);
    writeln!(out, "\nLaunching streamlink: {url} [{}]\n", args.quality)?;
    out.flush()?;
    launch(&url, &args.quality)
}
