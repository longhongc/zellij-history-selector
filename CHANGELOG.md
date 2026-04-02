# Changelog

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
