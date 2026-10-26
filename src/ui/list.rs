//! Plain-text output: the numbered list and its selection prompt.

use std::io::{self, BufRead, Write};

use anyhow::{Result, bail};

use super::{sanitize, summary};
use crate::twitch::models::Stream;

const TITLE_MAX_CHARS: usize = 80;

pub fn print_streams(out: &mut dyn Write, streams: &[&Stream]) -> io::Result<()> {
    writeln!(out, "\nLive now:\n")?;
    for (i, stream) in streams.iter().enumerate() {
        writeln!(out, "  [{}] {}", i + 1, summary(stream))?;
        let title = sanitize(&stream.title, TITLE_MAX_CHARS);
        if !title.is_empty() {
            writeln!(out, "        \"{title}\"")?;
        }
    }
    writeln!(out)
}

/// Prompts for a stream by number. Returns its 0-based index, or `None` to quit.
pub fn select_stream(
    input: &mut impl BufRead,
    out: &mut dyn Write,
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
        Ok(number) if (1..=count).contains(&number) => Ok(Some(number - 1)),
        _ => bail!("Invalid selection."),
    }
}

/// A picker that prints the numbered list and reads the choice from `input`.
pub fn numbered_picker(
    mut input: impl BufRead,
) -> impl FnOnce(&[&Stream], &mut dyn Write) -> Result<Option<usize>> {
    move |streams, out| {
        print_streams(out, streams)?;
        select_stream(&mut input, out, streams.len())
    }
}
