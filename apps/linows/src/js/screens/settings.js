import { getConfig, setConfig, requestIndexRefresh, reloadConfig, listFonts } from '../ipc.js';
import * as banner from '../components/banner.js';
import * as platform from '../platform.js';

let screen = null;
let active = false;
let activeTab = 'appearance';
let onExit = null;

const TABS = ['appearance', 'shortcuts', 'advanced'];

// Maps config keys to CSS custom property update functions.
// Each slider with data-key drives live CSS + config persistence.
const BLUR_PRESETS = {
  high_contrast: { ui_blur_radius: 12, ui_blur_opacity: 0.95 },
  balanced: { ui_blur_radius: 20, ui_blur_opacity: 0.80 },
  soft: { ui_blur_radius: 32, ui_blur_opacity: 0.60 },
};

const CSS_MAP = {
  ui_tint_red: applytint,
  ui_tint_green: applytint,
  ui_tint_blue: applytint,
  ui_tint_opacity: applytint,
  ui_blur_radius: (v) => {
    // Use platform-aware blur (native on Windows, CSS on Linux)
    const style = getCurrentBlurStyle();
    platform.applyBlur(parseFloat(v), style);
  },
  ui_blur_opacity: applyBlurOpacity,
  ui_font_size: (v) => {
    document.documentElement.style.setProperty('--font-size', Math.round(parseFloat(v)) + 'px');
  },
  ui_font_red: applyFontColor,
  ui_font_green: applyFontColor,
  ui_font_blue: applyFontColor,
  ui_font_opacity: applyFontColor,
  ui_border_thickness: (v) => {
    document.documentElement.style.setProperty('--border-thickness', v + 'px');
  },
  ui_border_red: applyBorderColor,
  ui_border_green: applyBorderColor,
  ui_border_blue: applyBorderColor,
  ui_border_opacity: applyBorderColor,
};

function switchToCustomTheme() {
  const dd = document.getElementById('settings-theme');
  const menu = dd.querySelector('.settings-dropdown-menu');
  dd.querySelector('.settings-dropdown-label').textContent = 'Custom';
  for (const el of menu.children) el.classList.remove('settings-dropdown-active');
  const customItem = menu.querySelector('[data-value="custom"]');
  if (customItem) customItem.classList.add('settings-dropdown-active');
  saveConfig({ ui_theme: 'custom' });
}

function getCurrentBlurStyle() {
  const dd = document.getElementById('settings-blur-style');
  const active = dd?.querySelector('.settings-dropdown-active');
  return active?.dataset.value || 'high_contrast';
}

