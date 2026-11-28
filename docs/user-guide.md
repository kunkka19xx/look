# look User Guide

The full guide is on the docs site: **<https://noah-code.com/docs/look>**. It is searchable and updated with every release. This page is the short version and a map to the rest.

On Windows and Linux, read `Cmd` as `Ctrl`. The launcher toggle there is `Alt+Space`, and the running-app digits use `Alt`.

## Start

1. Install Look: see the [README](../README.md#install).
2. Open it with `Cmd+Space` (macOS) or `Alt+Space` (Windows, Linux). On macOS, free the key from Spotlight in `System Settings > Keyboard > Keyboard Shortcuts > Spotlight`, or give Look another one with `launcher_hotkey`.
3. Type, move with `Tab` or the arrows, press `Enter`.

The first launch indexes your apps and files in the background. Results fill in as it goes.

## Keys to learn first

| Key                            | What it does                               |
| ------------------------------ | ------------------------------------------ |
| `Enter`                        | Open the selected row                      |
| `Ctrl+1`..`Ctrl+9`             | Open the result row showing that digit     |
| `Cmd+K`                        | List every action the row accepts          |
| `Cmd+F`                        | Reveal in the file manager                 |
| `Cmd+C` / `Cmd+P`              | Copy the file or folder / pick several     |
| `Cmd+Enter`                    | Web search the query                       |
| `"`                            | Browse the search prefixes (`a"`, `f"`, …) |
| `Cmd+/`                        | Command mode: calc, pomo, todo, and more   |
| `Cmd+,`                        | Settings                                   |
| `Cmd+Shift+;`                  | Reload the config and your sources         |
| `Cmd+Option+Q` / `Alt+Shift+Q` | Quit Look (`Cmd+Q` only hides it)          |

Every chord, and how to rebind them: [Keyboard Shortcuts](https://noah-code.com/docs/look#keyboard-shortcuts).

## Where to read more

| You want to                                                  | Read                                                                                                                                 |
| ------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------ |
| Install, update, uninstall, check permissions                | [Installation](https://noah-code.com/docs/look#installation)                                                                         |
| Search by prefix, use the clipboard, translate, do math      | [Getting Started](https://noah-code.com/docs/look#getting-started), [Search Modes](https://noah-code.com/docs/look#search-modes)     |
| Edit, reveal, copy, pick, paste or trash a row               | [Result Actions](https://noah-code.com/docs/look#result-actions)                                                                     |
| Use calc, pomo, todo, speed, kill, shell, sys                | [Command Mode](https://noah-code.com/docs/look#command-mode)                                                                         |
| Set up the search bar slot, super actions, tiles of your own | [Home Screen](https://noah-code.com/docs/look#home-screen)                                                                           |
| Ask questions, use the `>` AI session                        | [AI](https://noah-code.com/docs/look#ai)                                                                                             |
| Open Look straight into a mode from a keybinding             | [Launch Modes](https://noah-code.com/docs/look#launch-modes)                                                                         |
| Look up a config key                                         | [Configuration](https://noah-code.com/docs/look#configuration)                                                                       |
| Change the theme, layout, blur, shortcuts                    | [Settings Panel](https://noah-code.com/docs/look#settings-panel)                                                                     |
| Add your own rows                                            | [Declared Sources](https://noah-code.com/docs/look#sources), or [user-sources.md](user-sources.md) in this repo                      |
| Put your app's rows in Look                                  | [Connectors](https://noah-code.com/docs/look#connectors), or [integrating.md](integrating.md) in this repo                           |
| Fix something                                                | [Troubleshooting](https://noah-code.com/docs/look#troubleshooting)                                                                   |

## Config in short

Settings live in `~/.look/config`, one `key=value` per line. `LOOK_CONFIG_PATH` points Look at another file. Most keys have a control in Settings; after a manual edit, reload with `Cmd+Shift+;` or `lookapp reload-config`. `Settings > Advanced > Create Fresh Config` goes back to the defaults.

NixOS and Home Manager users can declare the same keys in Nix: see [BUILDING.md](../apps/linows/BUILDING.md).

## In this repo

- [features.md](features.md): what ships today, by platform
- [architecture.md](architecture.md): how the apps and the Rust core fit together
- [backend-guide.md](backend-guide.md): for contributors to the core
