from __future__ import annotations

import subprocess
import sys

import pytest

from tests.conftest import STREAMS, answering_input, patch_api_env
from twitch_watch import cli as module


def test_main_no_live_streams(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    patch_api_env(monkeypatch, [])
    monkeypatch.setattr(sys, "argv", ["twitch-watch"])
    module.main()
    assert "No followed channels are live right now." in capsys.readouterr().out


def test_main_list_flag_does_not_prompt(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    patch_api_env(monkeypatch, STREAMS)
    monkeypatch.setattr(sys, "argv", ["twitch-watch", "--list"])

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
    patch_api_env(monkeypatch, STREAMS)
    monkeypatch.setattr(sys, "argv", ["twitch-watch", "-q", "720p"])
    monkeypatch.setattr("builtins.input", answering_input("2"))
    calls: list[object] = []

    def fake_run(cmd: object, *_args: object, **_kwargs: object) -> None:
        calls.append(cmd)

    monkeypatch.setattr(subprocess, "run", fake_run)
    module.main()
    assert calls == [["streamlink", "https://twitch.tv/bob", "720p"]]
    assert "https://twitch.tv/bob [720p]" in capsys.readouterr().out


def test_main_quit_does_not_launch(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    patch_api_env(monkeypatch, STREAMS)
    monkeypatch.setattr(sys, "argv", ["twitch-watch"])
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
    patch_api_env(monkeypatch, STREAMS)
    monkeypatch.setattr(sys, "argv", ["twitch-watch"])
    monkeypatch.setattr("builtins.input", answering_input("99"))
    with pytest.raises(SystemExit) as exc:
        module.main()
    assert exc.value.code == 1
    assert "Invalid selection." in capsys.readouterr().err


def test_main_streamlink_not_found(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    patch_api_env(monkeypatch, STREAMS)
    monkeypatch.setattr(sys, "argv", ["twitch-watch"])
    monkeypatch.setattr("builtins.input", answering_input("1"))

    def fake_run(_cmd: object, *_args: object, **_kwargs: object) -> None:
        raise FileNotFoundError

    monkeypatch.setattr(subprocess, "run", fake_run)
    with pytest.raises(SystemExit) as exc:
        module.main()
    assert exc.value.code == 1
    assert "streamlink not found" in capsys.readouterr().err
