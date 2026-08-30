from __future__ import annotations

from collections.abc import Callable
from typing import final

import pytest

from twitch_watch import client as client_module
from twitch_watch.models import Stream


def make_stream(
    login: str,
    viewers: int,
    game: str = "Game",
    title: str = "Some title",
) -> Stream:
    return {
        "id": "1",
        "user_id": "10",
        "user_login": login,
        "user_name": login,
        "game_id": "1",
        "game_name": game,
        "type": "live",
        "title": title,
        "viewer_count": viewers,
        "started_at": "2026-01-01T00:00:00Z",
        "language": "en",
        "thumbnail_url": "https://example.com/thumb.jpg",
        "tags": ["English"],
        "is_mature": False,
    }


STREAMS = [make_stream("alice", 10), make_stream("bob", 200)]


@final
class FakeResponse:
    body: bytes

    def __init__(self, body: bytes) -> None:
        self.body = body

    def read(self) -> bytes:
        return self.body

    def close(self) -> None:
        pass


def patch_api_env(monkeypatch: pytest.MonkeyPatch, streams: list[Stream]) -> None:
    def fake_env(_name: str) -> str:
        return "x"

    def fake_user_id(_headers: dict[str, str]) -> str:
        return "user-1"

    def fake_get_live_followed(_headers: dict[str, str], _user_id: str) -> list[Stream]:
        return streams

    monkeypatch.setattr(client_module, "get_env_or_exit", fake_env)
    monkeypatch.setattr(client_module, "get_own_user_id", fake_user_id)
    monkeypatch.setattr(client_module, "get_live_followed", fake_get_live_followed)


def answering_input(answer: str) -> Callable[[object], str]:
    def fake_input(_prompt: object) -> str:
        return answer

    return fake_input
