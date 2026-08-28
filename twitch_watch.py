#!/usr/bin/env python3
"""
twitch_watch.py

Lightweight terminal tool: lists your currently-live followed Twitch
channels, lets you pick one, and launches it in streamlink.

REQUIREMENTS:
    - streamlink installed and on your PATH (https://streamlink.github.io/)
    - Python 3 (standard library only, no pip installs needed)

SETUP (one-time):
    export TWITCH_CLIENT_ID="your_client_id"
    export TWITCH_ACCESS_TOKEN="your_access_token"

    (No need to set TWITCH_USER_ID manually — this script fetches it
    automatically from your token.)

USAGE:
    python3 twitch_watch.py                 # list live followed channels, pick one, watch (best quality)
    python3 twitch_watch.py -q 720p         # pick a specific streamlink quality
    python3 twitch_watch.py --list          # just print the list, don't prompt/launch
"""

import os
import sys
import json
import argparse
import subprocess
import urllib.request
import urllib.error

API_BASE = "https://api.twitch.tv/helix"


def get_env_or_exit(name):
    val = os.environ.get(name)
    if not val:
        print(f"Missing required environment variable: {name}", file=sys.stderr)
        sys.exit(1)
    return val


def api_get(path, headers, params=None):
    url = f"{API_BASE}{path}"
    if params:
        query = "&".join(f"{k}={v}" for k, v in params.items())
        url = f"{url}?{query}"
    req = urllib.request.Request(url, headers=headers)
    try:
        with urllib.request.urlopen(req) as resp:
            return json.loads(resp.read().decode())
    except urllib.error.HTTPError as e:
        print(f"API error {e.code}: {e.read().decode()}", file=sys.stderr)
        sys.exit(1)


def get_all_pages(path, headers, params):
    results = []
    cursor = None
    while True:
        p = dict(params)
        if cursor:
            p["after"] = cursor
        payload = api_get(path, headers, p)
        results.extend(payload.get("data", []))
        cursor = payload.get("pagination", {}).get("cursor")
        if not cursor:
            break
    return results


def get_own_user_id(headers):
    payload = api_get("/users", headers)
    data = payload.get("data", [])
    if not data:
        print("Could not resolve user ID from access token.", file=sys.stderr)
        sys.exit(1)
    return data[0]["id"]


def get_live_followed(headers, user_id):
    streams = get_all_pages(
        "/streams/followed", headers, {"user_id": user_id, "first": 100}
    )
    streams.sort(key=lambda s: s.get("viewer_count", 0), reverse=True)
    return streams


def main():
    parser = argparse.ArgumentParser(description="Pick a live followed Twitch channel and watch it via streamlink.")
    parser.add_argument("-q", "--quality", default="best", help="streamlink quality (default: best)")
    parser.add_argument("--list", action="store_true", help="only list live channels, don't prompt or launch")
    args = parser.parse_args()

    client_id = get_env_or_exit("TWITCH_CLIENT_ID")
    token = get_env_or_exit("TWITCH_ACCESS_TOKEN")
    headers = {"Client-Id": client_id, "Authorization": f"Bearer {token}"}

    user_id = get_own_user_id(headers)
    streams = get_live_followed(headers, user_id)

    if not streams:
        print("No followed channels are live right now.")
        return

    print("\nLive now:\n")
    for i, s in enumerate(streams, start=1):
        name = s["user_name"]
        game = s.get("game_name", "")
        title = s.get("title", "").strip()
        viewers = s.get("viewer_count", 0)
        line = f"  [{i}] {name}"
        if game:
            line += f"  ({game})"
        line += f"  - {viewers} viewers"
        print(line)
        if title:
            print(f"        \"{title[:80]}\"")
    print()

    if args.list:
        return

    choice = input(f"Select a stream to watch [1-{len(streams)}] (or q to quit): ").strip()
    if choice.lower() in ("q", "quit", ""):
        return

    try:
        idx = int(choice)
        if not (1 <= idx <= len(streams)):
            raise ValueError
    except ValueError:
        print("Invalid selection.", file=sys.stderr)
        sys.exit(1)

    login = streams[idx - 1]["user_login"]
    url = f"https://twitch.tv/{login}"
    print(f"\nLaunching streamlink: {url} [{args.quality}]\n")

    try:
        subprocess.run(["streamlink", url, args.quality])
    except FileNotFoundError:
        print("streamlink not found. Make sure it's installed and on your PATH.", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\nStopped.")
        sys.exit(0)
