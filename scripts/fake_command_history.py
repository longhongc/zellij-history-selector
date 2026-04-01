#!/usr/bin/env python3

ENTRIES = [
    "git status",
    "git add src/config.rs README.md",
    "git commit -m 'Refine provider config parsing'",
    "cargo check --target wasm32-wasip1",
    "cargo build --release --target wasm32-wasip1",
    "cargo test --target wasm32-wasip1 --no-run",
    "rg -n \"provider\" src README.md",
    "uv run ipython",
    "python3 -m http.server 8000",
    "sqlite3 history.sqlite '.tables'",
    "SELECT command, created_at FROM command_history ORDER BY created_at DESC LIMIT 20;",
    "docker ps --format 'table {{.Names}}\\t{{.Status}}'",
    "kubectl get pods -A",
    "journalctl -u my-service -n 50",
    "systemctl restart my-service",
    "printf '%s\\n' alpha beta gamma",
    "curl -s https://example.com | head",
    "from pathlib import Path; sorted(p.name for p in Path('src').iterdir())",
    "items = ['alpha', 'beta', 'gamma']",
    "[item.upper() for item in items]",
]


def main() -> None:
    for entry in ENTRIES:
        print(entry)


if __name__ == "__main__":
    main()
