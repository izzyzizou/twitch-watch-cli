use std::io::{self, BufRead, Write};
use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::Parser;

use crate::client::Client;
use crate::models::Stream;

#[derive(Debug, Parser)]
#[command(about = "Pick a live followed Twitch channel and watch it via streamlink.")]
pub struct Args {
    /// streamlink quality
    #[arg(short, long, default_value = "best")]
    pub quality: String,

    /// only list live channels, don't prompt or launch
    #[arg(long)]
    pub list: bool,
}

pub fn print_streams(out: &mut impl Write, streams: &[Stream]) -> io::Result<()> {
    writeln!(out, "\nLive now:\n")?;
    for (i, stream) in streams.iter().enumerate() {
        let mut line = format!("  [{}] {}", i + 1, stream.user_name);
        if !stream.game_name.is_empty() {
            line.push_str(&format!("  ({})", stream.game_name));
        }
        line.push_str(&format!("  - {} viewers", stream.viewer_count));
        writeln!(out, "{line}")?;
        let title = stream.title.trim();
        if !title.is_empty() {
            // Truncate by characters, not bytes, so multibyte titles don't panic.
            let short: String = title.chars().take(80).collect();
            writeln!(out, "        \"{short}\"")?;
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
    if matches!(choice.to_lowercase().as_str(), "q" | "quit" | "") {
        return Ok(None);
    }
    match choice.parse::<usize>() {
        Ok(index) if (1..=count).contains(&index) => Ok(Some(index)),
        _ => bail!("Invalid selection."),
    }
}

pub fn launch_streamlink(url: &str, quality: &str) -> Result<()> {
    match Command::new("streamlink").arg(url).arg(quality).status() {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            bail!("streamlink not found. Make sure it's installed and on your PATH.")
        }
        Err(error) => Err(error).context("failed to launch streamlink"),
    }
}

pub fn run(
    args: &Args,
    client: &Client,
    input: &mut impl BufRead,
    out: &mut impl Write,
    launch: impl FnOnce(&str, &str) -> Result<()>,
) -> Result<()> {
    let user_id = client.own_user_id()?;
    let streams = client.live_followed(&user_id)?;

    if streams.is_empty() {
        writeln!(out, "No followed channels are live right now.")?;
        return Ok(());
    }

    print_streams(out, &streams)?;

    if args.list {
        return Ok(());
    }

    let Some(index) = select_stream(input, out, streams.len())? else {
        return Ok(());
    };

    let url = format!("https://twitch.tv/{}", streams[index - 1].user_login);
    writeln!(out, "\nLaunching streamlink: {url} [{}]\n", args.quality)?;
    out.flush()?;
    launch(&url, &args.quality)
}
