# Handoff: Process Finder (`ps"`) + Kill Fuzzy-Find — Windows & macOS

Reference implementation landed in **linows** (`apps/linows/`). This document
specifies the feature and what each other platform needs to reach parity.

- **Windows** = the linows Tauri build (`apps/linows/`, shared JS + `src-tauri`).
  Backend stubs need filling; the frontend is already shared and works as soon
  as the backend returns real data.
- **macOS** = the native SwiftUI app (`apps/macos/`), the design source of truth.
  Needs a full native port: backend, frontend, and keyboard shortcuts.

> The process code is intentionally **not** in `core/` — enumeration is
> irreducibly OS-specific (`/proc` vs Win32 vs libproc), so each platform
> implements its own. Only the fuzzy scoring is shared (`core/matching`).

---

## 1. What the feature is

### `ps"` prefix — a live process finder
Typing `ps"` enters a process mode (sibling of `c"` clipboard, `t"` translate):

- **Empty query** → all of the user's processes, name + PID.
- **Query** → fuzzy match on process name; a **numeric** query also matches a
  **listening TCP port** (find-by-port) or the PID.
- **Rows**: `Name` + `PID · :ports`. App-backed processes show the app icon;
  others a generic process glyph.
- **Preview**: Command line (argv), Memory, User, Parent PID, Started, CPU
  (measured **on demand**, see below), Ports.
- **Keys** (Linux/Windows use `Ctrl`, macOS uses `Cmd`):
  - `Enter` → measure & show CPU% for the selected process (on-demand only).
  - `Ctrl/Cmd+D` → kill (SIGKILL).
  - `Ctrl/Cmd+C` → copy PID.
  - `Esc` → clear query / leave mode.

### Kill command — now a fuzzy finder over apps + processes
The existing `kill` command (Ctrl+4 / Cmd+4) gained fuzzy search:

- **Empty query** → apps only (unchanged default).
- **Query** → apps that fuzzy-match rank **first**, then any other matching
  process, deduped by PID. App-backed processes keep their app icon.
- **`:port`** → unchanged port lookup.

---

## 2. Reference backend (linows `src-tauri/src/process.rs`)

Tauri commands and their serialized payloads. **All names are `snake_case` over
the IPC boundary** (serde) — match these field names on every platform so the
shared frontend keeps working.

```rust
// Row in the ps" finder.
struct ProcRow { name: String, pid: u32, icon_source: Option<String>, ports: Vec<u16> }

// Per-selection preview detail. start_epoch is Unix seconds (frontend formats it).
struct ProcDetail { cmdline: String, rss_kb: u64, user: String, ppid: u32, start_epoch: Option<u64> }

// Row in the kill command (app or raw process).
struct KillTarget { name: String, pid: u32, is_app: bool, desktop_id: Option<String>, exec: Option<String> }
```

| Command | Signature | Notes |
|---|---|---|
| `search_processes` | `(query: String, refresh: bool) -> Vec<ProcRow>` | **async**. Cached snapshot; `refresh` re-enumerates (mode entry + post-kill), else scores the cache. Limit 50. |
| `process_detail` | `(pid: u32) -> Option<ProcDetail>` | sync; cheap per-selection reads. |
| `process_cpu` | `(pid: u32) -> Option<f64>` | **async**; two-sample delta, ~200 ms. Percent of one core (may exceed 100, like `top`). |
| `search_kill_targets` | `(query: String) -> Vec<KillTarget>` | **async**. Empty → apps; else apps-first then processes, deduped by PID. Limit 60. |
| reused | `kill_process(pid)`, `copy_to_clipboard(text)`, `list_processes()` | already cross-platform. |

### Scoring rules (must match on every platform)
- **Name**: `look_matching::fuzzy_score_prepared`.
- **Numeric query** (`ps"`): exact port > partial-port-substring > PID-substring,
  all ranked **above** name matches; name fuzzy still runs (names can contain digits).
- **Kill ranking**: all matching apps (by display name) first, then matching
  processes, each group sorted by score then name.

### ⚠️ Gotchas (cost real debugging time — honor them)
1. **`look_matching::fuzzy_score` is CASE-SENSITIVE.** Lowercase **both** the
   query and each title before scoring (the engine does the same via its
   normalized-query path). App names are `Capitalized`; queries are typed
   lowercase — skip this and app matches silently vanish.
