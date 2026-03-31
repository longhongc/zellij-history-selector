# Project Spec: zellij-history-selector

## Overview

Build a Zellij plugin called `zellij-history-selector`.

The plugin opens a floating selector UI inside Zellij, lets the user fuzzy-search entries from one or more configurable history providers, and inserts the selected text into the pane that was focused before the plugin opened.

Primary use case:
- recover and reuse command history across ephemeral environments such as Docker containers
- especially useful for IPython history, shell history, SQL snippets, notebook cell history, and other reusable text entries

This should be a general-purpose selector, not hardcoded to IPython.

---

## Goals

### Core goals
1. Open from a Zellij keybinding such as `Alt+h`.
2. Appear as a floating plugin window.
3. Capture the originating pane when opened.
4. Load entries from configurable provider(s).
5. Let the user fuzzy-search and navigate entries quickly.
6. On selection, insert the chosen entry into the originating pane.
7. By default, do **not** execute the entry immediately.
8. Optionally support execute-on-select.

### Secondary goals
1. Support multiple history source types.
2. Support deduplication and recency ordering.
3. Support preview of multiline entries.
4. Be robust when the focused pane changes while the plugin is open.
5. Make configuration simple enough for normal Zellij users.

### Non-goals for v1
1. Do not support every imaginable database type.
2. Do not implement clipboard integration unless required as fallback.
3. Do not sync or persist history itself.
4. Do not attempt cross-machine replication.
5. Do not require external `fzf` for v1.

---

## Product positioning

`zellij-history-selector` is a floating selector for reusable command history and snippets in Zellij.

It is not a shell-only reverse search.
It is not limited to IPython.
It is not just a tab switcher.

The value proposition is:
- configurable history/snippet backends
- floating native Zellij UI
- insertion back into the original pane

---

## User stories

### Primary
- As a user working in Docker, I want to search my IPython history from a mounted SQLite file and insert a previous command into the current pane.

### Additional
- As a shell user, I want to search `~/.bash_history` or `~/.zsh_history`.
- As a data user, I want to query a SQLite database for previous SQL commands.
- As a power user, I want to generate candidates from a custom shell command.
- As a Zellij user, I want one consistent picker UI for all of the above.

---

## High-level UX

### Launch
- User presses `Alt+h`.
- The plugin opens as a floating pane.
- The plugin records the currently focused pane as the insertion target.

### Interaction
- User types to filter entries.
- User moves selection with arrow keys or Ctrl+j / Ctrl+k.
- A preview pane or preview area shows the full selected entry.
- User presses Enter to choose the entry.
- The plugin inserts the entry into the original pane.
- The plugin closes.

### Exit
- `Esc` closes without inserting.
- `Ctrl+c` closes without inserting.

### Optional modes
- insert mode: insert text only
- execute mode: insert text and append newline
- append mode: append to existing prompt content
- replace mode: only if later feasible; not required for v1

---

## Functional requirements

## 1. Plugin lifecycle

### 1.1 Launch
- Plugin must be launchable from Zellij keybindings.
- Plugin should be usable as a floating plugin pane.
- Plugin should support `LaunchOrFocusPlugin` style usage.

### 1.2 Target pane capture
- On open, plugin must identify and store the pane ID of the currently focused pane.
- This pane becomes the insertion target for the lifetime of that plugin instance.
- If focus changes while the plugin is open, insertion must still go to the original pane.

### 1.3 Close behavior
- After successful insert, plugin closes itself.
- On cancel, plugin closes without side effects.

---

## 2. Picker UI

### 2.1 Layout
Recommended minimal layout:
- title/header
- provider name or provider switcher
- search input
- scrollable result list
- preview area for selected item
- footer with shortcuts

### 2.2 Navigation
Required:
- Up / Down
- Enter
- Esc
- Ctrl+c

Nice to have:
- Ctrl+j / Ctrl+k
- PageUp / PageDown
- Home / End
- Tab to cycle provider
- Shift+Tab to reverse cycle provider

### 2.3 Search behavior
Required:
- incremental filtering as user types
- case-insensitive matching by default
- search across entry text

Nice to have:
- tokenized matching
- scoring that favors recent and exact-prefix matches
- highlight matched substrings

