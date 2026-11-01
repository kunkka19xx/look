//! The folder and image pickers. gpui's own path prompt hands the portal the
//! focused surface as the dialog's parent through xdg_foreign, and a
//! layer-shell surface is no xdg toplevel: the compositor answers with a
//! fatal protocol error and the Wayland connection dies with it. On Linux
//! the request goes to the portal directly, with no parent; elsewhere
//! gpui's prompt serves.
//!
//! A layer surface also outranks every toplevel, the dialog among them, so
//! the launcher hides for the dialog's run and comes back after it, on the
//! screen it left (see `Retained`). The pick lands in whichever launcher
//! is up by then.

use std::path::PathBuf;

use gpui::{App, Context, Task};

use crate::banner;
use crate::launcher::Launcher;
use crate::launchpad::Tone;

/// What the dialog is for; each has its title and its filter.
#[derive(Clone, Copy)]
enum Want {
    Folder,
    Image,
}

impl Want {
    /// The banner's lead when no dialog could open.
    fn missing(self) -> &'static str {
        match self {
            Want::Folder => "No folder dialog",
            Want::Image => "No file dialog",
        }
    }
}

/// What the result lands in, once the dialog is down.
pub type Then = dyn FnOnce(&mut Launcher, PathBuf, &mut Context<Launcher>);

/// Open the system folder dialog with `prompt` on its accept button; `then`
/// gets the pick. Backing out is silent, no dialog at all is a banner.
pub fn folder(
    prompt: &'static str,
    cx: &mut Context<Launcher>,
    then: impl FnOnce(&mut Launcher, PathBuf, &mut Context<Launcher>) + 'static,
) {
    pick(Want::Folder, prompt, cx, Box::new(then))
}

/// Open the system file dialog narrowed to pictures.
pub fn image(
    prompt: &'static str,
    cx: &mut Context<Launcher>,
    then: impl FnOnce(&mut Launcher, PathBuf, &mut Context<Launcher>) + 'static,
) {
    pick(Want::Image, prompt, cx, Box::new(then))
}

fn pick(want: Want, prompt: &'static str, cx: &mut Context<Launcher>, then: Box<Then>) {
    let picked = dialog(want, prompt, cx);
    cx.spawn(async move |_, cx| {
        let picked = picked.await;
        let Some(launcher) = cx.update(|cx| crate::launcher(cx)) else {
            return;
        };
        launcher.update(cx, |launcher, cx| match picked {
            Ok(Some(path)) => then(launcher, path, cx),
            Ok(None) => {}
            Err(err) => launcher.banner.show(
                format!("{}: {err}", want.missing()),
                Tone::Error,
                banner::SHORT,
                cx,
            ),
        });
    })
    .detach();
}

/// `Ok(None)` is the user backing out; `Err` is no dialog at all.
fn dialog(want: Want, prompt: &'static str, cx: &App) -> Task<Result<Option<PathBuf>, String>> {
    #[cfg(target_os = "linux")]
    {
        let hides = crate::host::layered();
        cx.spawn(async move |cx| {
            if hides {
                cx.update(crate::hide);
            }
            let picked = cx
                .background_executor()
                .spawn(async move { portal_pick(want, prompt).await })
                .await;
            if hides {
                cx.update(crate::show);
            }
            picked
        })
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