export function init(exitFn) {
  onExit = exitFn;
  screen = document.getElementById('settings-screen');

  // Tab clicks
  document.getElementById('settings-tabs').addEventListener('click', (e) => {
    const btn = e.target.closest('.settings-tab');
    if (btn) switchTab(btn.dataset.tab);
  });

  // Theme dropdown (custom)
  const themeDropdown = document.getElementById('settings-theme');
  const themeBtn = themeDropdown.querySelector('.settings-dropdown-btn');
  const themeMenu = themeDropdown.querySelector('.settings-dropdown-menu');

  themeBtn.addEventListener('click', () => {
    themeMenu.hidden = !themeMenu.hidden;
  });

  themeMenu.addEventListener('click', (e) => {
    const item = e.target.closest('.settings-dropdown-item');
    if (!item) return;
    const theme = item.dataset.value;
    themeDropdown.querySelector('.settings-dropdown-label').textContent = item.textContent;
    for (const el of themeMenu.children) el.classList.remove('settings-dropdown-active');
    item.classList.add('settings-dropdown-active');
    themeMenu.hidden = true;
    applyThemePreset(theme);
    saveConfig({ ui_theme: theme });
  });

  // Blur style dropdown — populate labels from platform
  const blurDropdown = document.getElementById('settings-blur-style');
  const blurBtn = blurDropdown.querySelector('.settings-dropdown-btn');
  const blurMenu = blurDropdown.querySelector('.settings-dropdown-menu');
  const blurHint = document.getElementById('settings-blur-style-hint');

  const blurStyles = platform.getBlurStyles();
  blurMenu.innerHTML = '';
  blurStyles.forEach((s, i) => {
    const div = document.createElement('div');
    div.className = 'settings-dropdown-item' + (i === 0 ? ' settings-dropdown-active' : '');
    div.dataset.value = s.value;
    div.textContent = s.label;
    blurMenu.appendChild(div);
  });
  if (blurStyles.length > 0) {
    blurDropdown.querySelector('.settings-dropdown-label').textContent = blurStyles[0].label;
    blurHint.textContent = blurStyles[0].hint;
  }

  blurBtn.addEventListener('click', () => {
    blurMenu.hidden = !blurMenu.hidden;
  });

  blurMenu.addEventListener('click', (e) => {
    const item = e.target.closest('.settings-dropdown-item');
    if (!item) return;
    const style = item.dataset.value;
    blurDropdown.querySelector('.settings-dropdown-label').textContent = item.textContent;
    for (const el of blurMenu.children) el.classList.remove('settings-dropdown-active');
    item.classList.add('settings-dropdown-active');
    blurMenu.hidden = true;
    const styles = platform.getBlurStyles();
    const match = styles.find(s => s.value === style);
    blurHint.textContent = match?.hint || '';

    // Apply preset values to sliders + CSS
    const preset = BLUR_PRESETS[style];
    if (preset) {
      for (const [key, val] of Object.entries(preset)) {
        const row = screen.querySelector(`.settings-row[data-key="${key}"]`);
        if (row) {
          const slider = row.querySelector('.settings-slider');
          const valueEl = row.querySelector('.settings-slider-value');
          slider.value = val;
          valueEl.textContent = formatValue(key, val);
        }
      }
      // Apply blur via platform module
      platform.applyBlur(preset.ui_blur_radius, style);
      if (platform.hasCompositor()) applytint();
      saveConfig({ ...preset, ui_blur_style: style });
    }
  });

  // Close dropdowns when clicking outside
  document.addEventListener('click', (e) => {
    if (!themeDropdown.contains(e.target)) themeMenu.hidden = true;
    if (!blurDropdown.contains(e.target)) blurMenu.hidden = true;
  });

  // Keys that mark the theme as "Custom" when manually changed
  const THEME_KEYS = new Set([
    'ui_tint_red', 'ui_tint_green', 'ui_tint_blue', 'ui_tint_opacity',
    'ui_font_red', 'ui_font_green', 'ui_font_blue', 'ui_font_opacity',
    'ui_border_red', 'ui_border_green', 'ui_border_blue', 'ui_border_opacity',
    'ui_border_thickness',
  ]);

  // All data-key sliders: live preview + save on release
  for (const row of screen.querySelectorAll('.settings-row[data-key]')) {
    const key = row.dataset.key;
    const slider = row.querySelector('.settings-slider');
    const valueEl = row.querySelector('.settings-slider-value');
    if (!slider) continue;

    slider.addEventListener('input', () => {
      const v = parseFloat(slider.value);
      valueEl.textContent = formatValue(key, v);
      if (CSS_MAP[key]) CSS_MAP[key](v);
    });
    slider.addEventListener('change', () => {
      if (THEME_KEYS.has(key)) switchToCustomTheme();
      saveConfig({ [key]: slider.value });
    });
  }

  // Advanced tab controls
  const depthSlider = document.getElementById('settings-scan-depth');
  const depthValue = document.getElementById('settings-scan-depth-value');
  depthSlider.addEventListener('input', () => { depthValue.textContent = depthSlider.value; });
  depthSlider.addEventListener('change', () => { saveConfig({ file_scan_depth: depthSlider.value }); });

  const limitSlider = document.getElementById('settings-file-limit');
  const limitValue = document.getElementById('settings-file-limit-value');
  limitSlider.addEventListener('input', () => { limitValue.textContent = limitSlider.value; });
  limitSlider.addEventListener('change', () => { saveConfig({ file_scan_limit: limitSlider.value }); });

  document.getElementById('settings-lazy-indexing').addEventListener('change', (e) => {
    saveConfig({ lazy_indexing_enabled: e.target.checked ? 'true' : 'false' });
  });

  document.getElementById('settings-refresh-index').addEventListener('click', async () => {
    try {
      await reloadConfig();
      await requestIndexRefresh();
      banner.show('Index refresh started', 'success', 1.2);
    } catch { banner.show('Refresh failed', 'error', 1.5); }
  });

  // Font name input with autocomplete
  const fontNameInput = document.getElementById('settings-font-name');
  const fontSuggestions = document.getElementById('settings-font-suggestions');
  let fontList = [];
  let fontActiveIdx = -1;

  listFonts().then(fonts => { fontList = fonts; }).catch(() => {});

  function applyFontName(name) {
    fontNameInput.value = name;
    document.documentElement.style.setProperty('--font-family', `"${name}", system-ui, sans-serif`);
    saveConfig({ ui_font_name: name });
    fontSuggestions.hidden = true;
  }

  fontNameInput.addEventListener('input', () => {
    if (fontList.length === 0) {
      fontSuggestions.hidden = true;
      return;
    }
    const q = fontNameInput.value.trim().toLowerCase();
    const matches = (q ? fontList.filter(f => f.toLowerCase().includes(q)) : fontList);
    if (matches.length === 0) {
      fontSuggestions.hidden = true;
      return;
    }
    fontActiveIdx = -1;
    fontSuggestions.innerHTML = '';
    for (const name of matches) {
      const div = document.createElement('div');
      div.className = 'settings-font-suggestion';
      div.textContent = name;
      div.addEventListener('mousedown', (e) => {
        e.preventDefault();
        applyFontName(name);
      });
      fontSuggestions.appendChild(div);
    }
    fontSuggestions.hidden = false;
  });

  fontNameInput.addEventListener('keydown', (e) => {
    if (fontSuggestions.hidden) return;
    const items = fontSuggestions.children;
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      fontActiveIdx = Math.min(fontActiveIdx + 1, items.length - 1);
      updateFontActive(items);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      fontActiveIdx = Math.max(fontActiveIdx - 1, 0);
      updateFontActive(items);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (fontActiveIdx >= 0 && items[fontActiveIdx]) {
        applyFontName(items[fontActiveIdx].textContent);
      } else {
        applyFontName(fontNameInput.value.trim());
      }
    } else if (e.key === 'Escape') {
      fontSuggestions.hidden = true;
    }
  });

  function updateFontActive(items) {
    for (let i = 0; i < items.length; i++) {
      items[i].classList.toggle('active', i === fontActiveIdx);
    }
    if (fontActiveIdx >= 0 && items[fontActiveIdx]) {
      items[fontActiveIdx].scrollIntoView({ block: 'nearest' });
    }
  }

  fontNameInput.addEventListener('focus', () => {
    fontNameInput.dispatchEvent(new Event('input'));
  });

  fontNameInput.addEventListener('blur', () => {
    setTimeout(() => { fontSuggestions.hidden = true; }, 150);
  });

  fontNameInput.addEventListener('change', () => {
    const name = fontNameInput.value.trim();
    if (name) applyFontName(name);
  });

  // Save Config button
  document.getElementById('settings-save-btn').addEventListener('click', async () => {
    try {
      await reloadConfig();
      const msg = document.getElementById('settings-save-msg');
      msg.textContent = 'Saved';
      setTimeout(() => { msg.textContent = ''; }, 1600);
    } catch {
      const msg = document.getElementById('settings-save-msg');
      msg.textContent = 'Save failed';
      msg.style.background = 'rgba(220, 60, 60, 0.42)';
      setTimeout(() => { msg.textContent = ''; msg.style.background = ''; }, 1600);
    }
  });
}

