from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import urllib.error
import urllib.request
from http.client import HTTPResponse
from typing import Generic, TypedDict, TypeVar, cast

API_BASE = "https://api.twitch.tv/helix"

T = TypeVar("T")


class Pagination(TypedDict):
    cursor: str | None


class ApiResponse(TypedDict, Generic[T]):
    data: list[T]
    pagination: Pagination | None


class Stream(TypedDict):
    id: str
    user_id: str
    user_login: str
    user_name: str
    game_id: str
    game_name: str
    type: str
    title: str
    viewer_count: int
    started_at: str
    language: str
    thumbnail_url: str
    tags: list[str]
    is_mature: bool


class User(TypedDict):
    id: str
    login: str
    display_name: str
    type: str
    broadcaster_type: str
    description: str
    profile_image_url: str
    offline_image_url: str
    view_count: int
    created_at: str


def get_env_or_exit(name: str) -> str:
    val = os.environ.get(name)
    if not val:
        print(f"Missing required environment variable: {name}", file=sys.stderr)
        sys.exit(1)
    return val


def api_get(path: str, headers: dict[str, str], params: dict[str, str] | None = None) -> object:
    url = f"{API_BASE}{path}"
    if params:
        query = "&".join(f"{k}={v}" for k, v in params.items())
        url = f"{url}?{query}"
    req = urllib.request.Request(url, headers=headers)
    try:
        response = cast(HTTPResponse, urllib.request.urlopen(req))
        try:
            return cast(object, json.loads(response.read().decode()))
        finally:
            response.close()
    except urllib.error.HTTPError as e:
        print(f"API error {e.code}: {e.read().decode()}", file=sys.stderr)
        sys.exit(1)


def get_all_pages(path: str, headers: dict[str, str], params: dict[str, str]) -> list[T]:
    results: list[T] = []
    cursor: str | None = None
    while True:
        p = dict(params)
        if cursor:
            p["after"] = cursor
        payload = cast(ApiResponse[T], api_get(path, headers, p))
        results.extend(payload["data"])
        pagination = payload.get("pagination")
        cursor = pagination["cursor"] if pagination else None
        if not cursor:
            break
    return results


def get_own_user_id(headers: dict[str, str]) -> str:
    payload = cast(ApiResponse[User], api_get("/users", headers))
    users = payload["data"]
    if not users:
        print("Could not resolve user ID from access token.", file=sys.stderr)
        sys.exit(1)
    return users[0]["id"]


def get_live_followed(headers: dict[str, str], user_id: str) -> list[Stream]:
    streams: list[Stream] = get_all_pages(
        "/streams/followed", headers, {"user_id": user_id, "first": "100"}
    )
    streams.sort(key=lambda s: s["viewer_count"], reverse=True)
    return streams


class CliArgs(argparse.Namespace):
    quality: str = "best"
    list: bool = False


def main() -> None:
    parser = argparse.ArgumentParser(description="Pick a live followed Twitch channel and watch it via streamlink.")
    _ = parser.add_argument("-q", "--quality", default="best", help="streamlink quality (default: best)")
    _ = parser.add_argument("--list", action="store_true", help="only list live channels, don't prompt or launch")
    args = parser.parse_args(namespace=CliArgs())

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
        _ = subprocess.run(["streamlink", url, args.quality], check=False)
    except FileNotFoundError:
        print("streamlink not found. Make sure it's installed and on your PATH.", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\nStopped.")
        sys.exit(0)
