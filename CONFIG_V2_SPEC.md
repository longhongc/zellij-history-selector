# Config V2 Spec

## Goal

V2 should make provider configuration:
- easier to read
- easier to reorder
- easier to document
- flexible enough for app-specific sources without adding one built-in provider type per app

This document proposes the public-facing config model for a future v2 implementation.

## Design Principles

### 1. Name providers by meaning, not by position

Numbered keys such as `provider_1_*` are awkward because:
- they are harder to scan
- reordering requires renumbering
- adding or removing one provider creates churn
- docs become more mechanical than readable

V2 should instead let users define provider ids explicitly and list them in order.

### 2. Keep the number of built-in provider types small

A provider type should represent a general source shape, not one specific application.

Why:
- app-specific built-ins do not scale
- each extra provider type increases parser complexity and docs burden
- many applications can already be expressed through a generic file, SQLite, or command source

V2 should prefer generic provider families plus a few convenience presets.

### 3. Separate source acquisition from entry mapping

A provider has two different jobs:
- get records from somewhere
- map those records into the selector's canonical entry fields

This separation makes the config more general and reduces pressure to add special-case code.

### 4. Keep an escape hatch for arbitrary applications

Users will always have niche tools and private workflows.

V2 should include one generic structured command provider so users can integrate arbitrary data sources without waiting for a built-in plugin release.

## Proposed Config Shape

### Provider ordering

```kdl
providers "ipython,bash,copyq"
```

This defines the provider order shown in the UI.

### Provider namespace

Each provider uses a flat namespaced key format:

```kdl
provider.<id>.<field> <value>
```

Example:

```kdl
provider.ipython.type "ipython"
provider.ipython.path "~/.ipython/profile_default/history.sqlite"
provider.bash.type "file_lines"
provider.bash.path "~/.bash_history"
```

## Why This Style Was Chosen

This style was chosen because it balances Zellij's flat plugin config model with human-readable configuration.

Reasons:
- it works with a flat `BTreeMap<String, String>` parser
- it is much easier to read than `provider_1_*`, `provider_2_*`
- it makes reordering explicit through `providers`
- it gives each provider a stable identity
- it keeps the external config clean without pretending nested KDL blocks are available when they are not
- it gives us a forward path without breaking current v1 users

## Canonical Provider Families

V2 should keep a small number of generic provider families.

### `file_lines`

Use for line-oriented text sources.

Good fit for:
- shell history files
- custom snippet files
- newline-delimited exports

Example:

```kdl
provider.bash.type "file_lines"
provider.bash.name "Bash"
provider.bash.path "~/.bash_history"
provider.bash.reverse true
provider.bash.dedupe true
provider.bash.limit 5000
```

### `sqlite_query`

Use for application data stored in SQLite.

Good fit for:
- IPython history
- clipboard managers with SQLite storage
- browser-like history stores
- app-specific local databases

Example:

```kdl
provider.copyq.type "sqlite_query"
provider.copyq.name "CopyQ"
provider.copyq.path "~/.local/share/copyq/copyq.sqlite"
provider.copyq.query_file "~/.config/zellij-history-selector/copyq.sql"
provider.copyq.text_column 0
provider.copyq.preview_column 1
provider.copyq.timestamp_column 2
provider.copyq.limit 5000
provider.copyq.dedupe true
```

### `command_lines`

Use when a command prints one entry per line.

Good fit for:
- lightweight exporters
- shell wrappers
- legacy integrations that do not need structured fields

Example:

```kdl
provider.custom.type "command_lines"
provider.custom.name "Custom"
provider.custom.command "my-exporter"
provider.custom.args "--plain"
provider.custom.limit 5000
```

### `command_json`

Use when a command can emit structured records.

This should be the main escape hatch for arbitrary applications.

Good fit for:
- apps with a CLI
- apps whose data is awkward to query directly inside the plugin
- private workflows that can be bridged with a short script

Example:

```kdl
provider.copyq.type "command_json"
provider.copyq.name "CopyQ"
provider.copyq.command "~/.config/zellij-history-selector/export_copyq.py"
provider.copyq.limit 5000
provider.copyq.dedupe true
```

The command should output one JSON object per line, for example:

```json
{"text":"clipboard text","preview":"full clipboard text","timestamp":"2026-04-01T10:00:00Z"}
```

## Presets vs Generic Types

V2 should distinguish between:
- generic provider families
- optional convenience presets

Example:
- `ipython` can remain as a convenience preset because it is common and easy to explain
- internally, it is still conceptually a preset over `sqlite_query`

Rule of thumb:
- add a built-in provider type only if the source cannot be expressed cleanly with existing generic families, or the source is common enough that repeated custom config becomes a real usability problem
- otherwise prefer a documented recipe or preset

## Canonical Entry Model

Regardless of source, providers should map records into a small canonical entry model:

- `text`
- `preview`
- `timestamp`
- optional `source_label`

This keeps UI and filtering logic simple while still supporting richer sources.

## Field Mapping

### `sqlite_query`

Recommended fields:
- `path`
- `query` or `query_file`
- `text_column`
- `preview_column`
- `timestamp_column`
- optional `source_column`
- `limit`
- `dedupe`

### `command_json`

Recommended fields:
- `command`
- `args`
- `cwd`
- `env_*`
- `limit`
- `dedupe`

Supported JSON keys:
- `text`
- `preview`
- `timestamp`
- `source`

Unknown keys should be ignored in v2.

## Multiple Provider Example

```kdl
providers "ipython,bash,copyq"

provider.ipython.type "ipython"
provider.ipython.name "IPython"
provider.ipython.path "~/.ipython/profile_default/history.sqlite"
provider.ipython.limit 5000
provider.ipython.dedupe true

provider.bash.type "file_lines"
provider.bash.name "Bash"
provider.bash.path "~/.bash_history"
provider.bash.reverse true
provider.bash.limit 5000
provider.bash.dedupe true

provider.copyq.type "command_json"
provider.copyq.name "CopyQ"
provider.copyq.command "~/.config/zellij-history-selector/export_copyq.py"
provider.copyq.limit 5000
provider.copyq.dedupe true
```

## Migration Strategy

V2 should keep v1 compatibility.

Recommended parser behavior:
- if `providers` is present, parse namespaced providers
- otherwise fall back to existing `provider_<n>_*` keys
- reject only clearly conflicting mixed definitions

This allows:
- better public docs for new users
- no forced migration for existing users

## Recommendation

Implementation priority:
1. add namespaced provider config with `providers`
2. rename current `command` behavior to `command_lines`
3. add `command_json`
4. keep `sqlite_query` as the generic DB-backed source
5. keep `ipython` as a convenience preset

This gives a cleaner public model without locking the plugin into app-specific provider sprawl.
