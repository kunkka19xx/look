//! WebKitGTK GPU-acceleration policy and render tweaks on Linux.
//!
//! Centralises the workarounds that keep the webview from crashing or
//! misrendering across the wide variety of Linux GPU / Mesa / compositor
//! stacks. Startup-ordered helpers (env vars) must run before any threads
//! spawn; the API-level helpers run inside Tauri's `.setup()` once the
//! webview exists.

use crate::config;
use crate::consts;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Manager;

/// Set once by detect_and_disable_virtual_gpu() at startup; read by
/// get_platform so the frontend can force the blur fallback in VMs
/// (software rendering + backdrop-filter ghost-renders).
static VIRTUAL_GPU: AtomicBool = AtomicBool::new(false);

pub fn virtual_gpu_detected() -> bool {
    VIRTUAL_GPU.load(Ordering::Relaxed)
}

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
        VIRTUAL_GPU.store(true, Ordering::Relaxed);
        unsafe {
            std::env::set_var("WEBKIT_DISABLE_GPU", "1");
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }
    detected
}

/// Read the `disable_gpu_compositing` config key, falling back to the
/// `arch_disable_gpu` name it shipped under. User-opt-in workaround for the
/// WebKitGTK ghost-rendering bug first seen on Arch GNOME 50 + webkit 2.52.3,
/// since reported on Ubuntu and in VMs; the affected stacks share no property
/// we can test for, so it stays a toggle in Advanced settings.
pub fn disable_gpu_from_config() -> bool {
    let path = config::config_file_path();
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return false;
    };
    // Last value wins, as get_config's entries do once the frontend folds them
    // into a map - a duplicated key must not mean one thing here and another in
    // Settings.
    let mut current = None;
    let mut legacy = None;
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let on = v.trim().eq_ignore_ascii_case("true");
        match k.trim() {
            consts::KEY_DISABLE_GPU => current = Some(on),
            consts::KEY_DISABLE_GPU_LEGACY => legacy = Some(on),
            _ => {}
        }
    }
    current.or(legacy).unwrap_or(false)
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

/// Turn off WebKit subsystems this launcher never uses, so their per-web-process
/// caches and init cost never land in `WebKitWebProcess` memory. All confirmed
/// unused: no HTML5 media (audio is rodio in the backend), no WebGL (only a 2D
/// canvas), no getUserMedia/WebRTC, no plugins, and no back/forward navigation
/// to justify the page (bf) cache. localStorage and JavaScript stay on.
///
/// This trims baseline, not the irreducible engine footprint; WebKitGTK's shared
/// libraries dominate and can't be unloaded.
pub fn trim_memory_features(app: &tauri::App) {
    if let Some(webview) = app.get_webview_window(consts::MAIN_WINDOW) {
        let _ = webview.with_webview(|wv| {
            use webkit2gtk::SettingsExt;
            let inner = wv.inner();
            if let Some(s) = webkit2gtk::WebViewExt::settings(&inner) {
                // No page navigation in a single-view launcher.
                s.set_enable_page_cache(false);
                // GPU/graphics features unused by the UI (2D canvas is unaffected).
                s.set_enable_webgl(false);
                // Full HTML5 media stack: unused, audio goes through rodio.
                s.set_enable_webaudio(false);
                s.set_enable_media(false);
                s.set_enable_mediasource(false);
                s.set_enable_media_capabilities(false);
                s.set_enable_encrypted_media(false);
                s.set_enable_media_stream(false);
                s.set_enable_webrtc(false);
                // Legacy / privacy-noise features.
                s.set_enable_offline_web_application_cache(false);
                s.set_enable_hyperlink_auditing(false);
                s.set_enable_dns_prefetching(false);
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