export function isActive() { return active; }

export async function enter(contentArea, searchBar) {
  active = true;
  contentArea.style.display = 'none';
  searchBar.style.display = 'none';
  screen.style.display = '';
  const hint = document.querySelector('#hint-bar span');
  if (hint) hint.textContent = 'Tab switch tabs \u2022 Esc back';
  await loadConfig();
}

export function exit(contentArea, searchBar) {
  active = false;
  screen.style.display = 'none';
  contentArea.style.display = '';
  searchBar.style.display = '';
  if (onExit) onExit();
}

export function handleKey(e) {
  if (!active) return false;

  if (e.key === 'Escape') {
    e.preventDefault();
    exit(
      document.getElementById('search-content'),
      document.getElementById('search-bar'),
    );
    return true;
  }

  if (e.key === 'Tab' || (e.code === 'Tab' && e.key === 'Unidentified')) {
    // Don't intercept Tab when a text input is focused
    if (document.activeElement?.tagName === 'INPUT' && document.activeElement.type === 'text') return false;
    e.preventDefault();
    const dir = e.shiftKey ? -1 : 1;
    const idx = TABS.indexOf(activeTab);
    switchTab(TABS[(idx + dir + TABS.length) % TABS.length]);
    return true;
  }

  return false;
}

export async function restoreOnStartup() {
  try {
    const map = await loadConfigMap();

    // Restore theme preset — preset values drive tint/font/border
    const theme = map.ui_theme || '';
    applyThemePreset(theme);

    // "custom" theme: restore individual overrides from config
    if (theme === 'custom') {
      applyTintFromMap(map);
      applyFontColorFromMap(map);
      applyBorderFromMap(map);
    }

    // Font name/size are always restored (not theme-dependent)
    if (map.ui_font_size) CSS_MAP.ui_font_size(map.ui_font_size);
    if (map.ui_font_name) {
      document.documentElement.style.setProperty('--font-family', `"${map.ui_font_name}", system-ui, sans-serif`);
    }
    if (map.ui_blur_radius) {
      const blurStyle = map.ui_blur_style || 'high_contrast';
      platform.applyBlur(parseFloat(map.ui_blur_radius), blurStyle);
    }
  } catch {
    // Config may not exist yet
  }
}

