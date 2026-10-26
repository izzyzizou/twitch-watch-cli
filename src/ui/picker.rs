//! Interactive type-to-filter picker, drawn on stderr. Filtering rules live
//! in [`crate::matching::filter`].

use std::io::{self, Write};

use anyhow::{Context, Result};
use console::{Key, Term, style, truncate_str};

use super::{clean_title, summary};
use crate::matching;
use crate::twitch::models::Stream;

const ROWS: usize = 15;
const TITLE_MAX_CHARS: usize = 60;

fn label(stream: &Stream) -> String {
    let title = clean_title(&stream.title, TITLE_MAX_CHARS);
    if title.is_empty() {
        summary(stream)
    } else {
        format!("{}  \"{title}\"", summary(stream))
    }
}

/// Shows the picker and returns the chosen index into `streams`, or `None`
/// if the user quits with Esc or Ctrl-C. Draws on stderr, not `_out`, so
/// stdout stays clean for the final "Launching" line.
pub fn pick(streams: &[&Stream], _out: &mut dyn Write) -> Result<Option<usize>> {
    let term = Term::stderr();
    let result = term.hide_cursor().and_then(|()| interact(&term, streams));
    let _ = term.show_cursor();
    result.context("stream picker failed")
}

fn interact(term: &Term, streams: &[&Stream]) -> io::Result<Option<usize>> {
    let labels: Vec<String> = streams.iter().map(|stream| label(stream)).collect();
    let mut query = String::new();
    let mut selected = 0;
    let mut drawn = 0;
    loop {
        let matches = matching::filter(streams, &query);
        selected = selected.min(matches.len().saturating_sub(1));
        term.clear_last_lines(drawn)?;
        drawn = draw(term, &query, &matches, &labels, selected)?;

        let key = match term.read_key() {
            Ok(key) => key,
            // Ctrl-C arrives as an error while the terminal is in raw mode.
            Err(error) if error.kind() == io::ErrorKind::Interrupted => Key::Escape,
            Err(error) => return Err(error),
        };
        match key {
            Key::Enter => {
                if let Some(&index) = matches.get(selected) {
                    term.clear_last_lines(drawn)?;
                    return Ok(Some(index));
                }
            }
            Key::Escape => {
                term.clear_last_lines(drawn)?;
                return Ok(None);
            }
            Key::ArrowUp | Key::BackTab => {
                selected = selected
                    .checked_sub(1)
                    .unwrap_or(matches.len().saturating_sub(1));
            }
            Key::ArrowDown | Key::Tab => {
                selected = if selected + 1 >= matches.len() {
                    0
                } else {
                    selected + 1
                };
            }
            Key::Backspace => {
                query.pop();
                selected = 0;
            }
            Key::Char(c) if !c.is_control() => {
                query.push(c);
                selected = 0;
            }
            _ => {}
        }
    }
}

/// Draws the prompt, the visible window of matches and a status line.
/// Returns the number of lines written, for clearing on the next redraw.
fn draw(
    term: &Term,
    query: &str,
    matches: &[usize],
    labels: &[String],
    selected: usize,
) -> io::Result<usize> {
    // Lines must never wrap, or clearing the previous frame leaves debris.
    let width = usize::from(term.size().1).max(20) - 1;
    let mut lines = vec![format!(
        "{} {query}{}",
        style("Live now ›").for_stderr().bold(),
        style("▏").for_stderr().dim()
    )];
    let start = selected.saturating_sub(ROWS - 1);
    for (position, &index) in matches.iter().enumerate().skip(start).take(ROWS) {
        let text = truncate_str(&labels[index], width - 2, "…");
        if position == selected {
            lines.push(format!(
                "{} {}",
                style("❯").for_stderr().cyan().bold(),
                style(text).for_stderr().cyan()
            ));
        } else {
            lines.push(format!("  {text}"));
        }
    }
    if matches.is_empty() {
        lines.push(
            style("  no matching channels")
                .for_stderr()
                .dim()
                .to_string(),
        );
    }
    lines.push(
        style(format!(
            "  {}/{} · ↑↓ move · Enter watch · Esc quit",
            matches.len(),
            labels.len()
        ))
        .for_stderr()
        .dim()
        .to_string(),
    );
    for line in &lines {
        term.write_line(&truncate_str(line, width, "…"))?;
    }
    Ok(lines.len())
}
