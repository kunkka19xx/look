//! The folder picker. gpui's own path prompt hands the portal the focused
//! surface as the dialog's parent through xdg_foreign, and a layer-shell
//! surface is no xdg toplevel: the compositor answers with a fatal protocol
//! error and the Wayland connection dies with it. On Linux the request goes
//! to the portal directly, with no parent; elsewhere gpui's prompt serves.

use std::path::PathBuf;

use gpui::{App, Task};

/// Open the system folder dialog with `prompt` on its accept button. `Ok(None)`
/// is the user backing out; `Err` is no dialog at all.
pub fn folder(prompt: &'static str, cx: &App) -> Task<Result<Option<PathBuf>, String>> {
    #[cfg(target_os = "linux")]
    {
        cx.background_executor()
            .spawn(async move { portal_folder(prompt).await })
    }
    #[cfg(not(target_os = "linux"))]
    {
        let picked = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(prompt.into()),
        });
        cx.background_executor().spawn(async move {
            match picked.await {
                Ok(Ok(paths)) => Ok(paths.and_then(|paths| paths.into_iter().next())),
                Ok(Err(err)) => Err(err.to_string()),
                Err(_) => Ok(None),
            }
        })
    }
}

#[cfg(target_os = "linux")]
async fn portal_folder(prompt: &str) -> Result<Option<PathBuf>, String> {
    use ashpd::desktop::file_chooser::OpenFileRequest;

    const TITLE: &str = "Open Folder";
    const FILE_SCHEME: &str = "file://";

    let request = OpenFileRequest::default()
        .title(TITLE)
        .accept_label(Some(prompt))
        .directory(true)
        .modal(true)
        .send()
        .await
        .map_err(|err| err.to_string())?;
    let response = match request.response() {
        Ok(response) => response,
        Err(ashpd::Error::Response(_)) => return Ok(None),
        Err(err) => return Err(err.to_string()),
    };
    let folder = response
        .uris()
        .iter()
        .filter_map(|uri| uri.as_str().strip_prefix(FILE_SCHEME))
        .map(|path| PathBuf::from(crate::pomo::percent_decode(path)))
        .next();
    Ok(folder)
}
