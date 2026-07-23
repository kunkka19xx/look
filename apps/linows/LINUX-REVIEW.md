# linows Linux review — findings + proposed fixes

Review date: 2026-07-17. Scope: Linux-specific paths in `apps/linows/src-tauri/src`
(app launching, window focus, global shortcut, startup, clipboard, icons, files).
All findings are **pre-existing** (not from the ignored-patterns branch).

Status legend: ✅ verified against source · ⚠️ logic-verified / relayed from review.
Line numbers are from the 2026-07-17 tree and may drift. The `Fix` blocks are
sketches to guide the change, not drop-in patches — verify against current code.

## High

### H1. Use-after-free in the index watcher ✅
- **Where:** `state.rs` — `AppState::new()` (`:131`), capture at `:257`, deref at `:268`/`:435`; `.manage(AppState::new())` at `main.rs:540`.
- **Defect:** `new()` captures raw addresses of its own fields (`&self.index_change_version as *const _ as usize`, `&self.engine`, …) into the watcher thread, then returns `self` by value; `.manage(...)` moves the struct. The watcher and its reindex workers dereference the stale pre-move addresses.
- **Impact:** watcher reindexes write the rebuilt engine into freed memory (UB); auto-refresh silently never updates the live managed state (masked because window-show refresh runs on the managed state).
- **Fix (preferred): move the shared fields behind one `Arc`, delete the raw-pointer `unsafe` entirely.**
  ```rust
  struct IndexShared {
      engine: RwLock<QueryEngine>,
      change_version: AtomicU64,
      cleared_version: AtomicU64,
      in_progress: AtomicBool,
      last_refresh_completed_unix_ms: AtomicU64,
  }
  pub struct AppState {
      shared: Arc<IndexShared>,
      watcher_control: Mutex<Option<mpsc::Sender<()>>>,
  }
  // spawn_refresh_worker takes `shared: Arc<IndexShared>` instead of WatcherStatePtrs;
  // start_index_watchers moves `self.shared.clone()` into the thread.
  thread::spawn(move || {
      // no unsafe, no address smuggling:
      let mut guard = shared.engine.write().unwrap_or_else(|p| p.into_inner());
      *guard = new_engine;
      shared.change_version.fetch_add(1, Ordering::AcqRel);
  });
  ```
  The `Arc` heap allocation is address-stable across the `.manage()` move, so `WatcherStatePtrs` and all five `unsafe` derefs disappear.
- **Fix (lighter alt):** don't start watchers in `new()`; expose `start_index_watchers(&self)` and call it *after* manage via `app.state::<AppState>()` (Tauri boxes managed state, so its address is stable). Keeps the fragile `unsafe`; prefer the `Arc` version.

### H2. `get_pointer` hangs the show path on an untimed D-Bus call ✅
- **Where:** `platform/linux/gnome_ext.rs:214`; on the show path via `main.rs:600/122/127` → `monitor_at_cursor`.
- **Fix: bound the call; callers already handle `None`.**
  ```rust
  pub fn get_pointer() -> Option<(i32, i32)> {
      let conn = dbus_conn()?;
      dbus_runtime().block_on(async {
          let call = conn.call_method(Some(DBUS_NAME), DBUS_PATH, Some(DBUS_IFACE), "GetPointer", &());
          let reply = tokio::time::timeout(Duration::from_millis(300), call).await.ok()??;
          reply.body().deserialize().ok()
      })
  }
  ```

### H3. File-clipboard silently fails on X11 ✅
- **Where:** `platform/linux/clipboard.rs:43`.
- **Fix: gate on exit status, not "did it run".**
  ```rust
  if matches!(wl_result, Ok(status) if status.success()) {
      return Ok(());
  }
  // else fall through to xclip
  ```

## Medium

### M1. Two concurrent `bootstrap_sqlite` on the same DB at startup ✅
- **Where:** `state.rs:190` (`start_background_bootstrap`) vs `state.rs:174` (`request_index_refresh`).
- **Fix: let the initial bootstrap take the refresh slot so window-show can't double up.**
  ```rust
  fn start_background_bootstrap(&self) {
      let holds_slot = self.try_acquire_refresh_slot(); // usually true (first refresh)
      let dirty = self.index_change_version.load(Ordering::Acquire);
      spawn_refresh_worker(self.ptrs(), BootstrapScope::ALL, dirty, holds_slot, true, "look: bootstrap");
  }
  ```
  A concurrent `request_index_refresh` then CAS-fails and returns `false` instead of spawning a second reindex.

### M2. `shell.rs` never drains piped output → false timeouts ✅
- **Where:** `shell.rs:31-44`.
- **Fix: drain both pipes on their own threads while polling, so the child never blocks on a full pipe.**
  ```rust
  let mut out = child.stdout.take().unwrap();
  let mut err = child.stderr.take().unwrap();
  let out_h = std::thread::spawn(move || { let mut b = Vec::new(); let _ = out.read_to_end(&mut b); b });
  let err_h = std::thread::spawn(move || { let mut b = Vec::new(); let _ = err.read_to_end(&mut b); b });
  // keep the existing try_wait poll loop (kill + wait on timeout);
  // after exit: let stdout = out_h.join().unwrap_or_default(); (same for stderr)
  ```