### 2.4 Preview
Required:
- show full selected entry
- preserve multiline formatting

Nice to have:
- metadata line with source/provider and timestamp
- preview truncation with scroll support

---

## 3. Insertion behavior

### 3.1 Default behavior
- Insert the selected text into the target pane stdin.
- Do not append newline by default.

### 3.2 Optional execution
- If config `execute_on_select = true`, append `\n` after insertion.

### 3.3 Multiline behavior
- Multiline entries must be inserted faithfully.
- Do not strip internal newlines unless explicitly configured.

### 3.4 Failure behavior
- If target pane no longer exists, show an error message in the plugin UI and allow retry or close.
- Do not silently discard the selection.

---

## 4. Providers

Implement provider architecture.

Each provider returns a list of entries in a normalized internal format.

### 4.1 Common entry model

Each candidate should be normalized into:

- `id: String`
- `provider_name: String`
- `text: String`
- `preview: Option<String>`
- `timestamp: Option<String>` or structured timestamp
- `score_hint: Option<i64>`
- `metadata: Map<String, String>`

At minimum, `id`, `provider_name`, and `text` are required.

### 4.2 Supported provider types for v1

#### A. `file_lines`
Read entries from a plain text file.

Config:
- `path`
- `reverse` (default true)
- `dedupe`
- `limit`
- `line_mode`:
  - `single_line`
  - `paragraph_blankline_split` (optional stretch goal)

Behavior:
- one line = one entry for v1
- read newest last or reverse depending on config
- ignore empty lines by default

Use cases:
- `.bash_history`
- `.zsh_history` after simple preprocessing
- custom text snippet files

#### B. `sqlite_query`
Run a query against a SQLite database and convert rows to entries.

Config:
- `path`
- `query`
- `text_column`
- `preview_column` optional
- `timestamp_column` optional
- `limit`
- `dedupe`

Behavior:
- execute query read-only
- each row becomes one entry
- use configured columns to map fields

Use cases:
- IPython history SQLite DB
- custom SQLite-backed snippet stores

#### C. `command`
Run a command and parse stdout into entries.

Config:
- `command`
- `args`
- `cwd` optional
- `env` optional
- `split_mode`:
  - `lines`
  - `json_lines` (stretch goal)
- `limit`
- `dedupe`

Behavior:
- execute command on plugin open or provider switch
- parse stdout into entries
- non-zero exit should surface a readable error

Use cases:
- custom history merger scripts
- project-local command generation
- remote history loaders

### 4.3 Convenience provider preset

#### D. `ipython`
This is a thin wrapper around `sqlite_query`.

Config:
- `path`
- `limit`
- `dedupe`

Default query should target IPython history in a sensible way.
Allow override if needed.

Example default mapping:
- query recent history ordered newest-first
- use the history text as the inserted value

This provider exists only for convenience and discoverability.

---

## 5. Multi-provider behavior

### 5.1 Provider selection
If multiple providers are configured:
- default to first provider
- allow cycling providers with Tab / Shift+Tab

### 5.2 Search scope
For v1, pick one of:
- search current provider only
or
- search across all providers with a source label

Preferred for v1:
- current provider only
- simpler mental model
- easier performance profile

### 5.3 Unified mode
Optional future mode:
- aggregate results from all providers into one merged list

Not required for v1.

---

## 6. Configuration

Use a clear plugin config schema that maps to provider definitions.

Do not attempt arbitrary config-file ingestion.
Instead, the plugin should consume structured plugin config from Zellij.

### 6.1 Example Zellij config snippet

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

    provider {
      type "ipython"
      name "IPython"
      path "~/.ipython/profile_default/history.sqlite"
      limit 5000
      dedupe true
    }

    provider {
      type "file_lines"
      name "Bash"
      path "~/.bash_history"
      reverse true
      dedupe true
      limit 5000
    }

    provider {
      type "sqlite_query"
      name "SQLite Commands"
      path "~/.local/share/my_history.sqlite"
      query "SELECT command, created_at FROM command_history ORDER BY created_at DESC LIMIT 5000"
      text_column 0
      timestamp_column 1
      dedupe true
    }

    provider {
      type "command"
      name "Custom"
      command "python3"
      args "-m" "my_history_exporter"
      limit 5000
      dedupe true
    }
  }
}
