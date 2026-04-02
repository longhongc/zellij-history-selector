# Scripts

This directory contains local testing helpers for the plugin.

They are meant to help users exercise provider types quickly without having to prepare their own data source first.

## Included Helpers

### `sample_file_history.txt`

Static demo data for the `file_lines` provider.

Use this when you want to verify:
- simple line-based history loading
- reverse ordering
- dedupe
- basic fuzzy search

Config:

```kdl
providers "file-demo"

provider.file-demo.type "file_lines"
provider.file-demo.name "File Demo"
provider.file-demo.path "/absolute/path/to/zellij-history-selector/scripts/sample_file_history.txt"
provider.file-demo.reverse "false"
provider.file-demo.dedupe "true"
provider.file-demo.limit "5000"
```

### `fake_command_history.py`

Deterministic line-based exporter for the `command_lines` provider.

Use this when you want to test:
- host command execution
- one-line command history
- command-backed provider config

Config:

```kdl
providers "command-demo"

provider.command-demo.type "command_lines"
provider.command-demo.name "Command Demo"
provider.command-demo.command "/absolute/path/to/zellij-history-selector/scripts/fake_command_history.py"
provider.command-demo.limit "5000"
provider.command-demo.dedupe "true"
```

### `fake_command_history_json.py`

Deterministic JSON-lines exporter for the `command_json` provider.

Use this when you want to test:
- multiline entries
- preview text
- grouped structured records

Each line printed by the script is one JSON object with `text`, `preview`, and `score_hint`.

Config:

```kdl
providers "json-demo"

provider.json-demo.type "command_json"
provider.json-demo.name "JSON Demo"
provider.json-demo.command "/absolute/path/to/zellij-history-selector/scripts/fake_command_history_json.py"
provider.json-demo.limit "5000"
provider.json-demo.dedupe "true"
```

### `generate_demo_sqlite.py`

Creates a simple SQLite fixture for the `sqlite_query` provider.

Run:

```bash
python3 scripts/generate_demo_sqlite.py
```

Output:

```text
scripts/generated/demo_history.sqlite
```

The generated database contains a `command_history` table with:
- `command`
- `preview`
- `created_at`
- `score_hint`

Config:

```kdl
providers "sqlite-demo"

provider.sqlite-demo.type "sqlite_query"
provider.sqlite-demo.name "SQLite Demo"
provider.sqlite-demo.path "/absolute/path/to/zellij-history-selector/scripts/generated/demo_history.sqlite"
provider.sqlite-demo.query "SELECT command, preview, created_at FROM command_history ORDER BY created_at DESC LIMIT 5000"
provider.sqlite-demo.text_column "0"
provider.sqlite-demo.preview_column "1"
provider.sqlite-demo.timestamp_column "2"
provider.sqlite-demo.limit "5000"
provider.sqlite-demo.dedupe "true"
```

## CopyQ Helpers

### `export_copyq_lines.py`

Exports CopyQ tab items as one escaped line per entry.

This is mainly useful for simple experiments, because multiline clipboard items get flattened into a line-oriented format.

Config:

```kdl
providers "copyq-lines"

provider.copyq-lines.type "command_lines"
provider.copyq-lines.name "CopyQ Lines"
provider.copyq-lines.command "/absolute/path/to/zellij-history-selector/scripts/export_copyq_lines.py"
provider.copyq-lines.args "clipboard"
provider.copyq-lines.limit "5000"
provider.copyq-lines.dedupe "true"
```

### `export_copyq_json.py`

Exports CopyQ items as JSON lines so multiline clipboard entries stay grouped and render correctly in preview.

Use this as the recommended CopyQ integration.

Config:

```kdl
providers "copyq"

provider.copyq.type "command_json"
provider.copyq.name "CopyQ"
provider.copyq.command "/absolute/path/to/zellij-history-selector/scripts/export_copyq_json.py"
provider.copyq.args "clipboard"
provider.copyq.limit "5000"
provider.copyq.dedupe "true"
```

## Quick Start

If you just want to test all provider families with repo-local data:

1. Use `sample_file_history.txt` for `file_lines`.
2. Use `fake_command_history.py` for `command_lines`.
3. Run `python3 scripts/generate_demo_sqlite.py` and use the generated file for `sqlite_query`.
4. Use `fake_command_history_json.py` for `command_json`.

All four paths work without needing any external app or personal history file.
