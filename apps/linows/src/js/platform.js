import { getPlatform, setWindowEffect } from './ipc.js';

let info = null;

export async function init() {
  try {
    info = await getPlatform();
  } catch {
    info = { os: 'linux', has_compositor: false };
  }
  document.documentElement.setAttribute('data-os', info.os);
}

export function os() {
  return info?.os || 'linux';
}

export function hasCompositor() {
  return info?.has_compositor ?? false;
}

export function isWindows() {
  return info?.os === 'windows';
}

export function isLinux() {
  return info?.os === 'linux';
}

/**
 * Apply blur effect based on platform.
 * - Windows: native Mica/Acrylic via Tauri window effects
 * - Linux + compositor: CSS backdrop-filter
 * - Linux bare (i3): no blur, tint-only
 */
export function applyBlur(radius, style) {
  if (isWindows()) {
    // Map blur style to Windows effect
    const effectMap = {
      high_contrast: 'mica',
      balanced: 'acrylic',
      soft: 'acrylic',
    };
    const effect = effectMap[style] || 'mica';
    setWindowEffect(radius > 0 ? effect : 'none').catch(() => {});
    // Clear CSS blur on Windows (native handles it)
    document.documentElement.style.setProperty('--blur-radius', '0px');
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
