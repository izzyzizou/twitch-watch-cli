//! Showing streams and letting the user choose one.
//!
//! A picker is any `FnOnce(&[&Stream], &mut dyn Write) -> Result<Option<usize>>`
//! returning the chosen index, or `None` to quit: [`list::numbered_picker`]
//! when not attached to a terminal, [`picker::pick`] when interactive.

pub mod list;
pub mod picker;

use crate::twitch::models::Stream;

/// Drops control characters (titles are streamer-controlled and could carry
/// terminal escape sequences) and truncates by chars, not bytes.
fn clean_title(title: &str, max_chars: usize) -> String {
    title
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(max_chars)
        .collect()
}

/// `name  (game)  - N viewers`, shared by the list and the picker.
fn summary(stream: &Stream) -> String {
    let mut summary = stream.user_name.clone();
    if !stream.game_name.is_empty() {
        summary.push_str("  (");
        summary.push_str(&stream.game_name);
        summary.push(')');
    }
    summary.push_str("  - ");
    summary.push_str(&stream.viewer_count.to_string());
    summary.push_str(" viewers");
    summary
}
