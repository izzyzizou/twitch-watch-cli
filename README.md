# twitch-watch-cli

Lightweight terminal tool: lists your currently-live followed Twitch channels,
lets you pick one, and launches it in streamlink.

## Requirements

- Python 3
- [streamlink](https://streamlink.github.io/) installed and on your `PATH`

## Setup

Copy the sample env file and fill in your credentials:

```sh
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt

cp .env.sample .env
# edit .env and set TWITCH_CLIENT_ID and TWITCH_ACCESS_TOKEN
```

Your `.env` should look like this:

```sh
TWITCH_CLIENT_ID=your_client_id
TWITCH_ACCESS_TOKEN=your_access_token
```

They are loaded automatically via [python-dotenv](https://github.com/theskumar/python-dotenv).
`TWITCH_USER_ID` is not needed — it is fetched automatically from your token.

> Get an access token (and match it with the client ID it was issued to) from
> [Twitch Token Generator](https://twitchtokengenerator.com/) — the generated
> token page shows both the access token and its client ID — or register your
> own app via the [Twitch Developer Console](https://dev.twitch.tv/console/apps).
> When generating the token, select the `user:read:follows` scope.

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