use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::{Command, ExitCode, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use dialoguer::FuzzySelect;
use dialoguer::theme::ColorfulTheme;

use crate::auth::{self, Credentials};
use crate::client::{self, Client, Endpoints};
use crate::models::Stream;

const TITLE_MAX_CHARS: usize = 80;
const PICKER_TITLE_MAX_CHARS: usize = 60;
const PICKER_ROWS: usize = 15;

/// Set while streamlink is running, so Ctrl-C is left for it to handle.
static CHILD_RUNNING: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Parser)]
#[command(
    version,
    about = "Pick a live followed Twitch channel and watch it via streamlink.",
    args_conflicts_with_subcommands = true
)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Cmd>,

    /// Channel to watch; a partial name narrows the list to matching channels
    pub channel: Option<String>,

    /// streamlink quality [default: `quality` from config.toml, else best]
    #[arg(short, long)]
    pub quality: Option<String>,

    /// only list live channels, don't prompt or launch
    #[arg(long)]
    pub list: bool,
}

#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Sign in with your Twitch account and save the credentials
    Login {
        /// Client ID of your Twitch app [default: TWITCH_CLIENT_ID, else the saved login's]
        #[arg(long)]
        client_id: Option<String>,
    },
    /// Revoke and delete the saved credentials
    Logout,
}

/// Ctrl-C exits immediately, except while streamlink is running: it receives
/// the signal too, and we wait for it to shut down and report its exit code.
pub fn install_interrupt_handler() -> Result<()> {
    ctrlc::set_handler(|| {
        if !CHILD_RUNNING.load(Ordering::SeqCst) {
            let _ = console::Term::stderr().show_cursor();
            eprintln!("\nStopped.");
            std::process::exit(130);
        }
    })
    .context("failed to install Ctrl-C handler")
}

/// Titles are streamer-controlled: drop control characters so they can't
/// inject terminal escape sequences, and truncate by chars, not bytes.
fn clean_title(title: &str, max_chars: usize) -> String {
    title
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(max_chars)
        .collect()
}

pub fn print_streams(out: &mut dyn Write, streams: &[&Stream]) -> io::Result<()> {
    writeln!(out, "\nLive now:\n")?;
    for (i, stream) in streams.iter().enumerate() {
        write!(out, "  [{}] {}", i + 1, stream.user_name)?;
        if !stream.game_name.is_empty() {
            write!(out, "  ({})", stream.game_name)?;
        }
        writeln!(out, "  - {} viewers", stream.viewer_count)?;
        let title = clean_title(&stream.title, TITLE_MAX_CHARS);
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

/// A picker that prints a numbered list and reads the choice from `input`;
/// used when not attached to a terminal.
pub fn numbered_picker(
    mut input: impl BufRead,
) -> impl FnOnce(&[&Stream], &mut dyn Write) -> Result<Option<usize>> {
    move |streams, out| {
        print_streams(out, streams)?;
        select_stream(&mut input, out, streams.len())
    }
}

/// An interactive type-to-filter picker, drawn on stderr.
pub fn fuzzy_picker(streams: &[&Stream], _out: &mut dyn Write) -> Result<Option<usize>> {
    let items: Vec<String> = streams.iter().map(|stream| picker_label(stream)).collect();
    let choice = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Live now (type to filter, Enter to watch, Esc to quit)")
        .items(&items)
        .default(0)
        .max_length(PICKER_ROWS)
        .interact_opt();
    match choice {
        Ok(choice) => Ok(choice),
        // Ctrl-C arrives as a key press in the picker's raw mode.
        Err(dialoguer::Error::IO(error)) if error.kind() == io::ErrorKind::Interrupted => {
            let _ = console::Term::stderr().show_cursor();
            Ok(None)
        }
        Err(error) => Err(error).context("stream picker failed"),
    }
}

fn picker_label(stream: &Stream) -> String {
    let mut label = stream.user_name.clone();
    if !stream.game_name.is_empty() {
        label.push_str(&format!("  ({})", stream.game_name));
    }
    label.push_str(&format!("  - {} viewers", stream.viewer_count));
    let title = clean_title(&stream.title, PICKER_TITLE_MAX_CHARS);
    if !title.is_empty() {
        label.push_str(&format!("  \"{title}\""));
    }
    label
}

/// Accepts `name`, `@name`, `twitch.tv/name` or a full channel URL.
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

/// Streams matching `query`: an exact login/display-name match if there is
/// one, otherwise every stream whose name contains it.
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
    quality: &str,
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

    let candidates = match &args.channel {
        Some(query) => {
            let matches = find_channels(&streams, query);
            if matches.is_empty() {
                bail!("None of your live followed channels match \"{query}\".");
            }
            matches
        }
        None => streams.iter().collect(),
    };

    if args.list {
        print_streams(out, &candidates)?;
        return Ok(ExitCode::SUCCESS);
    }

    // A channel argument with a single match launches without prompting.
    let stream = if let [only] = candidates[..]
        && args.channel.is_some()
    {
        only
    } else {
        match pick(&candidates, out)? {
            Some(index) => candidates[index],
            None => return Ok(ExitCode::SUCCESS),
        }
    };

    let url = format!("https://twitch.tv/{}", stream.user_login);
    writeln!(out, "\nLaunching streamlink: {url} [{quality}]\n")?;
    out.flush()?;
    launch(&url, quality)
}

/// Best-effort: opens `url` in the default browser.
pub fn open_in_browser(url: &str) {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    let _ = Command::new(program)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Signs in with the device-code flow and saves the credentials to `store`.
pub fn login(
    endpoints: &Endpoints,
    client_id: &str,
    store: &Path,
    out: &mut impl Write,
    open_url: impl FnOnce(&str),
) -> Result<()> {
    let agent = client::agent();
    let device = auth::request_device_code(&agent, &endpoints.auth, client_id)?;
    writeln!(
        out,
        "To sign in, open {}\nand confirm the code {}.\n",
        device.verification_uri, device.user_code
    )?;
    out.flush()?;
    open_url(&device.verification_uri);
    writeln!(out, "Waiting for you to authorize in the browser...")?;
    out.flush()?;

    let token = auth::poll_for_token(&agent, &endpoints.auth, client_id, &device)?;
    let credentials = Credentials {
        client_id: client_id.to_owned(),
        access_token: token.access_token,
        refresh_token: token.refresh_token,
    };
    let validation = Client::new(endpoints.clone(), credentials.clone(), None).validate()?;
    credentials.save(store)?;
    writeln!(
        out,
        "Logged in as {}. Credentials saved to {}.",
        validation.login,
        store.display()
    )?;
    Ok(())
}

/// Revokes the saved token with Twitch (best effort) and deletes it locally.
pub fn logout(endpoints: &Endpoints, store: &Path, out: &mut impl Write) -> Result<()> {
    match Credentials::load(store) {
        Ok(None) => {
            writeln!(out, "Not logged in.")?;
            return Ok(());
        }
        Ok(Some(credentials)) => {
            if let Err(error) = auth::revoke(&client::agent(), &endpoints.auth, &credentials) {
                writeln!(
                    out,
                    "Warning: could not revoke the token with Twitch: {error:#}"
                )?;
            }
        }
        // An unreadable file is still removed below.
        Err(_) => {}
    }
    fs::remove_file(store).with_context(|| format!("failed to delete {}", store.display()))?;
    writeln!(out, "Logged out; removed {}.", store.display())?;
    Ok(())
}
