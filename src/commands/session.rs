//! `login` and `logout`.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

use crate::credentials::Credentials;
use crate::twitch::{self, Client, Endpoints, oauth};

/// Signs in with the device-code flow and saves the credentials to `store`.
/// `open_url` is called with the activation page, e.g. [`open_in_browser`].
pub fn login(
    endpoints: &Endpoints,
    client_id: &str,
    store: &Path,
    out: &mut impl Write,
    open_url: impl FnOnce(&str),
) -> Result<()> {
    let agent = twitch::agent();
    let device = oauth::request_device_code(&agent, &endpoints.auth, client_id)?;
    writeln!(
        out,
        "To sign in, open {}\nand confirm the code {}.\n",
        device.verification_uri, device.user_code
    )?;
    out.flush()?;
    open_url(&device.verification_uri);
    writeln!(out, "Waiting for you to authorize in the browser...")?;
    out.flush()?;

    let token = oauth::poll_for_token(&agent, &endpoints.auth, client_id, &device)?;
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
            let revoked = oauth::revoke(
                &twitch::agent(),
                &endpoints.auth,
                &credentials.client_id,
                &credentials.access_token,
            );
            if let Err(error) = revoked {
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
