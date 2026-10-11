//! Work off the UI thread, answered back into an entity. The backend blocks
//! on D-Bus, SQLite, `/proc` and the clipboard, so every call to it goes
//! through here; the clipboard in particular would deadlock on the main
//! loop, which has to answer the ownership request the write makes.

use gpui::Context;

/// Blocking work on a thread of its own. On gpui's executor pool, slow
/// network calls held every thread and the search queued behind them.
pub fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Blocking<T> {
    let (tx, rx) = async_channel::bounded(1);
    std::thread::Builder::new()
        .name("look-blocking".into())
        .spawn(move || {
            let _ = tx.send_blocking(work());
        })
        .expect("spawn a blocking worker");
    Blocking(rx)
}

pub struct Blocking<T>(async_channel::Receiver<T>);

impl<T> Blocking<T> {
    pub async fn get(self) -> T {
        self.0.recv().await.expect("blocking work panicked")
    }
}

/// Run `work` on its own thread and hand its answer to `apply` on the
/// entity, if the entity is still alive.
pub fn fetch<V: 'static, T: Send + 'static>(
    cx: &mut Context<V>,
    work: impl FnOnce() -> T + Send + 'static,
    apply: impl FnOnce(&mut V, T, &mut Context<V>) + 'static,
) {
    cx.spawn(async move |this, cx| {
        let value = blocking(work).get().await;
        let _ = this.update(cx, |this, cx| {
            apply(this, value, cx);
            cx.notify();
        });
    })
    .detach();
}
