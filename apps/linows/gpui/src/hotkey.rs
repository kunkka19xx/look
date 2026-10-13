//! The launcher toggle and Alt+Shift+Q quit as global hotkeys. Windows only:
//! Linux binds through the compositor (see main.rs).

#[cfg(windows)]
mod imp {
    use std::cell::RefCell;

    use global_hotkey::hotkey::{Code, HotKey, Modifiers};
    use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
    use linows_backend::health;
    use linows_backend::hotkey::{configured, report_bind};

    /// The manager owns a message window, so it lives on the main thread.
    struct Bound {
        manager: GlobalHotKeyManager,
        key: Option<HotKey>,
    }

    thread_local! {
        static BOUND: RefCell<Option<Bound>> = const { RefCell::new(None) };
    }

    /// Main thread only, inside the running app.
    pub fn start(
        on_toggle: impl Fn() + Send + Sync + 'static,
        on_quit: impl Fn() + Send + Sync + 'static,
    ) {
        let quit = HotKey::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::KeyQ);
        let quit_id = quit.id();
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.state != HotKeyState::Pressed {
                return;
            }
            if event.id == quit_id {
                on_quit();
            } else {
                on_toggle();
            }
        }));
        set_active(true);
        BOUND.with_borrow(|slot| {
            // Losing it is minor: quitting stays available in the window.
            if let Some(Err(e)) = slot.as_ref().map(|b| b.manager.register(quit)) {
                eprintln!("look: failed to register Alt+Shift+Q: {e}");
            }
        });
    }

    /// Inactive frees the key for the settings recorder. Active binds what the
    /// config holds now.
    pub fn set_active(active: bool) {
        BOUND.with_borrow_mut(|slot| {
            if slot.is_none() {
                match GlobalHotKeyManager::new() {
                    Ok(manager) => *slot = Some(Bound { manager, key: None }),
                    Err(e) => {
                        report_bind(&configured(), Err(e.to_string()));
                        return;
                    }
                }
            }
            let Some(bound) = slot.as_mut() else {
                return;
            };
            if let Some(key) = bound.key.take() {
                let _ = bound.manager.unregister(key);
            }
            if !active {
                return;
            }
            health::clear(health::ISSUE_HOTKEY);
            let launcher = configured();
            if !launcher.enabled {
                return;
            }
            let result = launcher
                .accelerator
                .parse::<HotKey>()
                .map_err(|e| e.to_string())
                .and_then(|key| {
                    bound.manager.register(key).map_err(|e| e.to_string())?;
                    bound.key = Some(key);
                    Ok(())
                });
            report_bind(&launcher, result);
        });
    }
}

#[cfg(windows)]
pub use imp::{set_active, start};

#[cfg(not(windows))]
pub fn set_active(_: bool) {}
