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
  if (isWindows()) {
    const r = WINDOWS_BLUR_RADIUS[style] ?? WINDOWS_BLUR_RADIUS.balanced;
    document.documentElement.style.setProperty('--blur-radius', r + 'px');
  } else {
    // Linux: CSS backdrop-filter (works if compositor supports it)
    const r = hasCompositor() ? Math.round(radius) : 0;
    document.documentElement.style.setProperty('--blur-radius', r + 'px');
  }
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