2. **CPU is on-demand (Enter), never per-selection.** Sampling needs a sleep;
   doing it on every arrow-key would jank. Bind it to Enter only.
3. **Anything that walks the process table or sleeps must run off the UI/main
   thread** (Tauri: `async` + `spawn_blocking`). Same principle on every platform.
4. **Snapshot caching**: enumerate once on mode entry / after a kill, then
   re-score the cache per keystroke. Never walk the process table per keystroke.
5. **Modifier**: `Ctrl` on Linux/Windows, `Cmd` on macOS.

### Linux data sources (for translating field-by-field)

| Field | Source |
|---|---|
| enumerate / name / uid | `/proc/[pid]/status` (`Name:`, `Uid:`), filter own uid |
| cmdline | `/proc/[pid]/cmdline` (NUL-separated argv) |
| rss_kb | **USS** = `Private_Clean` + `Private_Dirty` from `/proc/[pid]/smaps_rollup` (private memory, matches macOS `phys_footprint`). `VmRSS:` from `/proc/[pid]/status` is the compatibility fallback whenever smaps_rollup can't be read or parsed (kernels < 4.14, permission denied, process exited mid-read); it counts shared pages and reads ~2× higher, so never prefer it. |
| ppid | `/proc/[pid]/status` `PPid:` |
| user | uid → `/etc/passwd` |
| start_epoch | `/proc/[pid]/stat` field 22 (starttime ticks) + `/proc/stat` `btime`, `CLK_TCK=100` |
| cpu | `/proc/[pid]/stat` utime(14)+stime(15) sampled twice; `100·(Δticks/CLK_TCK)/Δsecs` |
| ports | `/proc/net/tcp{,6}` LISTEN (state `0A`) → inode→port; `/proc/[pid]/fd/*` `socket:[inode]` → pid→ports |
| icon_source | match `/proc` names to `.desktop` entries → pid → desktop path |

### Reference frontend (shared JS — read before porting to SwiftUI)
- `src/js/catalog.js` — `PREFIX_ENTRIES` entry `{ prefix: 'ps"', ... }`.
- `src/js/search.js` — `processMode`, `forceProcessRefresh`, `performProcessSearch`.
- `src/js/components/results.js` — process row + app-icon-or-glyph.
- `src/js/components/preview.js` — `renderProcessPreview`, `measureCpu` (on Enter).
- `src/js/keyboard.js` — Enter/Ctrl+D/Ctrl+C/Esc handling in process mode.
- `src/js/screens/commands/kill.js` + `app.js` `kill-search` — kill fuzzy path.

---

## 3. Windows (linows Tauri) — fill the stubs

File: `apps/linows/src-tauri/src/platform/windows/process.rs`.
Win32 imports, `enumerate_processes() -> Vec<(u32, String)>`, `resolve_full_path`,
`GetExtendedTcpTable`, `kill`, and `list()`/`list_on_port()` already exist.

**Status:** `list_all()` works (Toolhelp) and now **attaches listening ports**
(`listening_ports_by_pid()` parses the `GetExtendedTcpTable`
`TCP_TABLE_OWNER_PID_LISTENER` table, big-endian port via `parse_port`), but
still returns `icon_source: None`. `process_detail` / `process_cpu` dispatch to
`None` in `process.rs` (search across `#[cfg(target_os = "windows")]`).

**TODO:**

1. **`list_all()` — icon.** (Ports are already done — see Status.)
   - `icon_source`: Windows apps have no `.desktop`; `list()` already emits
     `desktop_id = "app:<exe_path>"`. Set `icon_source = Some(exe_path)` (from
     `resolve_full_path(pid)`) so app-backed rows show the exe icon. The shared
     frontend loads it via `getIcon('app', path)`.

