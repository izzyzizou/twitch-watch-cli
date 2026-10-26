//! Showing streams and letting the user choose one.
//!
//! A picker is any `FnOnce(&[&Stream], &mut dyn Write) -> Result<Option<usize>>`
//! returning the chosen index, or `None` to quit: [`list::numbered_picker`]
//! when not attached to a terminal, [`picker::pick`] when interactive.

pub mod list;
pub mod picker;

use crate::twitch::models::Stream;

/// Makes API text safe to print: drops control characters (all of it is
/// remote-controlled and could carry terminal escape sequences) and
/// truncates by chars, not bytes.
fn sanitize(text: &str, max_chars: usize) -> String {
    text.trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(max_chars)
        .collect()
}

/// `name  (game)  - N viewers`, shared by the list and the picker.
fn summary(stream: &Stream) -> String {
    let mut summary = sanitize(&stream.user_name, usize::MAX);
    let game = sanitize(&stream.game_name, usize::MAX);
    if !game.is_empty() {
        summary.push_str("  (");
        summary.push_str(&game);
        summary.push(')');
    }
    summary.push_str("  - ");
    summary.push_str(&stream.viewer_count.to_string());
    summary.push_str(" viewers");
    summary
}
