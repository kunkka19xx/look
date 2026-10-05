//! Native file and folder pickers, through Tauri's dialog plugin.

use std::sync::atomic::Ordering;

use tauri::Manager;

type FileDialog = tauri_plugin_dialog::FileDialogBuilder<tauri::Wry>;

/// RAII guard around a native picker. Sets crate::PICKING_FILE for as long as it
/// lives, so the focus-loss auto-hide skips while a dialog is on screen, and
/// hands back `None` when one is already open so a double-click cannot stack two
/// dialogs.
struct PickerGuard;
impl PickerGuard {
    fn acquire() -> Option<Self> {
        crate::PICKING_FILE
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_| Self)
    }
}
impl Drop for PickerGuard {
    fn drop(&mut self) {
        crate::PICKING_FILE.store(false, Ordering::SeqCst);
    }
}

/// The launcher is a topmost window, and on Windows a topmost window outranks a
/// non-topmost dialog even when it owns it. Drop the topmost bit for as long as
/// the picker is up so it can sit in front, then restore it (see issue #467).
/// A no-op off Windows, where the field stays `None`: there the launcher is a
/// layer-shell surface and Tauri's window a hidden husk, so owning the dialog to
/// it is a platform-specific follow-up.
struct TopmostGuard {
    window: Option<tauri::WebviewWindow>,
}
impl TopmostGuard {
    fn lower(app: &tauri::AppHandle) -> Self {
        let window = cfg!(target_os = "windows")
            .then(|| app.get_webview_window(crate::consts::MAIN_WINDOW))
            .flatten();
        if let Some(w) = &window {
            let _ = w.set_always_on_top(false);
        }
        Self { window }
    }

    /// Owner the dialog to the launcher, so the OS keeps a non-topmost dialog
    /// above it. Reuses the window already resolved by `lower`.
    fn parent(&self, builder: FileDialog) -> FileDialog {
        match &self.window {
            Some(w) => builder.set_parent(w),
            None => builder,
        }
    }
}
impl Drop for TopmostGuard {
    fn drop(&mut self) {
        if let Some(w) = &self.window {
            let _ = w.set_always_on_top(true);
        }
    }
}

/// Shared picker plumbing: refuse a second dialog, lower the launcher's topmost
/// bit, own the dialog to it, then block until the callback reports back.
/// `show` picks the dialog flavour and sends the chosen path down `tx`.
fn run_picker<F>(app: &tauri::AppHandle, show: F) -> Option<String>
where
    F: FnOnce(FileDialog, std::sync::mpsc::Sender<Option<String>>),
{
    use tauri_plugin_dialog::DialogExt;
    let _guard = PickerGuard::acquire()?;
    // Dropped before `_guard`, so the topmost bit is back before the
    // focus-loss auto-hide is armed again.
    let topmost = TopmostGuard::lower(app);
    let (tx, rx) = std::sync::mpsc::channel();
    show(topmost.parent(app.dialog().file()), tx);
    rx.recv().ok().flatten()
}

pub fn pick_folder(app: &tauri::AppHandle) -> Option<String> {
    run_picker(app, |builder, tx| {
        builder
            .set_title("Choose Music Folder")
            .pick_folder(move |folder| {
                let _ = tx.send(folder.map(|f| f.to_string()));
            });
    })
}

pub fn pick_image(app: &tauri::AppHandle) -> Option<String> {
    run_picker(app, |builder, tx| {
        builder
            .set_title("Choose Background Image")
            .add_filter(
                "Images",
                &["png", "jpg", "jpeg", "webp", "bmp", "gif", "svg"],
            )
            .pick_file(move |file| {
                let _ = tx.send(file.map(|f| f.to_string()));
            });
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picker_guard_allows_only_one_at_a_time() {
        // Nothing else in the crate touches PICKING_FILE, so it starts clear.
        crate::PICKING_FILE.store(false, Ordering::SeqCst);
        let first = PickerGuard::acquire();
        assert!(first.is_some(), "the first picker should be allowed");
        assert!(
            PickerGuard::acquire().is_none(),
            "a second picker while one is open should be refused"
        );
        drop(first);
        assert!(
            PickerGuard::acquire().is_some(),
            "dropping the guard should free the next picker"
        );
    }
}