2. **`detail(pid) -> Option<ProcDetail>`** (new):
   - `cmdline`: `NtQueryInformationProcess(ProcessBasicInformation)` → PEB →
     `ProcessParameters.CommandLine` via `ReadProcessMemory`. Fallback to the
     exe path (`resolve_full_path`) if the remote read is denied.
   - `rss_kb`: `GetProcessMemoryInfo` with `PROCESS_MEMORY_COUNTERS_EX` →
     **`PrivateUsage / 1024`** (private commit charge: Task Manager's "Commit
     size", Process Explorer's "Private Bytes"), *not* `WorkingSetSize`. It is
     the closest Windows analog to macOS `phys_footprint` / Linux USS, excluding
     the shared pages that make `WorkingSetSize` read much higher.
   - `ppid`: Toolhelp `PROCESSENTRY32W.th32ParentProcessID` (build a pid→ppid
     map in the existing `enumerate_processes` pass).
   - `user`: `OpenProcessToken(TOKEN_QUERY)` → `GetTokenInformation(TokenUser)`
     → `LookupAccountSidW`. Fallback to `""`.
   - `start_epoch`: `GetProcessTimes` creation `FILETIME` → Unix seconds
     (`(filetime - 116444736000000000) / 10_000_000`).

3. **`cpu(pid) -> Option<f64>`** (new): `GetProcessTimes` kernel+user `FILETIME`
   sampled twice around a ~200 ms sleep; `100·(Δ100ns / 1e7) / Δsecs`. Keep the
   `spawn_blocking` wrapper (already in `process.rs`).

4. **Wire dispatch** in `process.rs`: replace the two windows `None` arms of
   `process_detail` / `process_cpu` with calls into the new functions.

Frontend needs no changes — ports, detail, and CPU populate automatically once
the backend returns them.

---

## 4. macOS (native SwiftUI `apps/macos/`) — ✅ implemented

Design source of truth. Matches the linows layout (which was modeled on the
macOS patterns — `LauncherView.swift`, `ResultPreviewView`, the command panels).

**Status: done.** Implemented natively; see the files below. Chosen approach:
**native Swift enumeration + `core/matching` fuzzy over a new FFI export** (the
hybrid recommended in 4a), so ranking is identical to linows without porting the
DP scorer.

New/changed files:
- `bridge/ffi/src/matching_api.rs` + `look_fuzzy_score` export in `lib.rs` —
  exposes `look_matching::fuzzy_score` over the C ABI (Swift decl + `fuzzyScore`
  wrapper in `EngineBridge.swift`). `look-matching` added to `bridge/ffi/Cargo.toml`.
- `Support/Launcher/ProcessScoring.swift` — pure port of `score_process` /
  `rank_kill_targets` (numeric port/PID tiers + apps-first kill ordering), fuzzy
  injected as a closure so it's unit-testable. Parity tests in
  `LauncherLogicTests/ProcessScoringTests.swift` mirror the Rust tests.
- `Support/Launcher/ProcessService.swift` — native `libproc`/`sysctl`
  enumeration, detail, on-demand CPU, ports, SIGKILL (off-main).
- `Support/Launcher/ProcessFinderModel.swift` — snapshot cache + detail/CPU
  caches, refreshed on mode entry / after a kill.
- `Views/Launcher/LauncherView+Process.swift` — `ps"` mode: results, preview,
  measure-CPU (Enter), kill (Cmd+D), copy-PID (Cmd+C). Rows/preview/kind wired
  through `LauncherResult.process` in the existing results/preview views.
- `KillCommand.swift` — `/kill` upgraded to fuzzy over apps + processes
  (apps-first, deduped); empty → apps, `:port` unchanged.

### 4a. Architecture choice
Process ops are OS-specific and are **not** in `core/`, so macOS reimplements
them. Two options:
- **Native Swift** (recommended): `libproc`/`sysctl` are C APIs trivially
  callable from Swift; keeps it close to the SwiftUI layer.
- **Rust module + FFI** (`bridge/ffi/`): only if you want to share the fuzzy
  scoring path end-to-end. Note `core/matching` is already reachable — you can
  call it over FFI for identical ranking while keeping enumeration native.
  **(This is what shipped: native enumeration, FFI for the fuzzy score only.)**

### 4b. Backend (macOS APIs)

| Field | API |
|---|---|
| enumerate | **`sysctl KERN_PROC_ALL`** (gives pid + uid + comm in one call); filter to `getuid()`. **Do NOT use `proc_listallpids`** — it is silently throttled in hardened/restricted contexts and returns only a partial list (observed ~146 of ~584 procs, dropping the user's own apps). |
| name | `proc_name` / comm |
| cmdline (argv) | `sysctl KERN_PROCARGS2` |
| memory | `proc_pid_rusage(pid, RUSAGE_INFO_V2)` → **`ri_phys_footprint`** / 1024. Use footprint, not `ri_resident_size` (RSS): footprint matches Activity Monitor's "Memory" column, while RSS counts shared pages and reads ~2× higher (confusing when users cross-check). |
| cpu | `proc_pid_rusage` `ri_user_time + ri_system_time` (ns) sampled twice; `100·(Δns/1e9)/Δsecs` |
| ppid / uid / start | `proc_pidinfo(PROC_PIDTBSDINFO)` → `pbi_ppid`, `pbi_uid`, `pbi_start_tvsec` (already Unix seconds → `start_epoch`) |
| user | `getpwuid` |
| ports | `proc_pidinfo(PROC_PIDLISTFDS)` → for `PROX_FDTYPE_SOCKET`, `proc_pidfdinfo(PROC_PIDFDSOCKETINFO)` → `socket_fdinfo`; keep TCP sockets in listen state (`soi_kind == SOCKINFO_TCP`, `tcpsi_state == TSI_S_LISTEN`), local port `insi_lport` (`ntohs`) |
| icon | `NSWorkspace.shared.icon(forFile: bundleOrExePath)`; app-backed via `NSRunningApplication(processIdentifier:)` → `.bundleURL` |
| kill | `kill(pid, SIGKILL)` |

**⚠️ macOS-specific gotchas:**
- **`task_for_pid` and reading other processes' info require elevated
  privileges / an entitlement** under SIP. `proc_pid_rusage` and `proc_pidinfo`
  work for the **current user's** processes without special entitlement — scope
  the finder to own-uid processes (matches Linux `Uid:` filter). Killing other
  users' processes needs privilege; surface a clear error rather than failing
  silently.
- Same on-demand-CPU and off-main-thread rules as §2 (use a background queue;
  don't block the UI on the sampling sleep).
- Same case-insensitive fuzzy rule — lowercase both sides.

### 4c. Frontend (SwiftUI)
- Detect the `ps"` prefix in the launcher input; enter a process mode alongside
  the existing prefix modes.
- Rows: `Name` + `PID` (append `· :port` when listening). App icon when
  app-backed, else an SF Symbol (`cpu` / `gearshape`).
- Preview pane mirroring linows: **Command** (argv), Memory, User, Parent PID,
  Started, **CPU** (shows "Press Enter to measure" until requested), Ports.
- Kill command: fuzzy over apps + processes, **apps first**, matching the
  linows behavior; keep the existing confirm-before-kill flow.

### 4d. Keyboard / shortkeys (macOS conventions, `Cmd` not `Ctrl`)

| Action | macOS | (linows equivalent) |
|---|---|---|
| Enter process mode | type `ps"` | same |
| Measure CPU | `Enter` (on selected) | same |
| Kill (SIGKILL) | **`Cmd+D`** | `Ctrl+D` |
| Copy PID | **`Cmd+C`** | `Ctrl+C` |
| Clear / leave mode | `Esc` | same |
| Navigate | `↑` / `↓` | same |

Hint bar: `Cmd+D: Kill • Cmd+C: Copy PID`.

---

## 5. Parity checklist

The boxes are the reference list every port must satisfy, not a per-platform
tracker, so they stay unchecked. **macOS meets every item** (see §4); **Windows
still owes** `icon_source`, `process_detail`, `process_cpu` (ports already done).

- [ ] `ps"` prefix enters process mode; empty query lists own-user processes.
- [ ] Numeric query matches exact port > partial port > PID; name fuzzy also runs.
- [ ] Rows: Name + PID (+ ports); app-backed rows show the app icon.
- [ ] Preview: cmdline, memory, user, ppid, started, ports; CPU on Enter only.
- [ ] Kill/CopyPID on the platform modifier; Esc clears.
- [ ] `kill` command: empty → apps; query → apps-first then processes, deduped.
- [ ] Fuzzy is case-insensitive (lowercase both sides).
- [ ] Enumeration/sleeps run off the UI thread; snapshot cached, refreshed on
      entry + after a kill.
- [ ] Kill failures on privileged processes report a clear error.
