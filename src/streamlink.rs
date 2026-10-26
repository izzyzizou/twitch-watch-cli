//! Launching streamlink, and Ctrl-C handling around it.

use std::io;
use std::process::{Command, ExitCode, ExitStatus};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result, bail};

/// Set while streamlink is running, so Ctrl-C is left for it to handle.
static CHILD_RUNNING: AtomicBool = AtomicBool::new(false);

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

/// Plays `url` at `quality` with streamlink.
pub fn watch(url: &str, quality: &str) -> Result<ExitCode> {
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
