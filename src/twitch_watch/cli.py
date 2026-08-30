import argparse
import subprocess
import sys

from twitch_watch import client
from twitch_watch.models import Stream


class CliArgs(argparse.Namespace):
    quality: str = "best"
    list: bool = False


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Pick a live followed Twitch channel and watch it via streamlink."
    )
    _ = parser.add_argument(
        "-q", "--quality", default="best", help="streamlink quality (default: best)"
    )
    _ = parser.add_argument(
        "--list",
        action="store_true",
        help="only list live channels, don't prompt or launch",
    )
    return parser


def print_streams(streams: list[Stream]) -> None:
    print("\nLive now:\n")
    for i, stream in enumerate(streams, start=1):
        name = stream["user_name"]
        game = stream.get("game_name", "")
        title = stream.get("title", "").strip()
        viewers = stream.get("viewer_count", 0)
        line = f"  [{i}] {name}"
        if game:
            line += f"  ({game})"
        line += f"  - {viewers} viewers"
        print(line)
        if title:
            print(f'        "{title[:80]}"')
    print()


def select_stream(streams: list[Stream]) -> int | None:
    choice = input(
        f"Select a stream to watch [1-{len(streams)}] (or q to quit): "
    ).strip()
    if choice.lower() in ("q", "quit", ""):
        return None
    try:
        index = int(choice)
        if not (1 <= index <= len(streams)):
            raise ValueError
    except ValueError:
        print("Invalid selection.", file=sys.stderr)
        sys.exit(1)
    return index


def launch_streamlink(url: str, quality: str) -> None:
    try:
        _ = subprocess.run(["streamlink", url, quality], check=False)
    except FileNotFoundError:
        print(
            "streamlink not found. Make sure it's installed and on your PATH.",
            file=sys.stderr,
        )
        sys.exit(1)


def run(args: CliArgs) -> None:
    client_id = client.get_env_or_exit("TWITCH_CLIENT_ID")
    token = client.get_env_or_exit("TWITCH_ACCESS_TOKEN")
    headers = {"Client-Id": client_id, "Authorization": f"Bearer {token}"}

    user_id = client.get_own_user_id(headers)
    streams = client.get_live_followed(headers, user_id)

    if not streams:
        print("No followed channels are live right now.")
        return

    print_streams(streams)

    if args.list:
        return

    index = select_stream(streams)
    if index is None:
        return

    login = streams[index - 1]["user_login"]
    url = f"https://twitch.tv/{login}"
    print(f"\nLaunching streamlink: {url} [{args.quality}]\n")
    launch_streamlink(url, args.quality)


def main() -> None:
    args = build_parser().parse_args(namespace=CliArgs())
    try:
        run(args)
    except KeyboardInterrupt:
        print("\nStopped.")
        sys.exit(0)


if __name__ == "__main__":
    main()
