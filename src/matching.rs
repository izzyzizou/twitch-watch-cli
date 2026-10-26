//! Matching user input against live streams, for both the channel argument
//! and the interactive picker's filter.
//!
//! Matching is deliberately strict: text must appear contiguously
//! (case-insensitive) in the channel name or game. Titles are never searched,
//! so typing a name narrows to that channel instead of to every row that
//! happens to contain the same letters.

use crate::twitch::models::Stream;

/// Streams matching a channel argument: an exact login/display-name match if
/// there is one, otherwise every stream whose name contains the query.
/// Accepts `name`, `@name`, `twitch.tv/name` or a full channel URL.
#[must_use]
pub fn find_channels<'a>(streams: &'a [Stream], query: &str) -> Vec<&'a Stream> {
    let query = normalize_channel(query);
    let exact = streams.iter().find(|stream| {
        stream.user_login.to_lowercase() == query || stream.user_name.to_lowercase() == query
    });
    if let Some(stream) = exact {
        return vec![stream];
    }
    streams
        .iter()
        .filter(|stream| {
            stream.user_login.to_lowercase().contains(&query)
                || stream.user_name.to_lowercase().contains(&query)
        })
        .collect()
}

fn normalize_channel(query: &str) -> String {
    let query = query.trim();
    let query = query
        .strip_prefix("https://")
        .or_else(|| query.strip_prefix("http://"))
        .unwrap_or(query);
    let query = query.strip_prefix("www.").unwrap_or(query);
    let query = query.strip_prefix("twitch.tv/").unwrap_or(query);
    query
        .trim_start_matches('@')
        .trim_end_matches('/')
        .to_lowercase()
}

/// How a stream matched a filter; better matches sort first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Rank {
    NamePrefix,
    Name,
    Game,
}

/// Indices of `streams` matching the picker's filter text, best first.
///
/// Every whitespace-separated term must occur in the channel's login, display
/// name or game. Streams whose name starts with every term rank first, then
/// those whose name contains them, then game matches; within a rank, the
/// input (viewer-count) order is kept.
#[must_use]
pub fn filter(streams: &[&Stream], query: &str) -> Vec<usize> {
    let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let mut matches: Vec<(Rank, usize)> = streams
        .iter()
        .enumerate()
        .filter_map(|(i, stream)| rank(stream, &terms).map(|rank| (rank, i)))
        .collect();
    // Stable, so ties keep their input order.
    matches.sort_by_key(|&(rank, _)| rank);
    matches.into_iter().map(|(_, i)| i).collect()
}

/// The weakest rank among all terms, or `None` if any term doesn't match.
fn rank(stream: &Stream, terms: &[String]) -> Option<Rank> {
    let login = stream.user_login.to_lowercase();
    let name = stream.user_name.to_lowercase();
    let game = stream.game_name.to_lowercase();
    terms.iter().try_fold(Rank::NamePrefix, |worst, term| {
        let term = term.as_str();
        let rank = if login.starts_with(term) || name.starts_with(term) {
            Rank::NamePrefix
        } else if login.contains(term) || name.contains(term) {
            Rank::Name
        } else if game.contains(term) {
            Rank::Game
        } else {
            return None;
        };
        Some(worst.max(rank))
    })
}
