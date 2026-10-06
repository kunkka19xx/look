//! Work off the UI thread, answered back into an entity. The backend blocks
//! on D-Bus, SQLite, `/proc` and the clipboard, so every call to it goes
//! through here; the clipboard in particular would deadlock on the main
//! loop, which has to answer the ownership request the write makes.

use gpui::Context;

/// Run `work` on the background executor and hand its answer to `apply` on
/// the entity, if the entity is still alive.
pub fn fetch<V: 'static, T: Send + 'static>(
    cx: &mut Context<V>,
    work: impl FnOnce() -> T + Send + 'static,
    apply: impl FnOnce(&mut V, T, &mut Context<V>) + 'static,
) {
    cx.spawn(async move |this, cx| {
        let value = cx.background_executor().spawn(async move { work() }).await;
        let _ = this.update(cx, |this, cx| {
            apply(this, value, cx);
            cx.notify();
        });
    })
    .detach();
}
