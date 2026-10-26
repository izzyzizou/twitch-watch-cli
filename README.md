# twitch-watch-cli

Lightweight terminal tool: lists your currently-live followed Twitch channels,
lets you pick one, and launches it in streamlink.

## Requirements

- [Rust](https://www.rust-lang.org/tools/install) 1.88+ (to build)
- [streamlink](https://streamlink.github.io/) installed and on your `PATH`

## Install

```sh
cargo install --path .
```

## Setup

### 1. Register a Twitch app (once)

In the [Twitch Developer Console](https://dev.twitch.tv/console/apps), click
**Register Your Application** and fill in:

- **Name**: anything unique, e.g. `twitch-watch-<your name>`
- **OAuth Redirect URLs**: `http://localhost` (required by the form, not used)
- **Category**: Application Integration
- **Client Type**: **Public**. Sign-in only works with public clients.

Copy the app's **Client ID**.

### 2. Log in

```sh
twitch-watch login --client-id <your client id>
```

This opens the Twitch activation page and shows a code to confirm. Once you
approve, the credentials are saved to `~/.config/twitch-watch/credentials.json`
(readable only by you) and refreshed automatically when they expire. To log in
again later, `twitch-watch login` is enough because the client ID is remembered.

`twitch-watch logout` revokes the token and deletes the file.

### Alternative: environment variables

For CI or scripts, you can skip `login` and provide a token directly, either
exported or in a `.env` file in the current directory (or a parent; see
`.env.sample`):

```sh
TWITCH_CLIENT_ID=your_client_id
TWITCH_ACCESS_TOKEN=your_access_token
```

The token needs the `user:read:follows` scope. When `TWITCH_ACCESS_TOKEN` is set
it takes precedence over the saved login, and it is not refreshed automatically.

## Usage

```sh
twitch-watch                  # pick from your live followed channels and watch
twitch-watch shroud           # watch a channel directly (also @name or a twitch.tv URL)
twitch-watch sh               # partial name: launches if one channel matches, else narrows the picker
twitch-watch -q 720p          # pick a specific streamlink quality
twitch-watch --list           # just print the list, don't prompt/launch
```

In a terminal, channels are shown in a picker: type to filter, arrow keys (or
Tab) to move, Enter to watch, Esc to quit. The filter matches each typed word
against channel names and games (not titles), case-insensitively. Names
starting with what you typed come first, then names containing it, then game
matches. When input or output is redirected, a numbered list and prompt are
used instead.

After launching, `twitch-watch` waits for streamlink and exits with its exit
code, so failures (e.g. an unavailable quality) are visible to scripts.

## Options

| Flag / command      | Description                                              |
| ------------------- | -------------------------------------------------------- |
| `[CHANNEL]`         | channel to watch, or part of a name to narrow the list   |
| `-q, --quality`     | streamlink quality (default: config `quality`, else `best`) |
| `--list`            | only list live channels, don't prompt/launch             |
| `login`             | sign in and save credentials (`--client-id <id>`)        |
| `logout`            | revoke and delete the saved credentials                  |
| `-V, --version`     | print version                                            |

## Configuration

Optional settings go in `~/.config/twitch-watch/config.toml`:

```toml
quality = "720p60"   # default streamlink quality; --quality overrides it
```

The directory is `$XDG_CONFIG_HOME/twitch-watch` if that variable is set
(`%APPDATA%\twitch-watch` on Windows), and `TWITCH_WATCH_CONFIG_DIR` overrides
it entirely.

## Development

```sh
cargo test
cargo clippy --all-targets -- -D warnings   # pedantic lints are enabled in Cargo.toml
cargo fmt --check
```

Layout:

| Path               | Responsibility                                          |
| ------------------ | ------------------------------------------------------- |
| `src/main.rs`      | process setup, dispatch, exit codes                     |
| `src/cli.rs`       | command-line definitions                                |
| `src/commands/`    | `watch`, `login`/`logout` logic                         |
| `src/twitch/`      | Helix client, OAuth requests, response models           |
| `src/credentials.rs`, `src/config.rs` | on-disk state                        |
| `src/matching.rs`  | channel-argument and picker filtering rules             |
| `src/ui/`          | numbered list/prompt and interactive picker             |
| `src/streamlink.rs`| launching streamlink, Ctrl-C handling                   |
| `tests/it/`        | integration tests (one binary; modules mirror `src/`)   |

See [ROADMAP.md](ROADMAP.md) for planned improvements.
