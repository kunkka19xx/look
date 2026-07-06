//! Shared session D-Bus plumbing (cached zbus connection + tokio runtime),
//! used by the GNOME Shell extension client and the KWin scripting client.

use std::sync::OnceLock;

/// Shared tokio runtime for D-Bus calls, avoids creating a new one each call.
pub fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to create D-Bus tokio runtime")
    })
}

/// Cached D-Bus session connection.
pub fn session() -> Option<&'static zbus::Connection> {
    static CONN: OnceLock<Option<zbus::Connection>> = OnceLock::new();
    CONN.get_or_init(|| runtime().block_on(async { zbus::Connection::session().await.ok() }))
        .as_ref()
}
