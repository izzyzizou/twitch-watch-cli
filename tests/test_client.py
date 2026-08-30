from __future__ import annotations

import io
import urllib.error
import urllib.request
from email.message import Message

import pytest

from tests.conftest import FakeResponse, make_stream
from twitch_watch import client as module
from twitch_watch.models import Stream


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

    def fake_get_all_pages(
        _path: str, _headers: dict[str, str], _params: dict[str, str]
    ) -> list[Stream]:
        return [low, high]

    monkeypatch.setattr(module, "get_all_pages", fake_get_all_pages)
    result = module.get_live_followed({}, "u")
    assert [stream["user_login"] for stream in result] == ["high", "low"]