// --- Internal ---

function switchTab(tabId) {
  if (!TABS.includes(tabId)) return;
  activeTab = tabId;
  for (const btn of document.getElementById('settings-tabs').children) {
    btn.classList.toggle('settings-tab-active', btn.dataset.tab === tabId);
  }
  for (const tab of TABS) {
    const panel = document.getElementById('settings-tab-' + tab);
    if (panel) panel.hidden = tab !== tabId;
  }
}

async function loadConfigMap() {
  const cfg = await getConfig();
  const map = {};
  for (const entry of cfg.entries) map[entry.key] = entry.value;
  return map;
}

async function loadConfig() {
  try {
    const cfg = await getConfig();
    document.getElementById('settings-config-path').textContent = cfg.path;

    const map = {};
    for (const entry of cfg.entries) map[entry.key] = entry.value;

    // Theme dropdown (custom)
    const currentTheme = document.documentElement.getAttribute('data-theme') || '';
    const themeDD = document.getElementById('settings-theme');
    const activeItem = themeDD.querySelector(`.settings-dropdown-item[data-value="${currentTheme}"]`);
    if (activeItem) {
      themeDD.querySelector('.settings-dropdown-label').textContent = activeItem.textContent;
      for (const el of themeDD.querySelector('.settings-dropdown-menu').children) el.classList.remove('settings-dropdown-active');
      activeItem.classList.add('settings-dropdown-active');
    }

    // Blur style dropdown
    const blurStyle = map.ui_blur_style || 'high_contrast';
    const blurDD = document.getElementById('settings-blur-style');
    const blurActiveItem = blurDD.querySelector(`.settings-dropdown-item[data-value="${blurStyle}"]`);
    if (blurActiveItem) {
      blurDD.querySelector('.settings-dropdown-label').textContent = blurActiveItem.textContent;
      for (const el of blurDD.querySelector('.settings-dropdown-menu').children) el.classList.remove('settings-dropdown-active');
      blurActiveItem.classList.add('settings-dropdown-active');
      const styles = platform.getBlurStyles();
      const match = styles.find(s => s.value === blurStyle);
      document.getElementById('settings-blur-style-hint').textContent = match?.hint || '';
    }

    // Font name
    const fontName = map.ui_font_name || 'system-ui';
    document.getElementById('settings-font-name').value = fontName;

    // Populate all data-key sliders
    // If a built-in theme is active, use preset values for theme keys
    const activeTheme = map.ui_theme || '';
    const preset = (activeTheme && activeTheme !== 'custom') ? THEME_PRESETS[activeTheme] : null;
    for (const row of screen.querySelectorAll('.settings-row[data-key]')) {
      const key = row.dataset.key;
      const slider = row.querySelector('.settings-slider');
      const valueEl = row.querySelector('.settings-slider-value');
      if (!slider) continue;

      const presetVal = preset?.[key];
      const val = presetVal !== undefined ? presetVal : map[key];
      if (val !== undefined) {
        slider.value = val;
        valueEl.textContent = formatValue(key, parseFloat(val));
      }
    }

    // Advanced tab
    const depth = map.file_scan_depth || '4';
    document.getElementById('settings-scan-depth').value = depth;
    document.getElementById('settings-scan-depth-value').textContent = depth;

    const limit = map.file_scan_limit || '8000';
    document.getElementById('settings-file-limit').value = limit;
    document.getElementById('settings-file-limit-value').textContent = limit;

    document.getElementById('settings-lazy-indexing').checked = map.lazy_indexing_enabled !== 'false';
  } catch (err) {
    console.error('Failed to load config:', err);
  }
}

// --- Theme presets ---

