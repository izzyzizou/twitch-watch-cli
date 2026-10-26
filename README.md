# twitch-watch-cli

Lightweight terminal tool: lists your currently-live followed Twitch channels,
lets you pick one, and launches it in streamlink.

## Requirements

- [Rust](https://www.rust-lang.org/tools/install) (to build)
- [streamlink](https://streamlink.github.io/) installed and on your `PATH`

## Setup

Copy the sample env file and fill in your credentials:

```sh
cp .env.sample .env
# edit .env and set TWITCH_CLIENT_ID and TWITCH_ACCESS_TOKEN
```

Your `.env` should look like this:

```sh
TWITCH_CLIENT_ID=your_client_id
TWITCH_ACCESS_TOKEN=your_access_token
```

They are loaded automatically from a `.env` file in the current directory (or any
parent), or you can export them in your shell instead.
`TWITCH_USER_ID` is not needed — it is fetched automatically from your token.

> Get an access token (and match it with the client ID it was issued to) from
> [Twitch Token Generator](https://twitchtokengenerator.com/) — the generated
> token page shows both the access token and its client ID — or register your
> own app via the [Twitch Developer Console](https://dev.twitch.tv/console/apps).
> When generating the token, select the `user:read:follows` scope.

## Usage

```sh
cargo run --release                       # list live followed channels, pick one, watch (best quality)
cargo run --release -- -q 720p            # pick a specific streamlink quality
cargo run --release -- --list             # just print the list, don't prompt/launch
```

Or install the `twitch-watch` binary onto your `PATH`:

```sh
cargo install --path .
twitch-watch --list
```

## Options

| Flag                | Description                                   |
| ------------------- | --------------------------------------------- |
| `-q, --quality`     | streamlink quality (default: `best`)          |
| `--list`            | only list live channels, don't prompt/launch  |

## Development

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```
