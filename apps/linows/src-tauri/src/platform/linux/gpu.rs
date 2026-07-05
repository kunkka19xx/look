//! WebKitGTK GPU-acceleration policy and render tweaks on Linux.
//!
//! Centralises the workarounds that keep the webview from crashing or
//! misrendering across the wide variety of Linux GPU / Mesa / compositor
//! stacks. Startup-ordered helpers (env vars) must run before any threads
//! spawn; the API-level helpers run inside Tauri's `.setup()` once the
//! webview exists.

use crate::config;
use crate::consts;
use tauri::Manager;

/// Detect if running inside a VM with a virtual GPU that doesn't support EGL.
/// Returns true if GPU acceleration should be disabled.
/// SAFETY: Sets env vars - must be called before any threads are spawned.
pub fn detect_and_disable_virtual_gpu() -> bool {
    let detected = if !std::path::Path::new("/dev/dri").exists() {
        true
    } else {
        // /dev/dri exists but the driver may not support EGL (common in VMs).
        // Check for known virtual GPU drivers via /dev/dri/card* sysfs.
        std::fs::read_dir("/sys/class/drm")
            .map(|entries| {
                entries.filter_map(Result::ok).any(|e| {
                    let driver = e.path().join("device/driver");
                    if let Ok(target) = std::fs::read_link(&driver) {
                        let name = target
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        matches!(
                            name.as_str(),
                            "virtio-pci"
                                | "virtio_gpu"
                                | "qxl"
                                | "bochs-drm"
                                | "vmwgfx"
                                | "vboxvideo"
                                | "cirrus"
                        )
                    } else {
                        false
                    }
                })
            })
            .unwrap_or(false)
    };
    if detected {
        unsafe {
            std::env::set_var("WEBKIT_DISABLE_GPU", "1");
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }
    detected
}

/// Read the `arch_disable_gpu` config key. User-opt-in workaround for
/// the WebKitGTK ghost-rendering bug observed on Arch GNOME 50 + webkit
/// 2.52.3 (other stacks with same webkit version, e.g. Ubuntu 26.04, are
/// unaffected, so we don't auto-detect - toggle lives in Advanced settings).
pub fn arch_disable_gpu_from_config() -> bool {
    let path = config::config_file_path();
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return false;
    };
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=')
            && k.trim() == "arch_disable_gpu"
        {
            return v.trim().eq_ignore_ascii_case("true");
        }
    }
    false
}

/// Disable hardware acceleration via WebKitGTK API for VM GPUs.
/// Env vars (WEBKIT_DISABLE_GPU) are ignored by newer WebKitGTK versions,
/// so we set the policy at the API level before the first render.
pub fn disable_gpu_acceleration(app: &tauri::App) {
    if let Some(webview) = app.get_webview_window(consts::MAIN_WINDOW) {
        let _ = webview.with_webview(|wv| {
            use webkit2gtk::SettingsExt;
            let inner = wv.inner();
            if let Some(settings) = webkit2gtk::WebViewExt::settings(&inner) {
                settings.set_hardware_acceleration_policy(
                    webkit2gtk::HardwareAccelerationPolicy::Never,
                );
            }
        });
    }
}

/// Disable WebKitGTK smooth scrolling on X11.
///
/// Why: GTK3 issue #3287 - on X11 with GDK_SMOOTH_SCROLL_MASK enabled, the
/// first scroll event after the cursor enters a window arrives with delta=0
/// (GDK has no previous value to subtract), so the first wheel notch is
/// effectively dropped. On tiling WMs like i3 the launcher pops up at a new
/// position every show, so users cross the window edge every session and hit
/// this bug every session ("scroll feels frozen, then works"). Switching to
/// discrete scroll events sidesteps the smooth-delta=0 path entirely.
///
/// Wayland uses a different event delivery path and isn't affected, so this
/// is X11-only.
pub fn disable_smooth_scrolling_x11(app: &tauri::App) {
    if let Some(webview) = app.get_webview_window(consts::MAIN_WINDOW) {
        let _ = webview.with_webview(|wv| {
            use webkit2gtk::SettingsExt;
            let inner = wv.inner();
            if let Some(settings) = webkit2gtk::WebViewExt::settings(&inner) {
                settings.set_enable_smooth_scrolling(false);
            }
        });
    }
}
