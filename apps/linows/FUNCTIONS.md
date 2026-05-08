# linows — Functions & Commands Reference

Mapping from Rust core to Tauri commands, and frontend IPC calls.

---

## Tauri Commands (Backend)

### Search & Results

| Command | Signature | Description |
|---------|-----------|-------------|
| `search` | `(query: String, limit: u32) -> SearchPayload` | Fuzzy search candidates |
| `record_usage` | `(candidate_id: String, action: String) -> UsageResult` | Track opens for ranking |

**SearchPayload:**
```json
{
  "count": 5,
  "results": [
    {
      "id": "app:firefox",
      "kind": "app",
      "title": "Firefox",
      "subtitle": null,
      "path": "/usr/bin/firefox",
      "score": 1850
    }
  ]
}
```

**UsageResult:**
```json
{ "ok": true, "error": null }
```

**Actions:** `"open_app"`, `"open_file"`, `"open_folder"`

---

### File Operations

| Command | Signature | Description |
|---------|-----------|-------------|
| `open_path` | `(path: String) -> Result<(), String>` | Open file/app/folder with default handler |
| `reveal_path` | `(path: String) -> Result<(), String>` | Show in file manager (Explorer/Nautilus) |
| `get_icon` | `(path: String) -> String` | Extract icon, return as base64 data URI |

---

### Configuration

| Command | Signature | Description |
|---------|-----------|-------------|
| `reload_config` | `() -> bool` | Reload .look.config, restart watchers |
| `request_index_refresh` | `() -> bool` | Trigger background re-index |
| `get_config` | `() -> ConfigPayload` | Read current config values for settings UI |
| `save_config` | `(entries: Vec<ConfigEntry>) -> bool` | Write key=value pairs to .look.config |

---

### Platform (Windows-specific)

| Command | Signature | Description |
|---------|-----------|-------------|
| `seed_uwp_apps` | `(apps_json: String) -> ()` | Seed UWP/packaged apps into index |

---

### Clipboard (Phase 3)

| Command | Signature | Description |
|---------|-----------|-------------|
| `get_clipboard_history` | `(query: String) -> Vec<ClipboardEntry>` | Filtered clipboard entries |
| `delete_clipboard_entry` | `(index: usize) -> bool` | Remove entry by index |
| `copy_to_clipboard` | `(text: String) -> bool` | Write text to system clipboard |

---

### Commands (Phase 3)

| Command | Signature | Description |
|---------|-----------|-------------|
| `eval_calc` | `(expr: String) -> CalcResult` | Evaluate math expression |
| `exec_shell` | `(cmd: String) -> ShellResult` | Run shell command, capture output |
| `list_processes` | `(query: String) -> Vec<ProcessInfo>` | Fuzzy-match running processes |
| `kill_process` | `(pid: u32) -> bool` | Terminate process by PID |
| `get_system_info` | `() -> SystemInfo` | CPU, memory, disk, GPU, network |
| `scan_music_folder` | `(folder: String) -> Vec<String>` | List audio files in folder |
| `pick_folder` | `(app: AppHandle) -> Option<String>` | Native folder picker dialog |
| `music_play` | `(path: String) -> Result<(), String>` | Play audio file via rodio |
| `music_pause` | `() -> ()` | Pause playback |
| `music_resume` | `() -> ()` | Resume playback |
| `music_stop` | `() -> ()` | Stop playback |
| `music_is_finished` | `() -> bool` | Check if current track finished |

---

### Translation (Phase 3)

| Command | Signature | Description |
|---------|-----------|-------------|
| `translate` | `(text: String, target_lang: String) -> TranslateResult` | Web translation |

**TranslateResult:**
```json
{ "original": "hello", "translated": "xin chao", "error": null }
```

---

### Window Control

| Command | Signature | Description |
|---------|-----------|-------------|
| `toggle_window` | `() -> ()` | Show/hide main window |
| `hide_window` | `() -> ()` | Hide window (Escape key) |

---

## Frontend IPC (JS → Rust)

All calls go through `js/ipc.js` wrapper:

```javascript
// ipc.js
const { invoke } = window.__TAURI__.core;

export async function search(query, limit = 40) {
  return invoke('search', { query, limit });
}

export async function recordUsage(candidateId, action) {
  return invoke('record_usage', { candidateId, action });
}

export async function openPath(path) {
  return invoke('open_path', { path });
}

// ... etc
```

---

## Frontend Events (Rust → JS)

| Event | Payload | Description |
|-------|---------|-------------|
| `window-shown` | `{}` | Window became visible (focus input) |
| `window-hidden` | `{}` | Window was hidden |
| `clipboard-changed` | `{ text, timestamp }` | New clipboard content captured |
| `index-refreshed` | `{ count }` | Background index completed |
| `config-reloaded` | `{}` | Config file was reloaded |

---

## State Management (Rust)

```rust
pub struct AppState {
    engine: RwLock<QueryEngine>,
    clipboard_history: Mutex<VecDeque<ClipboardEntry>>,
    index_dirty: AtomicBool,
    index_refresh_in_progress: AtomicBool,
    icon_cache: Mutex<HashMap<PathBuf, String>>,
}
```

Engine initialization flow:
1. Load from SQLite (or demo seed if empty)
2. Spawn background bootstrap (discover + index all candidates)
3. Start file watchers (mark index dirty on create/rename/delete)
4. On toggle_window: if dirty, trigger background refresh
