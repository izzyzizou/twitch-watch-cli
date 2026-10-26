//! The default command: list live followed channels, choose one, watch it.

use std::io::{self, Write};
use std::process::ExitCode;

use anyhow::{Result, bail};

use crate::matching;
use crate::twitch::Client;
use crate::twitch::models::Stream;
use crate::ui::list;

#[derive(Debug, Clone, Copy)]
pub struct WatchOptions<'a> {
    /// Channel name, or part of one, to narrow the choice to.
    pub channel: Option<&'a str>,
    /// Only print the (matching) live channels.
    pub list: bool,
    pub quality: &'a str,
}

/// Runs the watch flow. `pick` chooses among candidates (see [`crate::ui`]);
/// `launch` plays a URL at a quality and returns the player's exit code.
pub fn run(
    options: &WatchOptions,
    client: &Client,
    out: &mut impl Write,
    pick: impl FnOnce(&[&Stream], &mut dyn Write) -> Result<Option<usize>>,
    launch: impl FnOnce(&str, &str) -> Result<ExitCode>,
) -> Result<ExitCode> {
    let user_id = client.authenticated_user_id()?;
    let streams = client.live_followed(&user_id)?;
    if streams.is_empty() {
        writeln!(out, "No followed channels are live right now.")?;
        return Ok(ExitCode::SUCCESS);
    }

    let candidates = match options.channel {
        Some(query) => {
            let matches = matching::find_channels(&streams, query);
            if matches.is_empty() {
                bail!("None of your live followed channels match \"{query}\".");
            }
            matches
        }
        None => streams.iter().collect(),
    };

    if options.list {
        list::print_streams(out, &candidates)?;
        return Ok(ExitCode::SUCCESS);
    }

    let Some(stream) = choose(options, &candidates, out, pick)? else {
        return Ok(ExitCode::SUCCESS);
    };
    let url = format!("https://twitch.tv/{}", stream.user_login);
    writeln!(out, "\nLaunching streamlink: {url} [{}]\n", options.quality)?;
    out.flush()?;
    launch(&url, options.quality)
}

/// A channel argument with a single match is used without prompting.
fn choose<'a>(
    options: &WatchOptions,
    candidates: &[&'a Stream],
    out: &mut dyn Write,
    pick: impl FnOnce(&[&Stream], &mut dyn Write) -> Result<Option<usize>>,
) -> Result<Option<&'a Stream>> {
    if let [only] = candidates
        && options.channel.is_some()
    {
        return Ok(Some(only));
    }
    let choice = pick(candidates, out)?;
    choice
        .map(|index| {
            candidates
                .get(index)
                .copied()
                .ok_or_else(|| io::Error::other("picker returned an out-of-range index").into())
        })
        .transpose()
}
