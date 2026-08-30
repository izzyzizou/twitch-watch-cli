# twitch-watch-cli

Lightweight terminal tool: lists your currently-live followed Twitch channels,
lets you pick one, and launches it in streamlink.

## Requirements

- Python 3 (standard library only, no pip installs needed)
- [streamlink](https://streamlink.github.io/) installed and on your `PATH`

## Setup

```sh
python3 -m venv .venv
source .venv/bin/activate

export TWITCH_CLIENT_ID="your_client_id"
export TWITCH_ACCESS_TOKEN="your_access_token"
```

`TWITCH_USER_ID` is not needed — it is fetched automatically from your token.

> Get a client ID and access token via the [Twitch Developer
> Console](https://dev.twitch.tv/console/apps) and an
> [OAuth token](https://twitchtokengenerator.com/).

## Usage

```sh
python3 -m twitch_watch                   # list live followed channels, pick one, watch (best quality)
python3 -m twitch_watch -q 720p           # pick a specific streamlink quality
python3 -m twitch_watch --list            # just print the list, don't prompt/launch
```

Or install as a package and use the `twitch-watch` command:

```sh
pip install -e .
twitch-watch --list
```

## Options

| Flag                | Description                                   |
| ------------------- | --------------------------------------------- |
| `-q, --quality`     | streamlink quality (default: `best`)          |
| `--list`            | only list live channels, don't prompt/launch  |

## Development

```sh
python3 -m venv .venv
.venv/bin/pip install -r requirements-dev.txt
.venv/bin/pytest
.venv/bin/ruff check src tests
.venv/bin/mypy
.venv/bin/basedpyright
```