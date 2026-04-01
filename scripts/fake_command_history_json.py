#!/usr/bin/env python3

import json


ENTRIES = [
    {
        "text": "git status",
        "preview": "git status",
        "score_hint": 20,
    },
    {
        "text": "def fib(n):\n    if n < 2:\n        return n\n    return fib(n - 1) + fib(n - 2)\n\nfib(8)",
        "preview": "def fib(n):\n    if n < 2:\n        return n\n    return fib(n - 1) + fib(n - 2)\n\nfib(8)",
        "score_hint": 18,
    },
    {
        "text": "items = ['alpha', 'beta', 'gamma']\n[item.upper() for item in items]",
        "preview": "items = ['alpha', 'beta', 'gamma']\n[item.upper() for item in items]",
        "score_hint": 16,
    },
    {
        "text": "docker ps --format 'table {{.Names}}\\t{{.Status}}'",
        "preview": "docker ps --format 'table {{.Names}}\\t{{.Status}}'",
        "score_hint": 14,
    },
]


def main() -> None:
    for entry in ENTRIES:
        print(json.dumps(entry, ensure_ascii=False))


if __name__ == "__main__":
    main()
