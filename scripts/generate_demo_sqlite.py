#!/usr/bin/env python3

from __future__ import annotations

import sqlite3
from pathlib import Path


ROWS = [
    (
        "git status",
        "git status",
        "2026-04-01T10:00:00Z",
        30,
    ),
    (
        "cargo check --target wasm32-wasip1",
        "cargo check --target wasm32-wasip1",
        "2026-04-01T10:03:00Z",
        28,
    ),
    (
        "def fibonacci(n):\n    if n < 2:\n        return n\n    return fibonacci(n - 1) + fibonacci(n - 2)\n\nfibonacci(8)",
        "def fibonacci(n):\n    if n < 2:\n        return n\n    return fibonacci(n - 1) + fibonacci(n - 2)\n\nfibonacci(8)",
        "2026-04-01T10:05:00Z",
        26,
    ),
    (
        "items = ['alpha', 'beta', 'gamma']\n[item.upper() for item in items]",
        "items = ['alpha', 'beta', 'gamma']\n[item.upper() for item in items]",
        "2026-04-01T10:08:00Z",
        24,
    ),
    (
        "SELECT command, preview, created_at FROM command_history ORDER BY created_at DESC LIMIT 20;",
        "SELECT command, preview, created_at FROM command_history ORDER BY created_at DESC LIMIT 20;",
        "2026-04-01T10:12:00Z",
        22,
    ),
    (
        "copyq eval -- \"tab('clipboard'); print(size())\"",
        "copyq eval -- \"tab('clipboard'); print(size())\"",
        "2026-04-01T10:16:00Z",
        20,
    ),
    (
        "from pathlib import Path\nsorted(path.name for path in Path('src').iterdir())",
        "from pathlib import Path\nsorted(path.name for path in Path('src').iterdir())",
        "2026-04-01T10:19:00Z",
        18,
    ),
    (
        "docker ps --format 'table {{.Names}}\\t{{.Status}}'",
        "docker ps --format 'table {{.Names}}\\t{{.Status}}'",
        "2026-04-01T10:23:00Z",
        16,
    ),
    (
        "kubectl get pods -A",
        "kubectl get pods -A",
        "2026-04-01T10:27:00Z",
        14,
    ),
    (
        "journalctl -u my-service -n 50",
        "journalctl -u my-service -n 50",
        "2026-04-01T10:31:00Z",
        12,
    ),
]


SCHEMA = """
CREATE TABLE IF NOT EXISTS command_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    command TEXT NOT NULL,
    preview TEXT NOT NULL,
    created_at TEXT NOT NULL,
    score_hint INTEGER NOT NULL DEFAULT 0
);
"""


def main() -> int:
    output_path = (
        Path(__file__).resolve().parent / "generated" / "demo_history.sqlite"
    )
    output_path.parent.mkdir(parents=True, exist_ok=True)

    if output_path.exists():
        output_path.unlink()

    connection = sqlite3.connect(output_path)
    try:
        connection.execute(SCHEMA)
        connection.executemany(
            """
            INSERT INTO command_history (command, preview, created_at, score_hint)
            VALUES (?, ?, ?, ?)
            """,
            ROWS,
        )
        connection.commit()
    finally:
        connection.close()

    print(output_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
