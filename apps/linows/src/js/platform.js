import { getPlatform, setWindowEffect } from './ipc.js';

let info = null;

export async function init() {
    try {
        info = await getPlatform();
    } catch {
        info = { os: 'linux', has_compositor: false, compositor: null };
    }
    document.documentElement.setAttribute('data-os', info.os);
    if (info.compositor) {
        document.documentElement.setAttribute('data-compositor', info.compositor);
    }
    // Mirror of apply_transparency (main.rs) with the same has_compositor
    // semantics: the Rust eval runs once at setup, so a page reload (dev hot
    // reload) would otherwise lose the attribute and square the corners.
    document.documentElement.setAttribute('data-transparent', String(hasCompositor()));
    // Virtual GPU (VM): hardware acceleration is already off backend-side, but
    // software compositing still ghost-renders backdrop-filter layers. Force
    // the blur fallback without touching the user's config.
    if (blurForcedOff()) {
        document.documentElement.setAttribute('data-disable-blur', '');
    }
    if (compositorBlur()) {
        document.documentElement.setAttribute('data-blur', 'compositor');
    }
}

// True when the compositor grants behind-window blur on request. A capability,
// not a setting: it only says whether Blur Opacity has real frost to act on.
export function compositorBlur() {
    return info?.compositor_blur ?? false;
}

// True when the blur fallback is forced by the platform (VM GPU) rather than
// the disable_blur_effect config toggle. Settings must not remove the
// attribute in this case.
export function blurForcedOff() {
    return info?.virtual_gpu ?? false;
}

// The floating inner-gap layout depends on see-through gaps and frosted
// tiles, so it needs WebKitGTK to composite translucency faithfully. That
// rules out: no compositor (bare X11/i3 - "transparent" pixels come out
// opaque, gaps read as empty boxes), the VM software-rendering fallback, and
// the ghost-rendering stacks where blur is dropped (Hyprland auto, Arch
// toggle). Those render the classic framed panel regardless of the inner_gap
// setting; the config value stays untouched and applies again on a capable
// setup.
//
// The frosting is gone (layout.css made backdrop-filter Windows-only) but the
// gate is not just about frosting: .bar-free drops the window's opaque tint,
// and on a stack that leaves stale pixels in the window buffer that opaque
// fill is the only thing overwriting them each frame. Verified 2026-09-06 in
// the Arch VM - allowing floating there painted the launchpad and a stale
// results list on top of each other. Do not widen this without a fix for the
// buffer itself.
export function floatingSupported() {
    return (
        hasCompositor() &&
        !blurForcedOff() &&
        compositor() !== 'hyprland' &&
        !document.documentElement.hasAttribute('data-disable-blur')
    );
}

export function os() {
    return info?.os || 'linux';
}

export function hasCompositor() {
    return info?.has_compositor ?? false;
}

export function compositor() {
    return info?.compositor || null;
}

export function isWindows() {
    return info?.os === 'windows';
}

export function isLinux() {
    return info?.os === 'linux';
}

// Read live off the query list so an OS toggle mid-session takes effect.
// Windows is excluded to match the CSS: its reduce-motion flag tracks the
// "best performance" visual-effects preset, not motion sensitivity.
const reduceMotion = matchMedia('(prefers-reduced-motion: reduce)');

export function prefersReducedMotion() {
    return reduceMotion.matches && !isWindows();
}

// For surfaces that have to act on the switch rather than read it per frame.
export function onReducedMotionChange(callback) {
    reduceMotion.addEventListener('change', callback);
}

// Ctrl+Shift+Enter target: exes and look-cmd:// applets. ms-settings: pages
// have no elevated form, so neither gesture nor hint is offered for them.
export function canRunElevated(item) {
    return isWindows() && item?.kind === 'app' && !item.path?.startsWith('ms-settings:');
}

// The name of the platform's own file manager, for a Reveal entry that has no
// declared `file_manager` to name. Null on Linux: the desktop's handler has no
// one name every distro agrees on, so the entry keeps its plain wording.
export function systemFileManager() {
    return isWindows() ? 'Explorer' : null;
}

// Windows calls it the Recycle Bin; Linux/macOS call it the Trash. Used for
// user-facing strings so the banner/confirm copy matches the OS.
export function trashLabel() {
    return isWindows() ? 'Recycle Bin' : 'Trash';
}

// Windows blur styles map to CSS backdrop-filter radii. We deliberately do
// NOT use native Mica/Acrylic via tauri's `set_effects` - that path
// reconfigures DWM and brings back the sharp rectangular outline outside
// the CSS-clipped rounded silhouette (DWM can't round transparent windows).
// CSS backdrop-filter is supported in WebView2 and respects our border-radius.
const WINDOWS_BLUR_RADIUS = {
    high_contrast: 30,
    balanced: 20,
    soft: 12,
};

/**
 * Apply blur effect based on platform.
 * - Windows: CSS backdrop-filter, strength chosen by Blur Style preset
 * - Linux + compositor: CSS backdrop-filter, strength from `radius` arg
 * - Linux bare (i3): no blur, tint-only
 */
export function applyBlur(radius, style) {
    const r = isWindows()
        ? (WINDOWS_BLUR_RADIUS[style] ?? WINDOWS_BLUR_RADIUS.balanced)
        : hasCompositor()
          ? Math.round(radius)
          : 0;
    document.documentElement.style.setProperty('--blur-radius', r + 'px');
}

/**
 * Get available blur style options for the current platform.
 */
export function getBlurStyles() {
    if (isWindows()) {
        return [
            { value: 'high_contrast', label: 'Mica', hint: 'Windows 11 native blur' },
            { value: 'balanced', label: 'Acrylic', hint: 'Translucent with blur' },
            { value: 'soft', label: 'Acrylic (Soft)', hint: 'Lightest acrylic' },
        ];
    }
    return [
        { value: 'high_contrast', label: 'High Contrast', hint: 'Darkest and most readable' },
        { value: 'balanced', label: 'Balanced', hint: 'Default translucency' },
        { value: 'soft', label: 'Soft', hint: 'Lightest, most transparent' },
    ];
}
