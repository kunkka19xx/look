//! Everything the linows launcher does that is not drawing.
//!
//! Search, launching, clipboard, controls, config, health: the Tauri shell and
//! the gpui port both call into here, so a fix lands once. The crate owns no
//! window and no event loop; what it needs from the shell goes through
//! [`host`].

pub mod answers;
pub mod autostart;
pub mod calc;
pub mod cli_path;
pub mod clipboard;
pub mod clipimage;
pub mod config;
pub mod consts;
pub mod crash;
pub mod files;
pub mod geometry;
pub mod health;
pub mod highlight;
pub mod host;
pub mod hotkey;
pub mod launch;
pub mod launch_query;
pub mod lunar;
pub mod music;
pub mod netspeed;
pub mod nowplaying;
pub mod paste;
pub mod platform;
pub mod process;
pub mod qactions;
pub mod query_retention;
pub mod search;
pub mod shell;
pub mod sources;
pub mod startup;
pub mod state;
pub mod sysinfo;
pub mod todo;
pub mod tools;
pub mod translate;
pub mod trash;
pub mod update;
pub mod weather;
pub mod weburl;

// The shells spell their request and reply types with these, so one import
// serves both instead of each pinning the core crates again.
pub use look_answers;
pub use look_calc;
pub use look_engine;
pub use look_lunar;
pub use look_matching;
pub use look_netspeed;
pub use look_qactions;
pub use look_sources;
pub use look_storage;
pub use look_todo;
pub use look_tools;

/// The release version, from `tauri.conf.json` through `build.rs`.
pub const APP_VERSION: &str = env!("APP_VERSION");