// Theme preset definitions matching theme.css custom properties.
// Each maps to the raw slider values stored in config.
const THEME_PRESETS = {
  '': { // Catppuccin Mocha — base #1e1e2e, surface0 #313244
    ui_tint_red: 0.12, ui_tint_green: 0.12, ui_tint_blue: 0.18, ui_tint_opacity: 0.95,
    ui_font_red: 0.81, ui_font_green: 0.80, ui_font_blue: 0.90, ui_font_opacity: 1.0,
    ui_border_red: 0.58, ui_border_green: 0.58, ui_border_blue: 0.65, ui_border_opacity: 0.18,
    ui_border_thickness: 1.0,
  },
  'tokyo-night': {
    ui_tint_red: 0.10, ui_tint_green: 0.11, ui_tint_blue: 0.15, ui_tint_opacity: 0.60,
    ui_font_red: 0.84, ui_font_green: 0.87, ui_font_blue: 0.96, ui_font_opacity: 0.98,
    ui_border_red: 0.66, ui_border_green: 0.69, ui_border_blue: 0.84, ui_border_opacity: 0.10,
    ui_border_thickness: 1.0,
  },
  'rose-pine': {
    ui_tint_red: 0.10, ui_tint_green: 0.09, ui_tint_blue: 0.14, ui_tint_opacity: 0.58,
    ui_font_red: 0.95, ui_font_green: 0.93, ui_font_blue: 0.91, ui_font_opacity: 0.98,
    ui_border_red: 0.88, ui_border_green: 0.87, ui_border_blue: 0.96, ui_border_opacity: 0.10,
    ui_border_thickness: 1.0,
  },
  'gruvbox': {
    ui_tint_red: 0.16, ui_tint_green: 0.16, ui_tint_blue: 0.16, ui_tint_opacity: 0.60,
    ui_font_red: 0.93, ui_font_green: 0.89, ui_font_blue: 0.79, ui_font_opacity: 0.98,
    ui_border_red: 0.92, ui_border_green: 0.86, ui_border_blue: 0.70, ui_border_opacity: 0.10,
    ui_border_thickness: 1.0,
  },
  'dracula': {
    ui_tint_red: 0.16, ui_tint_green: 0.16, ui_tint_blue: 0.21, ui_tint_opacity: 0.58,
    ui_font_red: 0.97, ui_font_green: 0.97, ui_font_blue: 0.98, ui_font_opacity: 0.98,
    ui_border_red: 0.97, ui_border_green: 0.97, ui_border_blue: 0.95, ui_border_opacity: 0.10,
    ui_border_thickness: 1.0,
  },
  'kanagawa': {
    ui_tint_red: 0.09, ui_tint_green: 0.09, ui_tint_blue: 0.11, ui_tint_opacity: 0.60,
    ui_font_red: 0.87, ui_font_green: 0.86, ui_font_blue: 0.79, ui_font_opacity: 0.98,
    ui_border_red: 0.86, ui_border_green: 0.84, ui_border_blue: 0.73, ui_border_opacity: 0.10,
    ui_border_thickness: 1.0,
  },
};

function applyThemePreset(themeId) {
  // "custom" = user-modified values, don't override anything
  if (themeId === 'custom') return;

  if (themeId) {
    document.documentElement.setAttribute('data-theme', themeId);
  } else {
    document.documentElement.removeAttribute('data-theme');
  }

  const preset = THEME_PRESETS[themeId];
  if (!preset) return;

  // Update slider DOMs first (so getSliderVal reads correct values)
  for (const row of screen.querySelectorAll('.settings-row[data-key]')) {
    const key = row.dataset.key;
    if (preset[key] !== undefined) {
      const slider = row.querySelector('.settings-slider');
      const valueEl = row.querySelector('.settings-slider-value');
      if (slider) {
        slider.value = preset[key];
        valueEl.textContent = formatValue(key, preset[key]);
      }
    }
  }

  // Now apply CSS from slider values
  applytint();
  applyFontColor();
  applyBorderColor();
  if (preset.ui_font_size !== undefined) CSS_MAP.ui_font_size(preset.ui_font_size);
  if (preset.ui_border_thickness !== undefined) CSS_MAP.ui_border_thickness(preset.ui_border_thickness);
}

// --- CSS application ---

function getSliderVal(key) {
  const row = screen?.querySelector(`.settings-row[data-key="${key}"]`);
  if (!row) return 0;
  return parseFloat(row.querySelector('.settings-slider')?.value || 0);
}

