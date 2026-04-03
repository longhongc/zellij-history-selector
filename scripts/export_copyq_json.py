#!/usr/bin/env python3

import argparse
import json
import subprocess
import sys


DEFAULT_MAX_ITEMS = 500
DEFAULT_MAX_CHARS = 20_000


def run_copyq(*args: str) -> str:
    result = subprocess.run(
        ["copyq", *args],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    return result.stdout


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Export CopyQ items as JSON lines for zellij-history-selector."
    )
    parser.add_argument(
        "tab_name",
        nargs="?",
        default="clipboard",
        help="CopyQ tab name to export (default: clipboard)",
    )
    parser.add_argument(
        "--max-items",
        type=int,
        default=DEFAULT_MAX_ITEMS,
        help=f"Maximum number of items to export (default: {DEFAULT_MAX_ITEMS})",
    )
    parser.add_argument(
        "--max-chars",
        type=int,
        default=DEFAULT_MAX_CHARS,
        help=(
            "Maximum number of characters per exported item before truncation "
            f"(default: {DEFAULT_MAX_CHARS})"
        ),
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    tab_name = args.tab_name
    max_items = max(args.max_items, 1)
    max_chars = max(args.max_chars, 1)

    script = (
        f"tab({json.dumps(tab_name)}); "
        f"var limit = Math.min(size(), {max_items}); "
        f"var maxChars = {max_chars}; "
        "for (var i = 0; i < limit; ++i) { "
        "var item = str(read(i)); "
        "if (!item.length) continue; "
        "var truncated = item; "
        "if (truncated.length > maxChars) "
        "  truncated = truncated.slice(0, Math.max(0, maxChars - 1)) + '…'; "
        "print(JSON.stringify({text: truncated, preview: truncated, score_hint: size() - i}) + '\\n'); "
        "}"
    )

    try:
        sys.stdout.write(run_copyq("eval", "--", script))
    except Exception as error:
        sys.stderr.write(f"Failed to export CopyQ tab '{tab_name}': {error}\n")
        return 1

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
