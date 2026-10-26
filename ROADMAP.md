# Roadmap

Planned improvements, roughly in priority order. None of these are committed
to a date; open an issue or PR if you want to pick one up.

## Quality of life

- **Return to the list when a stream ends.** After streamlink exits, show the
  picker again instead of quitting, so you can hop between channels in one
  session. Esc/q still exits.
- **Richer list.** Show uptime (Helix's `started_at`) and compact viewer counts
  (`12.3k`), with colour that switches off when output isn't a terminal or
  `NO_COLOR` is set.
- **Pass-through streamlink options.** `twitch-watch -- --player mpv
  --twitch-low-latency`, plus `player` / `streamlink_args` / `low_latency` keys
  in `config.toml`.
- **`--chat`.** Also open the channel's popout chat
  (`https://www.twitch.tv/popout/<login>/chat`) in the browser.
- **Filters.** `--game <name>` and `--lang <code>` to narrow the list.

## Scripting and integrations

- **`--json` output.** Machine-readable list of live channels for status bars
  (tmux, waybar, sketchybar) and scripts.
- **`watch` mode.** Poll every few minutes and send a desktop notification when
  a followed channel goes live.

## Auth

- **OS keychain storage.** Optionally keep credentials in the macOS Keychain,
  Windows Credential Manager or Secret Service instead of `credentials.json`.
- **Bundled client ID.** Ship a default public client ID for official release
  builds so `twitch-watch login` works without registering an app.

## Distribution

- **Shell completions and man page.** Generate them from the clap definitions
  with `clap_complete` and `clap_mangen`.
- **Prebuilt binaries.** Publish release binaries for macOS, Linux and Windows
  via `cargo-dist` on tagged releases, plus a Homebrew tap, so users don't need
  a Rust toolchain.
- **Windows CI.** Add `windows-latest` to the CI matrix (a couple of tests rely
  on Unix `true`/`false` and file modes and need gating).
