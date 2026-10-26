# Security policy

## Reporting a vulnerability

Please report vulnerabilities privately through
[GitHub's private vulnerability reporting](https://github.com/izzyzizou/twitch-watch-cli/security/advisories/new)
rather than in a public issue. Include steps to reproduce and the version or
commit affected. You can expect an initial response within a week.

## Supported versions

Only the latest commit on `main` is supported.

## How credentials are handled

- `twitch-watch login` stores the access and refresh tokens in
  `credentials.json` in the config directory (`~/.config/twitch-watch/` by
  default). On Unix the file is created with mode `0600` and its directory
  with `0700`. Anyone who can read it can act as your Twitch account with the
  `user:read:follows` scope, so don't share or commit it.
- Tokens are only sent to Twitch over HTTPS (`api.twitch.tv`, `id.twitch.tv`),
  in headers or form bodies, never in URLs.
- `twitch-watch logout` revokes the token with Twitch and deletes the file.
- From a `.env` file, only `TWITCH_CLIENT_ID` and `TWITCH_ACCESS_TOKEN` are
  read, and they are not exported to the environment of child processes.
- Text from the Twitch API (names, games, titles) is stripped of control
  characters before being printed.
