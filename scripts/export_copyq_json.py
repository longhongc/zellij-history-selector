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
    script = (
        f"tab({json.dumps(tab_name)}); "
        "for (var i = 0; i < size(); ++i) { "
        "var item = str(read(i)); "
        "if (item.length) "
        "print(JSON.stringify({text: item, preview: item, score_hint: size() - i}) + '\\n'); "
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
