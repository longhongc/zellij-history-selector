# zellij-history-selector

`zellij-history-selector` is a floating [Zellij](https://github.com/zellij-org/zellij) plugin for searching reusable history and snippet sources, previewing the selected entry, and inserting it back into the pane you were using before opening the plugin.

It is designed for practical, mixed-source workflows:
- shell history
- IPython history
- SQLite-backed local history stores
- clipboard managers such as [CopyQ](https://github.com/hluk/CopyQ)
- custom scripts that export structured history

## What It Does

- opens in a floating pane
- captures the pane you were on before opening the plugin
- loads entries from one or more providers
- filters entries interactively
- shows preview for multiline or rich entries
- inserts the selected entry back into the original pane
- optionally executes the selected entry immediately

## Why The Config Looks Different

Zellij `0.44.x` passes plugin config to Rust as a flat key/value map.

Because of that, this plugin does not use nested `provider { ... }` blocks. Instead, it uses:

- a `providers` order list
- flat namespaced keys like `provider.shell.type`

This keeps the config readable while still fitting Zellij's current plugin config model.

Legacy numbered config like `provider_1_type` still works, but the namespaced form is the recommended one.

## Build

```bash
rustup target add wasm32-wasip1
cargo build --release --target wasm32-wasip1
```

Artifact:

```text
target/wasm32-wasip1/release/zellij-history-selector.wasm
```

## Minimal Zellij Setup

Add a plugin alias and a keybind to your `~/.config/zellij/config.kdl`.

```kdl
keybinds {
  shared_except "locked" {
    bind "Alt h" {
      LaunchOrFocusPlugin "zellij-history-selector" {
        floating true
      }
    }
  }
}

plugins {
  zellij-history-selector location="file:/absolute/path/to/zellij-history-selector.wasm" {
    default_mode "insert"
    execute_on_select "false"
    max_results "500"
    preview_lines "12"
    case_sensitive "false"

    providers "shell"

    provider.shell.type "file_lines"
    provider.shell.name "Shell"
    provider.shell.path "~/.bash_history"
    provider.shell.reverse "true"
    provider.shell.dedupe "true"
    provider.shell.limit "5000"
  }
}
```

Important:
- launch the plugin by alias name: `LaunchOrFocusPlugin "zellij-history-selector"`
- do not launch the raw `file:/.../plugin.wasm` path if you want the `plugins { ... }` config block to apply

## Recommended Config Shape

The preferred schema is:

```kdl
providers "ipython,shell,copyq"

provider.ipython.type "ipython"
provider.ipython.path "~/.ipython/profile_default/history.sqlite"

provider.shell.type "file_lines"
provider.shell.path "~/.bash_history"

provider.copyq.type "command_json"
provider.copyq.command "~/.config/zellij/plugins/zellij-history-selector/scripts/export_copyq_json.py"
provider.copyq.args "clipboard"
```

Pattern:
- `providers` defines UI order
- `provider.<id>.type` defines the provider kind
- `provider.<id>.<field>` configures that provider

## Supported Provider Types

### `file_lines`

Use this when the source is a text file with one entry per line.

Good for:
- Bash history
- Zsh history if you already have a cleaned export
- custom snippet files

Example:

```kdl
provider.shell.type "file_lines"
provider.shell.name "Shell"
provider.shell.path "~/.bash_history"
provider.shell.reverse "true"
provider.shell.dedupe "true"
provider.shell.limit "5000"
```

### `ipython`

Convenience preset for IPython history.

Good for:
- Python REPL workflows
- notebook prototyping that gets copied into IPython

Example:

```kdl
provider.ipython.type "ipython"
provider.ipython.name "IPython"
provider.ipython.path "~/.ipython/profile_default/history.sqlite"
provider.ipython.limit "5000"
provider.ipython.dedupe "true"
```

### `sqlite_query`

Use this when your data is already stored in SQLite and you can query it directly.

Good for:
- app-specific local history databases
- internal tools with SQLite storage
- browser-like or command-like stores

Example:

```kdl
provider.sqlite.type "sqlite_query"
provider.sqlite.name "SQLite History"
provider.sqlite.path "~/.local/share/my_history.sqlite"
provider.sqlite.query "SELECT command, preview, created_at FROM command_history ORDER BY created_at DESC LIMIT 5000"
provider.sqlite.text_column "0"
provider.sqlite.preview_column "1"
provider.sqlite.timestamp_column "2"
provider.sqlite.limit "5000"
provider.sqlite.dedupe "true"
```

### `command_lines`

Use this when a command prints one logical entry per line.

Good for:
- simple exporters
- shell wrappers
- lightweight scripts

Example:

```kdl
provider.custom.type "command_lines"
provider.custom.name "Custom"
provider.custom.command "~/.config/zellij/plugins/zellij-history-selector/scripts/my_exporter.py"
provider.custom.limit "5000"
provider.custom.dedupe "true"
```

### `command_json`

Use this when a command can emit one JSON object per line.

This is the most flexible provider type and the best choice for tools where:
- one logical item can contain embedded newlines
- you want list text and preview text to stay grouped
- you need structured exports from another tool

Each output line must be a JSON object with:
- required: `text`
- optional: `preview`
- optional: `score_hint`

Example output:

```json
{"text":"first line\nsecond line","preview":"full item\nwith details","score_hint":42}
```

Example config:

```kdl
provider.copyq.type "command_json"
provider.copyq.name "CopyQ"
provider.copyq.command "~/.config/zellij/plugins/zellij-history-selector/scripts/export_copyq_json.py"
provider.copyq.args "clipboard"
provider.copyq.limit "5000"
provider.copyq.dedupe "true"
```

Legacy compatibility:
- `command` still works as an alias for `command_lines`

## Practical Recipes

### 1. Shell History

```kdl
providers "shell"

provider.shell.type "file_lines"
provider.shell.name "Bash"
provider.shell.path "~/.bash_history"
provider.shell.reverse "true"
provider.shell.dedupe "true"
provider.shell.limit "5000"
```

### 2. IPython History

```kdl
providers "ipython"

provider.ipython.type "ipython"
provider.ipython.name "IPython"
provider.ipython.path "~/.ipython/profile_default/history.sqlite"
provider.ipython.limit "5000"
provider.ipython.dedupe "true"
```

### 3. SQLite-Backed Custom History

```kdl
providers "sqlite"

provider.sqlite.type "sqlite_query"
provider.sqlite.name "SQLite Test"
provider.sqlite.path "/absolute/path/to/history.sqlite"
provider.sqlite.query "SELECT command, preview, created_at FROM command_history ORDER BY created_at DESC LIMIT 5000"
provider.sqlite.text_column "0"
provider.sqlite.preview_column "1"
provider.sqlite.timestamp_column "2"
provider.sqlite.limit "5000"
provider.sqlite.dedupe "true"
```

### 4. CopyQ With Helper Script

[CopyQ](https://github.com/hluk/CopyQ) stores tab data in its own format, so the practical route is to export through the `copyq` CLI.

The repo includes a helper:

- [export_copyq_json.py](/home/longhongc/project/zellij-history-selector/scripts/export_copyq_json.py)

Config:

```kdl
providers "copyq"

provider.copyq.type "command_json"
provider.copyq.name "CopyQ"
provider.copyq.command "/absolute/path/to/export_copyq_json.py"
provider.copyq.args "clipboard"
provider.copyq.limit "5000"
provider.copyq.dedupe "true"
```

Why `command_json` instead of `command_lines`:
- CopyQ items can be multiline
- one clipboard item should stay one entry
- preview should show the full item

### 5. CopyQ Directly Through `copyq eval`

If you do not want a helper script, you can emit JSON directly from CopyQ:

```kdl
providers "copyq"

provider.copyq.type "command_json"
provider.copyq.name "CopyQ Direct"
provider.copyq.command "copyq"
provider.copyq.args "eval -- \"tab('clipboard'); for (var i = size(); i > 0; --i) { var item = str(read(i - 1)); if (item.length) print(JSON.stringify({text: item, preview: item, score_hint: i}) + '\\n'); }\""
provider.copyq.limit "5000"
provider.copyq.dedupe "true"
```

This is more compact, but a helper script is usually easier to maintain.

## Where To Put Helper Scripts

Recommended location:

```text
~/.config/zellij/plugins/zellij-history-selector/scripts/
```

Why this location:
- it keeps plugin-related config in one place
- paths are stable across shell sessions
- users can version or back up their helper scripts easily

If you keep a dotfiles repo or a local scripts repo, that is also fine. The important part is to use an absolute path in `config.kdl`.

## If A Tool Is Hard To Parse

Not every app has a friendly history format.

Practical approach:
1. inspect what the app can export
2. if it is already line-based, use `command_lines`
3. if entries are multiline or structured, write a small exporter and use `command_json`
4. if the data is in SQLite, try `sqlite_query`

For many tools, the fastest path is to ask AI to help generate:
- the exporter script
- the SQL query
- the Zellij provider config

Useful prompt shape:

```text
I am using zellij-history-selector.
I need to export history from <tool>.
Generate a script that prints one JSON object per line with:
- text
- preview
- optional score_hint
Then generate the matching provider.copyq-style config block for Zellij.
```

This is especially useful for beginners who know what they want to load, but do not want to reverse-engineer another app's storage format by hand.

## Included Test Helpers

The repo includes local examples you can point the plugin at directly:

- [scripts/fake_command_history.py](/home/longhongc/project/zellij-history-selector/scripts/fake_command_history.py)
- [scripts/fake_command_history_json.py](/home/longhongc/project/zellij-history-selector/scripts/fake_command_history_json.py)
- [export_copyq_lines.py](/home/longhongc/project/zellij-history-selector/scripts/export_copyq_lines.py)
- [export_copyq_json.py](/home/longhongc/project/zellij-history-selector/scripts/export_copyq_json.py)
- [history.sqlite](/home/longhongc/project/zellij-history-selector/.sqlite-test/history.sqlite)

## Notes

- `file_lines` reads through Zellij's WASI host mount and needs filesystem permission.
- `command_lines`, `command_json`, and SQLite-backed providers use Zellij host command execution.
- `sqlite_query` and `ipython` currently use host `python3` for read-only SQLite access.
- `command_json` is the preferred route for multiline clipboard or snippet tools.
- do not use nested `provider { ... }` blocks with the current implementation

## Demo Video Idea

Keep it short: around 45 to 75 seconds.

Recommended flow:
1. Start in a shell pane with a realistic prompt and some recent commands visible.
2. Press your keybind to open the floating picker.
3. Show one fast shell-history search and insert.
4. Switch provider to IPython and show preview on a multiline Python snippet.
5. Switch provider to CopyQ and show that one multiline clipboard item stays one entry with full preview.
6. Select it and paste it back into the original pane.
7. Close with one final provider-switch shot so people understand it is multi-source.

What the video should communicate quickly:
- it opens fast
- it searches live
- it supports multiple providers
- preview matters for multiline content
- selection inserts back into the original pane

Recording tips:
- use a small but readable floating pane size
- keep the query strings short and intentional
- use one multiline example, not many
- avoid showing config editing in the main demo video; keep that for README screenshots or a separate setup clip

## Related Projects

- [Zellij](https://github.com/zellij-org/zellij)
- [CopyQ](https://github.com/hluk/CopyQ)
- [IPython](https://github.com/ipython/ipython)

## Config Design Notes

The config direction and rationale are documented in [CONFIG_V2_SPEC.md](/home/longhongc/project/zellij-history-selector/CONFIG_V2_SPEC.md).
