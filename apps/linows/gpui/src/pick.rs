//! The folder and image pickers. gpui's own path prompt hands the portal the
//! focused surface as the dialog's parent through xdg_foreign, and a
//! layer-shell surface is no xdg toplevel: the compositor answers with a
//! fatal protocol error and the Wayland connection dies with it. On Linux
//! the request goes to the portal directly, with no parent; elsewhere
//! gpui's prompt serves.

use std::path::PathBuf;

use gpui::{App, Task};

/// What the dialog is for; each has its title and its filter.
#[derive(Clone, Copy)]
enum Want {
    Folder,
    Image,
}

/// Open the system folder dialog with `prompt` on its accept button. `Ok(None)`
/// is the user backing out; `Err` is no dialog at all.
pub fn folder(prompt: &'static str, cx: &App) -> Task<Result<Option<PathBuf>, String>> {
    pick(Want::Folder, prompt, cx)
}

/// Open the system file dialog narrowed to pictures.
pub fn image(prompt: &'static str, cx: &App) -> Task<Result<Option<PathBuf>, String>> {
    pick(Want::Image, prompt, cx)
}

fn pick(want: Want, prompt: &'static str, cx: &App) -> Task<Result<Option<PathBuf>, String>> {
    #[cfg(target_os = "linux")]
    {
        cx.background_executor()
            .spawn(async move { portal_pick(want, prompt).await })
    }
    #[cfg(not(target_os = "linux"))]
    {
        let directories = matches!(want, Want::Folder);
        let picked = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: !directories,
            directories,
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
async fn portal_pick(want: Want, prompt: &str) -> Result<Option<PathBuf>, String> {
    use ashpd::desktop::file_chooser::{FileFilter, OpenFileRequest};

    const FILE_SCHEME: &str = "file://";
    const IMAGE_MIME: &str = "image/*";

    let request = match want {
        Want::Folder => OpenFileRequest::default()
            .title("Open Folder")
            .directory(true),
        Want::Image => OpenFileRequest::default()
            .title("Choose Image")
            .filter(FileFilter::new("Images").mimetype(IMAGE_MIME)),
    };
    let request = request
        .accept_label(Some(prompt))
        .modal(true)
        .send()
        .await
        .map_err(|err| err.to_string())?;
    let response = match request.response() {
        Ok(response) => response,
        Err(ashpd::Error::Response(_)) => return Ok(None),
        Err(err) => return Err(err.to_string()),
    };
    let picked = response
        .uris()
        .iter()
        .filter_map(|uri| uri.as_str().strip_prefix(FILE_SCHEME))
        .map(|path| PathBuf::from(crate::pomo::percent_decode(path)))
        .next();
    Ok(picked)
}
