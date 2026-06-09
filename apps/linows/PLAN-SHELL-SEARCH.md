# Shell Search (`>` prefix)

Search and launch executables from `$PATH` + shell history, ranked by usage.

## Prefix

`>` in the main search bar triggers this mode (same pattern as `c"` clipboard, `t"` translate).

## Sources

### 1. Shell History

| Shell | File | Format |
|-------|------|--------|
| bash | `~/.bash_history` | one command per line |
| zsh | `~/.zsh_history` | `: timestamp:0;command` |
| fish | `~/.local/share/fish/fish_history` | `- cmd: command` |

- Extract base command (first word) for matching
- Preserve full command (with args) for display
- Track frequency + recency per command

### 2. PATH Executables

- Walk all directories in `$PATH`
- Collect executable name + full path
- Deduplicate (same name in multiple dirs — keep first per PATH order)
- Skip entries that already have a `.desktop` match (avoid dupes with app search)

## Ranking

1. History frequency (highest weight)
2. History recency
3. Exact prefix match bonus
4. PATH-only entries (never run) ranked lowest

## Execution

- GUI apps (known or has `.desktop`) -> spawn directly
- CLI/TUI commands -> spawn in default terminal emulator
- Working directory: `$HOME`
- Heuristic: if command has no `.desktop` entry and isn't in a known-GUI list, assume terminal
- User override: Enter = auto-detect, Ctrl+Enter = force terminal

## Frontend

- `search.js`: new `>` prefix branch (like `c"` and `t"`)
- Results render in the normal results list with a terminal icon
- Hint bar: `Enter run * Ctrl+Enter run in terminal * Esc clear`
- Preview panel: executable path, type, source (history/PATH/both), frequency

## Backend

- New module: `src-tauri/src/executables.rs`
  - `scan_path()` — collect executables from `$PATH`
  - `parse_shell_history()` — parse bash/zsh/fish history
  - `search_executables(query)` — fuzzy match + rank
- Cache PATH + history on startup
- Re-read history tail on window show (lightweight refresh)

## Platforms

- Linux: full support (PATH + bash/zsh/fish history)
- macOS: full support (same shells, PATH includes Homebrew dirs)
- Windows: deferred

## Open Questions

- [ ] Show full history commands with args (e.g. `ssh myserver`) as separate entries, or just unique command names?
- [ ] Terminal emulator detection: respect `$TERMINAL`, `x-terminal-emulator`, or config option?
- [ ] Max results cap for PATH scan (thousands of binaries)?
- [ ] Should history commands with args be re-runnable as-is, or just pre-fill a terminal?
