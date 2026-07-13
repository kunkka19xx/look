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
}

// True when the blur fallback is forced by the platform (VM GPU) rather than
// the arch_disable_blur config toggle. Settings must not remove the
// attribute in this case.
export function blurForcedOff() {
    return info?.virtual_gpu ?? false;
}

// The floating inner-gap layout depends on see-through gaps and frosted
// tiles, so it needs WebKitGTK to composite translucency faithfully. That
// rules out the same environments applytint() degrades on: no compositor
// (bare X11/i3 - "transparent" pixels come out opaque, gaps read as empty
// boxes), the VM software-rendering fallback, and the ghost-rendering
// stacks where blur is dropped (Hyprland auto, Arch toggle). Those render
// the classic framed panel regardless of the inner_gap setting; the config
// value stays untouched and applies again on a capable setup.
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
