// Shared constants used across multiple modules.

/// Windows process creation flag to suppress console windows.
#[cfg(target_os = "windows")]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Delay between starting something and trying to focus the window it opened -
/// long enough for the app to have received the input and drawn. Used by the
/// file/URL open path and by the preferred-tools launcher on both platforms.
pub const HANDLER_FOCUS_DELAY_MS: u64 = 150;
