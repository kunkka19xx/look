//! Shared session D-Bus plumbing (cached zbus connection + tokio runtime),
//! used by the GNOME Shell extension client and the KWin scripting client.

use std::sync::OnceLock;

/// Shared tokio runtime for D-Bus calls, avoids creating a new one each call.
/// Private: every caller goes through [`block_on`], which is the only spelling
/// that survives being reached from a thread that is already inside a runtime.
fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to create D-Bus tokio runtime")
    })
}

/// Runs a D-Bus call to completion from synchronous code.
///
/// `Runtime::block_on` panics when the calling thread is already driving a
/// runtime, and D-Bus helpers get called from threads on both sides of that
/// line. A thread that is inside one hands the future to a thread that is not,
/// which borrows exactly as the direct path does, so a call site reads the same
/// either way.
pub fn block_on<F>(future: F) -> F::Output
where
    F: Future + Send,
    F::Output: Send,
{
    if tokio::runtime::Handle::try_current().is_err() {
        return runtime().block_on(future);
    }
    std::thread::scope(|scope| scope.spawn(|| runtime().block_on(future)).join())
        .unwrap_or_else(|payload| std::panic::resume_unwind(payload))
}

/// Cached D-Bus session connection.
pub fn session() -> Option<&'static zbus::Connection> {
    static CONN: OnceLock<Option<zbus::Connection>> = OnceLock::new();
    CONN.get_or_init(|| block_on(async { zbus::Connection::session().await.ok() }))
        .as_ref()
}

/// Cached D-Bus system connection (system services like BlueZ).
pub fn system() -> Option<&'static zbus::Connection> {
    static CONN: OnceLock<Option<zbus::Connection>> = OnceLock::new();
    CONN.get_or_init(|| block_on(async { zbus::Connection::system().await.ok() }))
        .as_ref()
}

#[cfg(test)]
mod tests {
    /// The Wayland toggle service answers its D-Bus method inside a runtime,
    /// and everything it does to the window reaches this module from there.
    #[test]
    fn block_on_works_from_inside_a_runtime() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("test runtime");
        assert_eq!(rt.block_on(async { super::block_on(async { 7 }) }), 7);
    }
}
