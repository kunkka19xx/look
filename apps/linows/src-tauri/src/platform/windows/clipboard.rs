//! Windows file-clipboard stub. Real impl (CF_HDROP via SetClipboardData) lands in M4.

pub(crate) fn copy_files(_paths: &[String]) -> Result<(), String> {
    // TODO(M4): OpenClipboard + EmptyClipboard + SetClipboardData(CF_HDROP, DROPFILES).
    Err("file clipboard not yet implemented on Windows".to_string())
}