### M3. `wlr_focus` busy-spins 500 ms when no windows are open ✅
- **Where:** `platform/linux/wlr_focus.rs:44` and `:91`.
- **Fix: sleep between roundtrips and stop after enumeration settles (incl. the zero-toplevel case).**
  ```rust
  let mut empty_rounds = 0;
  while Instant::now() < deadline {
      if !state.toplevels.is_empty() && state.toplevels.iter().all(|t| t.done) { break; }
      if state.toplevels.is_empty() { empty_rounds += 1; if empty_rounds >= 2 { break; } }
      std::thread::sleep(Duration::from_millis(4)); // no busy spin
      if queue.roundtrip(&mut state).is_err() { break; }
  }
  ```

### M4. `wm_class_matches` uses substring match → wrong-window focus ✅
- **Where:** `platform/linux/window_focus.rs:149`.
- **Fix: WM_CLASS is NUL-separated `instance\0class\0`; compare tokens for exact (case-insensitive) equality.**
  ```rust
  String::from_utf8_lossy(&reply.value)
      .split('\0')
      .filter(|s| !s.is_empty())
      .any(|tok| tok.eq_ignore_ascii_case(target))
  ```

### M5. GNOME Alt+Space window-menu binding permanently lost after a crash ⚠️
- **Where:** `platform/linux/wayland_shortcut.rs:328` (in-memory `SAVED_WM_BINDING` only), restore on `RunEvent::Exit`.
- **Fix: persist the original to a state file before blanking; recover on next launch.**
  ```rust
  // path: ~/.local/state/look/gnome-wm-binding.saved
  fn disable_window_menu_binding() -> bool {
      let cur = gsettings_get("org.gnome.desktop.wm.keybindings", "activate-window-menu");
      if cur.contains("<Alt>space") {
          write_saved_binding(&cur);           // disk + in-memory
          gsettings_set(.., "['']");
          return true;
      }
      // crash recovery: value already blank, but we have a saved original
      if is_blank(&cur) && let Some(saved) = read_saved_binding() {
          remember_in_memory(saved);            // restored on exit
          return true;
      }
      false
  }
  ```
  Delete the state file after a successful restore. Same treatment for KDE `SAVED_KRUNNER_KEYS` (`:404`).

### M6. D-Bus focus calls lack timeouts ⚠️
- **Where:** `gnome_ext.rs` `try_focus_app` (`:306`), `list_windowed_apps` (`:262`), verify ping (`:117`); `kde_focus.rs` `load_and_run` (`:124`), `stop` (`:82`).
- **Fix: one helper, applied at every call site.**
  ```rust
  async fn call_bounded(fut: impl Future<Output = zbus::Result<Message>>) -> Option<Message> {
      tokio::time::timeout(Duration::from_millis(1500), fut).await.ok()?.ok()
  }
  ```
  Matters most for the KDE path (runs under `CALL_LOCK`, so a hang serializes all later focus attempts).

### M7. `resolve_themed_icon` uncached ⚠️ (perf)
- **Where:** `platform/linux/icons.rs:216-272`, `build_icon_search_dirs` (`:235`), `detect_gtk_icon_theme` (`:274`).
- **Fix: compute dirs/theme once; add a negative cache for misses.**
  ```rust
  static SEARCH_DIRS: OnceLock<Vec<String>> = OnceLock::new();
  fn icon_search_dirs() -> &'static [String] { SEARCH_DIRS.get_or_init(build_icon_search_dirs) }

  static MISSES: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
  // resolve_themed_icon: bail early if MISSES contains name; on unresolved, insert(name).
  ```
  Trade-off: a runtime theme change needs a restart (or invalidate on the GTK settings-change signal) — acceptable for a launcher; note it in a comment.

## Low

- **L1** `wayland_shortcut.rs:495` — wrap the `presses.next().await` loop in a reconnect-with-backoff loop; on stream end call `health::report(...)` and re-establish the kglobalaccel signal instead of returning `Ok(())`.
- **L2** `transparency.rs:74` — cache the result: `static HAS_COMPOSITOR: OnceLock<bool>` filled once, like `is_wayland()` in `main.rs:276`.
- **L3** `window_focus.rs` — reuse one X11 connection for a show/focus/list sequence (pass `&conn` into the helpers) instead of `x11rb::connect(None)` per call; the monitor thread already holds one.
- **L4** `window_focus.rs:340` — reset `MONITOR_RUNNING.store(false, ..)` on the atom-intern failure path too (the connect-failure sibling already does), so the monitor can re-spawn.
- **L5** `state.rs:375` — snapshot dirty flags but clear them only after the refresh returns `Ok`; on failure, leave `apps_dirty/files_dirty/last_dirty_at` set (or re-arm them) so the retry isn't lost.
- **L6** `files.rs:371` (`time_from_unix`) — apply the local UTC offset before formatting. No tz dep is present; either read the offset via `libc::localtime_r` in a small `unsafe` shim, or add a lightweight tz crate (confirm before adding).
- **L7** `files.rs:239-287` (`pick_folder`/`pick_image`) — replace the blocking `std::sync::mpsc::recv()` in the async command with `tauri::async_runtime::spawn_blocking` (or an async oneshot) so the dialog doesn't pin a tokio worker.
- **L8** `files.rs:230/316` — for non-UTF8 names, either skip them with a logged warning or carry the raw bytes so the frontend can round-trip the real path; `to_string_lossy` currently corrupts them silently.
- **L9** `main.rs:392-447` — decide intent: if Alt+Shift+Q should work on Wayland, register it through the same portal/global-shortcut path as Alt+Space; otherwise add a comment that quit is intentionally in-app only on Wayland.

## Coverage gap

The actual app-launch chain (`gtk-launch → gio → exec`) lives in `commands.rs:414+`,
not `platform/linux/process.rs` (which only does `/proc` listing + `kill -9`), so it
was outside this review's scope. Review that next.
