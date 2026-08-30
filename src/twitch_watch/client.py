import json
import os
import sys
import urllib.error
import urllib.request
from http.client import HTTPResponse
from typing import TypeVar, cast

from dotenv import load_dotenv

from twitch_watch.models import ApiResponse, Stream, User

API_BASE = "https://api.twitch.tv/helix"

T = TypeVar("T")


_ = load_dotenv()


def get_env_or_exit(name: str) -> str:
    value = os.environ.get(name)
    if not value:
        print(f"Missing required environment variable: {name}", file=sys.stderr)
        sys.exit(1)
    return value


def api_get(
    path: str, headers: dict[str, str], params: dict[str, str] | None = None
) -> object:
    url = f"{API_BASE}{path}"
    if params:
        query = "&".join(f"{key}={value}" for key, value in params.items())
        url = f"{url}?{query}"
    req = urllib.request.Request(url, headers=headers)
    try:
        response = cast(HTTPResponse, urllib.request.urlopen(req))
        try:
            return cast(object, json.loads(response.read().decode()))
        finally:
            response.close()
    except urllib.error.HTTPError as error:
        print(f"API error {error.code}: {error.read().decode()}", file=sys.stderr)
        sys.exit(1)


def get_all_pages(
    path: str, headers: dict[str, str], params: dict[str, str]
) -> list[T]:
    results: list[T] = []
    cursor: str | None = None
    while True:
        page_params = dict(params)
        if cursor:
            page_params["after"] = cursor
        payload = cast(ApiResponse[T], api_get(path, headers, page_params))
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
    streams.sort(key=lambda stream: stream["viewer_count"], reverse=True)
    return streams
