# zellij-history-selector

`zellij-history-selector` is a floating Zellij plugin for fuzzy-searching reusable history/snippet entries and inserting the selected text back into the pane that was focused before the plugin opened.

## Status

This repository implements the plugin with a flat-config compatibility model for Zellij `0.44.x`:

- Zellij `0.44.x` passes plugin config to Rust as a flat key/value map.
- Because of that, repeated nested `provider { ... }` blocks from the spec cannot be deserialized literally.
- The current implementation now supports:
  - preferred namespaced config with `providers` plus keys like `provider.shell.type`
  - legacy indexed keys like `provider_1_type` as a fallback for older setups

The config direction and rationale are documented in [`CONFIG_V2_SPEC.md`](/home/longhongc/project/zellij-history-selector/CONFIG_V2_SPEC.md).

## Build

```bash
rustup target add wasm32-wasip1
cargo build --release
```

The plugin artifact will be:

```text
target/wasm32-wasip1/release/zellij-history-selector.wasm
```

## Example Zellij Config

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
  zellij-history-selector location="file:~/.config/zellij/plugins/zellij-history-selector.wasm" {
    default_mode "insert"
    execute_on_select false
    max_results 500
    preview_lines 12
    case_sensitive false

    providers "ipython,bash,sqlite,custom"

    provider.ipython.type "ipython"
    provider.ipython.name "IPython"
    provider.ipython.path "~/.ipython/profile_default/history.sqlite"
    provider.ipython.limit "5000"
    provider.ipython.dedupe "true"

    provider.bash.type "file_lines"
    provider.bash.name "Bash"
    provider.bash.path "~/.bash_history"
    provider.bash.reverse "true"
    provider.bash.dedupe "true"
    provider.bash.limit "5000"

    provider.sqlite.type "sqlite_query"
    provider.sqlite.name "SQLite Commands"
    provider.sqlite.path "~/.local/share/my_history.sqlite"
    provider.sqlite.query "SELECT command, created_at FROM command_history ORDER BY created_at DESC LIMIT 5000"
    provider.sqlite.text_column "0"
    provider.sqlite.timestamp_column "1"
    provider.sqlite.dedupe "true"

    provider.custom.type "command_lines"
    provider.custom.name "Custom"
    provider.custom.command "python3"
    provider.custom.args "-m my_history_exporter"
    provider.custom.limit "5000"
    provider.custom.dedupe "true"

    provider.copyq.type "command_json"
    provider.copyq.name "CopyQ"
    provider.copyq.command "/home/longhongc/project/zellij-history-selector/scripts/export_copyq_json.py"
    provider.copyq.args "clipboard"
    provider.copyq.limit "5000"
    provider.copyq.dedupe "true"
  }
}
```

When launching from a keybind, use the plugin alias name if you want the `plugins { zellij-history-selector ... }` configuration to be applied:

```kdl
bind "Alt h" {
  LaunchOrFocusPlugin "zellij-history-selector" {
    floating true
  }
}
```

If you launch the raw `file:/.../plugin.wasm` URL directly, Zellij will not apply the alias-backed config block.

## Supported Providers

- `file_lines`
- `sqlite_query`
- `command_lines`
- `command_json`
- `ipython`

Legacy compatibility:
- `command` still works as an alias for `command_lines`
- `provider_1_*` style numbered config still works as a fallback

## Notes

- `file_lines` reads through Zellij's WASI host mount and needs filesystem permission.
- `command_lines` and SQLite-backed providers use Zellij host command execution.
- `command_json` expects one JSON object per output line with at least a `text` field, and supports multiline `text` / `preview` values.
- `sqlite_query` and `ipython` currently use host `python3` for read-only SQLite access.
- Do not use nested `provider { ... }` blocks with the current implementation.