function applytint() {
  const r = Math.round(getSliderVal('ui_tint_red') * 255);
  const g = Math.round(getSliderVal('ui_tint_green') * 255);
  const b = Math.round(getSliderVal('ui_tint_blue') * 255);
  // No compositor = force fully opaque background (no desktop to show through)
  if (!platform.hasCompositor()) {
    document.documentElement.style.setProperty('--bg-tint', `rgb(${r}, ${g}, ${b})`);
    return;
  }
  const tintA = getSliderVal('ui_tint_opacity');
  const blurA = getSliderVal('ui_blur_opacity') || 0.95;
  const a = tintA * blurA;
  document.documentElement.style.setProperty('--bg-tint', `rgba(${r}, ${g}, ${b}, ${a.toFixed(2)})`);
}

function applyFontColor() {
  const r = Math.round(getSliderVal('ui_font_red') * 255);
  const g = Math.round(getSliderVal('ui_font_green') * 255);
  const b = Math.round(getSliderVal('ui_font_blue') * 255);
  const a = getSliderVal('ui_font_opacity');
  document.documentElement.style.setProperty('--font-color', `rgba(${r}, ${g}, ${b}, ${a.toFixed(2)})`);
}

function applyBorderColor() {
  const r = Math.round(getSliderVal('ui_border_red') * 255);
  const g = Math.round(getSliderVal('ui_border_green') * 255);
  const b = Math.round(getSliderVal('ui_border_blue') * 255);
  const a = getSliderVal('ui_border_opacity');
  document.documentElement.style.setProperty('--border-color', `rgba(${r}, ${g}, ${b}, ${a.toFixed(2)})`);
}

function applyBlurOpacity() {
  // Re-apply tint with blur opacity as a multiplier on the tint alpha
  applytint();
}

// Used by restoreOnStartup (no DOM sliders available yet)
function applyTintFromMap(map) {
  const r = Math.round((parseFloat(map.ui_tint_red ?? 0.12)) * 255);
  const g = Math.round((parseFloat(map.ui_tint_green ?? 0.12)) * 255);
  const b = Math.round((parseFloat(map.ui_tint_blue ?? 0.18)) * 255);
  if (!platform.hasCompositor()) {
    document.documentElement.style.setProperty('--bg-tint', `rgb(${r}, ${g}, ${b})`);
    return;
  }
  const tintA = parseFloat(map.ui_tint_opacity ?? 0.95);
  const blurA = parseFloat(map.ui_blur_opacity ?? 0.95);
  const a = tintA * blurA;
  document.documentElement.style.setProperty('--bg-tint', `rgba(${r}, ${g}, ${b}, ${a.toFixed(2)})`);
}

function applyFontColorFromMap(map) {
  if (!map.ui_font_red && !map.ui_font_green && !map.ui_font_blue) return;
  const r = Math.round((parseFloat(map.ui_font_red) || 0.96) * 255);
  const g = Math.round((parseFloat(map.ui_font_green) || 0.96) * 255);
  const b = Math.round((parseFloat(map.ui_font_blue) || 0.98) * 255);
  const a = parseFloat(map.ui_font_opacity) || 0.96;
  document.documentElement.style.setProperty('--font-color', `rgba(${r}, ${g}, ${b}, ${a.toFixed(2)})`);
}

function applyBorderFromMap(map) {
  if (map.ui_border_thickness) {
    document.documentElement.style.setProperty('--border-thickness', map.ui_border_thickness + 'px');
  }
  if (!map.ui_border_red && !map.ui_border_green && !map.ui_border_blue) return;
  const r = Math.round((parseFloat(map.ui_border_red) || 1.0) * 255);
  const g = Math.round((parseFloat(map.ui_border_green) || 1.0) * 255);
  const b = Math.round((parseFloat(map.ui_border_blue) || 1.0) * 255);
  const a = parseFloat(map.ui_border_opacity) || 0.12;
  document.documentElement.style.setProperty('--border-color', `rgba(${r}, ${g}, ${b}, ${a.toFixed(2)})`);
}

// --- Formatting ---

function formatValue(key, v) {
  if (key === 'ui_blur_radius') return Math.round(v).toString();
  return v.toFixed(2);
}

async function saveConfig(updates) {
  try {
    const list = Object.entries(updates).map(([key, value]) => ({ key, value: String(value) }));
    await setConfig(list);
  } catch (err) {
    console.error('Failed to save config:', err);
  }
}
