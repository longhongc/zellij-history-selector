# zellij-history-selector

`zellij-history-selector` is a floating Zellij plugin for fuzzy-searching reusable history/snippet entries and inserting the selected text back into the pane that was focused before the plugin opened.

## Status

This repository implements the v1 spec from [`project_spec.md`](/home/longhongc/project/zellij-history-selector/project_spec.md) with one compatibility adjustment:

- Zellij `0.44.x` passes plugin config to Rust as a flat key/value map.
- Because of that, repeated nested `provider { ... }` blocks from the spec cannot be deserialized literally.
- V1 therefore uses flat indexed keys like `provider_1_type`, `provider_1_name`, `provider_1_path`.

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
      LaunchOrFocusPlugin "file:~/.config/zellij/plugins/zellij-history-selector.wasm" {
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

    provider_1_type "ipython"
    provider_1_name "IPython"
    provider_1_path "~/.ipython/profile_default/history.sqlite"
    provider_1_limit "5000"
    provider_1_dedupe "true"

    provider_2_type "file_lines"
    provider_2_name "Bash"
    provider_2_path "~/.bash_history"
    provider_2_reverse "true"
    provider_2_dedupe "true"
    provider_2_limit "5000"

    provider_3_type "sqlite_query"
    provider_3_name "SQLite Commands"
    provider_3_path "~/.local/share/my_history.sqlite"
    provider_3_query "SELECT command, created_at FROM command_history ORDER BY created_at DESC LIMIT 5000"
    provider_3_text_column "0"
    provider_3_timestamp_column "1"
    provider_3_dedupe "true"

    provider_4_type "command"
    provider_4_name "Custom"
    provider_4_command "python3"
    provider_4_args "-m my_history_exporter"
    provider_4_limit "5000"
    provider_4_dedupe "true"
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
- `command`
- `ipython`

## Notes

- `file_lines` reads through Zellij's WASI host mount and needs filesystem permission.
- `command` and SQLite-backed providers use Zellij host command execution.
- `sqlite_query` and `ipython` currently use host `python3` for read-only SQLite access.
