#!/usr/bin/env python3

import json
import subprocess
import sys


def run_copyq(*args: str) -> str:
    result = subprocess.run(
        ["copyq", *args],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    return result.stdout


def main() -> int:
    tab_name = sys.argv[1] if len(sys.argv) > 1 else "clipboard"

    try:
        count_output = run_copyq("tab", tab_name, "count").strip()
        count = int(count_output or "0")
    except Exception as error:
        sys.stderr.write(f"Failed to query CopyQ tab '{tab_name}': {error}\n")
        return 1

    for index in range(count - 1, -1, -1):
        try:
            item = run_copyq("tab", tab_name, "read", str(index))
        except Exception as error:
            sys.stderr.write(f"Failed to read CopyQ item {index}: {error}\n")
            return 1

        item = item.rstrip("\n")
        if not item.strip():
            continue

        # Current command_lines provider is line-oriented.
        # Encode embedded newlines so each CopyQ item stays a single record.
        print(json.dumps(item, ensure_ascii=False)[1:-1])

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
