from __future__ import annotations

import io
import subprocess
import sys
import urllib.error
import urllib.request
from collections.abc import Callable
from email.message import Message
from typing import final

import pytest

import main as module
from main import Stream


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


def _patch_api_env(monkeypatch: pytest.MonkeyPatch, streams: list[Stream]) -> None:
    def fake_env(_name: str) -> str:
        return "x"

    def fake_user_id(_headers: dict[str, str]) -> str:
        return "user-1"

    def fake_get_live_followed(_headers: dict[str, str], _user_id: str) -> list[Stream]:
        return streams

    monkeypatch.setattr(module, "get_env_or_exit", fake_env)
    monkeypatch.setattr(module, "get_own_user_id", fake_user_id)
    monkeypatch.setattr(module, "get_live_followed", fake_get_live_followed)


def answering_input(answer: str) -> Callable[[object], str]:
    def fake_input(_prompt: object) -> str:
        return answer

    return fake_input


def test_get_env_or_exit_returns_value(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("TWITCH_CLIENT_ID", "abc")
    assert module.get_env_or_exit("TWITCH_CLIENT_ID") == "abc"


def test_get_env_or_exit_missing_exits(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    monkeypatch.delenv("TWITCH_CLIENT_ID", raising=False)
    with pytest.raises(SystemExit) as exc:
        _ = module.get_env_or_exit("TWITCH_CLIENT_ID")
    assert exc.value.code == 1
    assert "Missing required environment variable" in capsys.readouterr().err


def test_api_get_parses_json_and_builds_query(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    requests: list[urllib.request.Request] = []

    def fake_urlopen(
        req: urllib.request.Request, *_args: object, **_kwargs: object
    ) -> FakeResponse:
        requests.append(req)
        return FakeResponse(b'{"data": [{"id": "1"}]}')

    monkeypatch.setattr(urllib.request, "urlopen", fake_urlopen)
    result = module.api_get("/users", {"Client-Id": "cid"}, {"a": "b", "c": "d"})
    assert result == {"data": [{"id": "1"}]}
    assert requests[0].full_url == "https://api.twitch.tv/helix/users?a=b&c=d"
    assert requests[0].headers["Client-id"] == "cid"


def test_api_get_exits_on_http_error(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    def fake_urlopen(
        _req: urllib.request.Request, *_args: object, **_kwargs: object
    ) -> FakeResponse:
        raise urllib.error.HTTPError(
            "https://api.twitch.tv/helix/x",
            404,
            "Not Found",
            Message(),
            io.BytesIO(b'{"error": "nope"}'),
        )

    monkeypatch.setattr(urllib.request, "urlopen", fake_urlopen)
    with pytest.raises(SystemExit) as exc:
        _ = module.api_get("/x", {"Client-Id": "cid"})
    assert exc.value.code == 1
    assert "API error 404" in capsys.readouterr().err


def test_get_all_pages_follows_cursor(monkeypatch: pytest.MonkeyPatch) -> None:
    calls: list[dict[str, str]] = []

    def fake_api_get(
        _path: str, _headers: dict[str, str], params: dict[str, str]
    ) -> object:
        calls.append(params)
        if "after" in params:
            return {"data": [{"n": 2}], "pagination": None}
        return {"data": [{"n": 1}], "pagination": {"cursor": "next"}}

    monkeypatch.setattr(module, "api_get", fake_api_get)
    result: list[dict[str, int]] = module.get_all_pages(
        "/streams/followed", {}, {"user_id": "u", "first": "100"}
    )
    assert result == [{"n": 1}, {"n": 2}]
    assert calls[0] == {"user_id": "u", "first": "100"}
    assert calls[1] == {"user_id": "u", "first": "100", "after": "next"}


def test_get_own_user_id_returns_id(monkeypatch: pytest.MonkeyPatch) -> None:
    def fake_api_get(
        _path: str, _headers: dict[str, str], _params: dict[str, str] | None = None
    ) -> object:
        return {"data": [{"id": "42"}]}

    monkeypatch.setattr(module, "api_get", fake_api_get)
    assert module.get_own_user_id({"Client-Id": "cid"}) == "42"


def test_get_own_user_id_empty_exits(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    def fake_api_get(
        _path: str, _headers: dict[str, str], _params: dict[str, str] | None = None
    ) -> object:
        return {"data": []}

    monkeypatch.setattr(module, "api_get", fake_api_get)
    with pytest.raises(SystemExit) as exc:
        _ = module.get_own_user_id({})
    assert exc.value.code == 1
    assert "Could not resolve user ID" in capsys.readouterr().err


def test_get_live_followed_sorts_by_viewers(monkeypatch: pytest.MonkeyPatch) -> None:
    low = make_stream("low", 5)
    high = make_stream("high", 500)

    def fake_get_all_pages(*_args: object, **_kwargs: object) -> list[Stream]:
        return [low, high]

    monkeypatch.setattr(module, "get_all_pages", fake_get_all_pages)
    result = module.get_live_followed({}, "u")
    assert [s["user_login"] for s in result] == ["high", "low"]


def test_main_no_live_streams(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    _patch_api_env(monkeypatch, [])
    monkeypatch.setattr(sys, "argv", ["main.py"])
    module.main()
    assert "No followed channels are live right now." in capsys.readouterr().out


def test_main_list_flag_does_not_prompt(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    _patch_api_env(monkeypatch, STREAMS)
    monkeypatch.setattr(sys, "argv", ["main.py", "--list"])

    def never_input(_prompt: object) -> str:
        raise AssertionError("input() should not be called with --list")

    monkeypatch.setattr("builtins.input", never_input)
    module.main()
    out = capsys.readouterr().out
    assert "[1] alice" in out
    assert "[2] bob" in out


def test_main_launches_streamlink(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    _patch_api_env(monkeypatch, STREAMS)
    monkeypatch.setattr(sys, "argv", ["main.py", "-q", "720p"])
    monkeypatch.setattr("builtins.input", answering_input("2"))
    calls: list[object] = []

    def fake_run(cmd: object, *_args: object, **_kwargs: object) -> None:
        calls.append(cmd)

    monkeypatch.setattr(subprocess, "run", fake_run)
    module.main()
    assert calls == [["streamlink", "https://twitch.tv/bob", "720p"]]
    assert "https://twitch.tv/bob [720p]" in capsys.readouterr().out


def test_main_quit_does_not_launch(monkeypatch: pytest.MonkeyPatch) -> None:
    _patch_api_env(monkeypatch, STREAMS)
    monkeypatch.setattr(sys, "argv", ["main.py"])
    monkeypatch.setattr("builtins.input", answering_input("q"))
    calls: list[object] = []

    def fake_run(cmd: object, *_args: object, **_kwargs: object) -> None:
        calls.append(cmd)

    monkeypatch.setattr(subprocess, "run", fake_run)
    module.main()
    assert calls == []


def test_main_invalid_selection_exits(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    _patch_api_env(monkeypatch, STREAMS)
    monkeypatch.setattr(sys, "argv", ["main.py"])
    monkeypatch.setattr("builtins.input", answering_input("99"))
    with pytest.raises(SystemExit) as exc:
        module.main()
    assert exc.value.code == 1
    assert "Invalid selection." in capsys.readouterr().err


def test_main_streamlink_not_found(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    _patch_api_env(monkeypatch, STREAMS)
    monkeypatch.setattr(sys, "argv", ["main.py"])
    monkeypatch.setattr("builtins.input", answering_input("1"))

    def fake_run(_cmd: object, *_args: object, **_kwargs: object) -> None:
        raise FileNotFoundError

    monkeypatch.setattr(subprocess, "run", fake_run)
    with pytest.raises(SystemExit) as exc:
        module.main()
    assert exc.value.code == 1
    assert "streamlink not found" in capsys.readouterr().err