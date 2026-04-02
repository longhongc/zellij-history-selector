# zellij-history-selector

`zellij-history-selector` is a floating [Zellij](https://github.com/zellij-org/zellij) plugin for searching history and snippets from multiple sources, previewing the selected entry, and inserting it back into the pane you were using before opening the plugin.

It is built for practical workflows:
- shell history
- IPython history
- SQLite-backed local history stores
- clipboard managers such as [CopyQ](https://github.com/hluk/CopyQ)
- custom scripts that export lines or JSON

## What It Does

- opens in a floating pane
- captures the pane you were on before opening the plugin
- filters entries interactively
- previews multiline entries
- switches between multiple providers
- inserts the selected entry back into the original pane
- can optionally execute the selected entry immediately

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

Add a plugin alias and a keybind to `~/.config/zellij/config.kdl`.

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

## Config Model

Zellij `0.44.x` passes plugin config as a flat key/value map, so this plugin uses:
- a `providers` order list
- flat namespaced keys like `provider.shell.type`

Recommended shape:

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

Legacy numbered keys such as `provider_1_type` still work, but the namespaced form is the recommended one.

## Provider Types

Use the simplest provider that matches your source:

- `file_lines`
  Best for plain text files with one entry per line.
- `ipython`
  Convenience preset for IPython history.
- `sqlite_query`
  Best for local SQLite-backed history stores.
- `command_lines`
  Best when a command prints one logical entry per line.
- `command_json`
  Best when entries can be multiline or need structured preview.

For `command_json`, each output line must be a JSON object with:
- required: `text`
- optional: `preview`
- optional: `score_hint`

Example:

```json
{"text":"first line\nsecond line","preview":"full item\nwith details","score_hint":42}
```

## Practical Recipes

### Shell History

```kdl
providers "shell"

provider.shell.type "file_lines"
provider.shell.name "Bash"
provider.shell.path "~/.bash_history"
provider.shell.reverse "true"
provider.shell.dedupe "true"
provider.shell.limit "5000"
```

### IPython History

```kdl
providers "ipython"

provider.ipython.type "ipython"
provider.ipython.name "IPython"
provider.ipython.path "~/.ipython/profile_default/history.sqlite"
provider.ipython.limit "5000"
provider.ipython.dedupe "true"
```

### SQLite-Backed Custom History

```kdl
providers "sqlite"

provider.sqlite.type "sqlite_query"
provider.sqlite.name "SQLite History"
provider.sqlite.path "/absolute/path/to/history.sqlite"
provider.sqlite.query "SELECT command, preview, created_at FROM command_history ORDER BY created_at DESC LIMIT 5000"
provider.sqlite.text_column "0"
provider.sqlite.preview_column "1"
provider.sqlite.timestamp_column "2"
provider.sqlite.limit "5000"
provider.sqlite.dedupe "true"
```

### CopyQ

[CopyQ](https://github.com/hluk/CopyQ) works best through `command_json`, so multiline clipboard items stay grouped and render correctly in preview.

With helper script:

```kdl
providers "copyq"

provider.copyq.type "command_json"
provider.copyq.name "CopyQ"
provider.copyq.command "/absolute/path/to/export_copyq_json.py"
provider.copyq.args "clipboard"
provider.copyq.limit "5000"
provider.copyq.dedupe "true"
```

Directly through `copyq eval`:

```kdl
providers "copyq"

provider.copyq.type "command_json"
provider.copyq.name "CopyQ Direct"
provider.copyq.command "copyq"
provider.copyq.args "eval -- \"tab('clipboard'); for (var i = size(); i > 0; --i) { var item = str(read(i - 1)); if (item.length) print(JSON.stringify({text: item, preview: item, score_hint: i}) + '\\n'); }\""
provider.copyq.limit "5000"
provider.copyq.dedupe "true"
```

## Custom Integrations

If a tool is easy to export, you usually do not need a built-in provider for it:

- use `file_lines` if the tool already writes a line-based file
- use `sqlite_query` if the data is already in SQLite
- use `command_lines` if a command can print one entry per line
- use `command_json` if entries are multiline or need structured preview

For tools that are hard to parse, write a small exporter script first. A practical location is:

```text
~/.config/zellij/plugins/zellij-history-selector/scripts/
```

If you want repo-local demo providers and ready-to-use testing fixtures, see [scripts/README.md](scripts/README.md).

## Related Projects

- [Zellij](https://github.com/zellij-org/zellij)
- [CopyQ](https://github.com/hluk/CopyQ)
- [IPython](https://github.com/ipython/ipython)
