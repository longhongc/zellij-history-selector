# Changelog

## 1.5.1

- use bracketed paste for multiline insertions so editors like Vim preserve indentation instead of treating pasted blocks as line-by-line typed input

## 1.5.0

- add phase-1 provider profiles with `profiles`, `profile.<name>.providers`, and top-level `profile`
- document the cleaner base-alias plus launch-time `profile` override pattern for Zellij keybinds
- request permissions from the full declared provider set so switching profiles does not trigger a second prompt
- show the active profile in the floating pane title
- simplify the in-pane header to a provider flow line that keeps the current provider first and the remaining providers in actual Tab order

## 1.4.1

- recommend `move_to_focused_tab true` in the Zellij keybind example so reopening the plugin from another tab follows the current tab
- retarget and reset plugin state when an existing floating instance is moved across tabs
- reapply the custom floating height after the plugin is moved to another tab

## 1.4.0

- rename the plugin pane to a fixed short title instead of inheriting a long path-derived name
- preserve the end of long target pane titles in the header so truncated labels keep the useful suffix
- keep target pane selection scoped to the current tab so multi-tab sessions do not insert into another tab by mistake

## 1.3.0

- expand `~` in `provider.<id>.command` so helper scripts can be referenced with home-relative paths
- cap bundled CopyQ helper output by item count and item length to avoid oversized `command_json` payloads

## 1.2.2

- switched the README demo from embedded MP4 to a GIF that renders on GitHub

## 1.2.1

- added the README demo video embed

## 1.2.0

- added direct support for Zsh `EXTENDED_HISTORY` files in `file_lines`
- decode shell history files with UTF-8 lossily so invalid bytes do not break loading
- wrap provider load errors instead of truncating them to a single line
- clarified the shell history docs to explicitly target Bash and Zsh

## 1.1.0

- simplified selection behavior around `default_mode`
- added `copy` mode to copy the selected entry to the clipboard without inserting it
- improved the target pane label to show pane titles instead of raw pane IDs when available
- added lightweight shell syntax highlighting for `file_lines` and `command_lines` providers
- reduced the default preview height from 12 lines to 10 lines

## 1.0.0

- first stable release
- floating multi-provider history picker for Zellij
- support for `file_lines`, `ipython`, `sqlite_query`, `command_lines`, and `command_json`
- preview, provider switching, and insert-into-original-pane flow
- namespaced provider config with `providers` and `provider.<id>.*`
