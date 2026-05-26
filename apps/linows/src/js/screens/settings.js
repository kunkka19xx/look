import { getConfig, setConfig, forceIndexRefresh, reloadConfig, resetConfig, listFonts, pickFolder, pickImage, setAutostart, getAutostart, listCandidateDrives } from '../ipc.js';
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
  high_contrast: { ui_blur_opacity: 0.95 },
  balanced: { ui_blur_opacity: 0.80 },
  soft: { ui_blur_opacity: 0.60 },
};

const CSS_MAP = {
  ui_tint_red: applytint,
  ui_tint_green: applytint,
  ui_tint_blue: applytint,
  ui_tint_opacity: applytint,
  ui_bg_opacity: (v) => {
    document.documentElement.style.setProperty('--bg-image-opacity', v);
  },
  ui_bg_blur: (v) => {
    document.documentElement.style.setProperty('--bg-image-blur', parseFloat(v).toFixed(1) + 'px');
  },
  ui_blur_opacity: applyBlurOpacity,
  settings_blur_multiplier: applyBlurOpacity,
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

function markCustomTheme() {
  const dd = document.getElementById('settings-theme');
  const menu = dd.querySelector('.settings-dropdown-menu');
  dd.querySelector('.settings-dropdown-label').textContent = 'Custom';
  for (const el of menu.children) el.classList.remove('settings-dropdown-active');
  const customItem = menu.querySelector('[data-value="custom"]');
  if (customItem) customItem.classList.add('settings-dropdown-active');
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
      if (platform.hasCompositor()) applytint();
      // Drive actual CSS --blur-radius from the chosen style (Windows uses
      // a fixed per-style map; Linux uses the provided radius arg).
      platform.applyBlur(0, style);
      saveConfig({ ...preset, ui_blur_style: style });
    }
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
      const updates = { [key]: slider.value };
      if (THEME_KEYS.has(key)) {
        markCustomTheme();
        updates.ui_theme = 'custom';
      }
      saveConfig(updates);
    });
  }

  // Advanced tab controls
  const depthInput = document.getElementById('settings-scan-depth');
  depthInput.addEventListener('change', () => {
    const v = parseInt(depthInput.value) || 4;
    depthInput.value = Math.max(1, Math.min(12, v));
    saveConfig({ file_scan_depth: depthInput.value });
  });

  const limitInput = document.getElementById('settings-file-limit');
  limitInput.addEventListener('change', () => {
    const v = parseInt(limitInput.value) || 8000;
    limitInput.value = Math.max(500, Math.min(50000, v));
    saveConfig({ file_scan_limit: limitInput.value });
  });

  document.getElementById('settings-lazy-indexing').addEventListener('change', (e) => {
    saveConfig({ lazy_indexing_enabled: e.target.checked ? 'true' : 'false' });
  });

  // Extra scan dirs
  const extraDirsList = document.getElementById('settings-extra-dirs');
  extraDirsList.dataset.empty = 'No extra scan directories';
  document.getElementById('settings-add-scan-dir').addEventListener('click', async () => {
    const folder = await pickFolder();
    if (!folder) return;
    await addDirToConfig('file_scan_extra_roots', folder);
    renderDirList(extraDirsList, 'file_scan_extra_roots');
    renderDriveChips();
  });

  // Detected drives (Windows-only; the chips section is CSS-hidden elsewhere).
  // Renders one chip per non-system fixed drive; toggling adds/removes its
  // root (e.g. "D:\") in file_scan_extra_roots, keeping the chip and the
  // dir-list view in sync.
  renderDriveChips();

  // Skip folders (user-added only, not engine defaults)
  const skipDirsList = document.getElementById('settings-skip-dirs');
  skipDirsList.dataset.empty = 'No excluded folder paths yet';
  document.getElementById('settings-add-skip-dir').addEventListener('click', async () => {
    const folder = await pickFolder();
    if (!folder) return;
    await addDirToConfig('file_exclude_paths', folder);
    renderDirList(skipDirsList, 'file_exclude_paths');
  });

  // Background image
  document.getElementById('settings-choose-bg').addEventListener('click', async () => {
    const file = await pickImage();
    if (!file) return;
    document.getElementById('settings-bg-path').textContent = file;
    applyBackgroundImage(file);
    saveConfig({ ui_bg_image: file });
  });

  document.getElementById('settings-clear-bg').addEventListener('click', () => {
    document.getElementById('settings-bg-path').textContent = 'No background image';
    clearBackgroundImage();
    saveConfig({ ui_bg_image: '' });
  });

  // Background layout dropdown
  const bgLayoutDD = document.getElementById('settings-bg-layout');
  const bgLayoutBtn = bgLayoutDD.querySelector('.settings-dropdown-btn');
  const bgLayoutMenu = bgLayoutDD.querySelector('.settings-dropdown-menu');
  const bgLayoutHint = document.getElementById('settings-bg-layout-hint');
  const BG_LAYOUT_HINTS = { center: 'Original size, centered', fill: 'Fill area and crop edges', stretch: 'Stretch to fill exactly', duplicate: 'Repeat as tiles' };

  bgLayoutBtn.addEventListener('click', () => { bgLayoutMenu.hidden = !bgLayoutMenu.hidden; });
  bgLayoutMenu.addEventListener('click', (e) => {
    const item = e.target.closest('.settings-dropdown-item');
    if (!item) return;
    const val = item.dataset.value;
    bgLayoutDD.querySelector('.settings-dropdown-label').textContent = item.textContent;
    for (const el of bgLayoutMenu.children) el.classList.remove('settings-dropdown-active');
    item.classList.add('settings-dropdown-active');
    bgLayoutMenu.hidden = true;
    bgLayoutHint.textContent = BG_LAYOUT_HINTS[val] || '';
    applyBgLayout(val);
    saveConfig({ ui_bg_layout: val });
  });

  // Log level dropdown
  const logDD = document.getElementById('settings-log-level');
  const logBtn = logDD.querySelector('.settings-dropdown-btn');
  const logMenu = logDD.querySelector('.settings-dropdown-menu');

  logBtn.addEventListener('click', () => { logMenu.hidden = !logMenu.hidden; });
  logMenu.addEventListener('click', (e) => {
    const item = e.target.closest('.settings-dropdown-item');
    if (!item) return;
    logDD.querySelector('.settings-dropdown-label').textContent = item.textContent;
    for (const el of logMenu.children) el.classList.remove('settings-dropdown-active');
    item.classList.add('settings-dropdown-active');
    logMenu.hidden = true;
    saveConfig({ backend_log_level: item.dataset.value });
  });

  // Launch at login
  document.getElementById('settings-arch-disable-gpu').addEventListener('change', (e) => {
    saveConfig({ arch_disable_gpu: e.target.checked ? 'true' : 'false' });
  });

  document.getElementById('settings-arch-disable-blur').addEventListener('change', (e) => {
    const on = e.target.checked;
    if (on) {
      document.documentElement.setAttribute('data-disable-blur', '');
    } else {
      document.documentElement.removeAttribute('data-disable-blur');
    }
    saveConfig({ arch_disable_blur: on ? 'true' : 'false' });
    applytint();
  });

  document.getElementById('settings-launch-login').addEventListener('change', (e) => {
    const enabled = e.target.checked;
    saveConfig({ launch_at_login: enabled ? 'true' : 'false' });
    setAutostart(enabled).catch(() => {});
  });

  // Fresh config
  document.getElementById('settings-fresh-config').addEventListener('click', async () => {
    try {
      await resetConfig();
      await reloadConfig();
      await forceIndexRefresh();
      await loadConfig();
      applyThemePreset('');
      clearBackgroundImage();
      banner.show('Config reset to defaults', 'success', 1.5);
    } catch { banner.show('Reset failed', 'error', 1.5); }
  });

  // Close all dropdowns on outside click
  document.addEventListener('click', (e) => {
    if (!themeDropdown.contains(e.target)) themeMenu.hidden = true;
    if (!blurDropdown.contains(e.target)) blurMenu.hidden = true;
    if (!bgLayoutDD.contains(e.target)) bgLayoutMenu.hidden = true;
    if (!logDD.contains(e.target)) logMenu.hidden = true;
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

  // Save Config button — grab all current UI values and write to .look.config
  document.getElementById('settings-save-btn').addEventListener('click', async () => {
    try {
      const updates = {};

      // Theme
      const themeDD = document.getElementById('settings-theme');
      const activeThemeItem = themeDD.querySelector('.settings-dropdown-active');
      updates.ui_theme = activeThemeItem?.dataset.value ?? '';

      // All data-key sliders (tint, font color, border, blur, bg opacity/blur)
      for (const row of screen.querySelectorAll('.settings-row[data-key]')) {
        const key = row.dataset.key;
        const slider = row.querySelector('.settings-slider');
        if (slider) updates[key] = slider.value;
      }

      // Blur style
      const blurDD = document.getElementById('settings-blur-style');
      const activeBlur = blurDD?.querySelector('.settings-dropdown-active');
      if (activeBlur) updates.ui_blur_style = activeBlur.dataset.value;

      // Font
      updates.ui_font_name = document.getElementById('settings-font-name').value.trim() || 'system-ui';

      // Background
      const bgPath = document.getElementById('settings-bg-path').textContent;
      updates.ui_bg_image = bgPath === 'No background image' ? '' : bgPath;
      const bgLayoutDD = document.getElementById('settings-bg-layout');
      const activeBgLayout = bgLayoutDD?.querySelector('.settings-dropdown-active');
      if (activeBgLayout) updates.ui_bg_layout = activeBgLayout.dataset.value;

      // Advanced: indexing
      updates.file_scan_depth = document.getElementById('settings-scan-depth').value;
      updates.file_scan_limit = document.getElementById('settings-file-limit').value;
      updates.lazy_indexing_enabled = document.getElementById('settings-lazy-indexing').checked ? 'true' : 'false';

      // Advanced: log level
      const logDD = document.getElementById('settings-log-level');
      const activeLog = logDD?.querySelector('.settings-dropdown-active');
      if (activeLog) updates.backend_log_level = activeLog.dataset.value;

      // Advanced: launch at login
      updates.launch_at_login = document.getElementById('settings-launch-login').checked ? 'true' : 'false';

      await saveConfig(updates);
      await reloadConfig();
      await forceIndexRefresh();

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

// Ctrl+Shift+; — reload all values from .look.config file into running app
export async function reloadFromFile() {
  try {
    await reloadConfig();
    const map = await loadConfigMap();

    // Theme
    const theme = map.ui_theme || '';
    applyThemePreset(theme);
    if (theme === 'custom') {
      applyTintFromMap(map);
      applyFontColorFromMap(map);
      applyBorderFromMap(map);
    }

    // Font
    if (map.ui_font_size) CSS_MAP.ui_font_size(map.ui_font_size);
    if (map.ui_font_name) {
      document.documentElement.style.setProperty('--font-family', `"${map.ui_font_name}", system-ui, sans-serif`);
    }

    // Border thickness
    if (map.ui_border_thickness) CSS_MAP.ui_border_thickness(map.ui_border_thickness);

    // Background image
    if (map.ui_bg_image) {
      applyBackgroundImage(map.ui_bg_image);
      if (map.ui_bg_layout) applyBgLayout(map.ui_bg_layout);
      if (map.ui_bg_opacity) CSS_MAP.ui_bg_opacity(map.ui_bg_opacity);
      if (map.ui_bg_blur) CSS_MAP.ui_bg_blur(map.ui_bg_blur);
    } else {
      clearBackgroundImage();
    }

    // If settings screen is open, refresh the UI sliders too
    if (active) await loadConfig();

    // Rebuild index with new config
    await forceIndexRefresh();

    banner.show('Config reloaded from file', 'success', 1.2);
  } catch {
    banner.show('Reload failed', 'error', 1.5);
  }
}

export async function enter(contentArea, searchBar) {
  active = true;
  contentArea.style.display = 'none';
  searchBar.style.display = 'none';
  screen.style.display = '';
  updateSettingsHint();
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

    // Apply Arch blur-disable BEFORE first tint pass so initial render is
    // already opaque if the user toggled it.
    if (map.arch_disable_blur === 'true') {
      document.documentElement.setAttribute('data-disable-blur', '');
    }

    // Restore theme preset — preset values drive tint/font/border
    const theme = map.ui_theme || '';
    applyThemePreset(theme);

    // "custom" theme: restore individual overrides from config
    if (theme === 'custom') {
      applyTintFromMap(map);
      applyFontColorFromMap(map);
      applyBorderFromMap(map);
    }

    // Font
    if (map.ui_font_size) CSS_MAP.ui_font_size(map.ui_font_size);
    if (map.ui_font_name) {
      document.documentElement.style.setProperty('--font-family', `"${map.ui_font_name}", system-ui, sans-serif`);
    }

    // Blur — drive --blur-radius from saved style so the launcher renders
    // with the user's blur on first paint, not only after they open Settings.
    platform.applyBlur(0, map.ui_blur_style || 'high_contrast');

    // Border thickness
    if (map.ui_border_thickness) {
      CSS_MAP.ui_border_thickness(map.ui_border_thickness);
    }

    // Background image
    if (map.ui_bg_image) {
      applyBackgroundImage(map.ui_bg_image);
      if (map.ui_bg_layout) applyBgLayout(map.ui_bg_layout);
      if (map.ui_bg_opacity) CSS_MAP.ui_bg_opacity(map.ui_bg_opacity);
      if (map.ui_bg_blur) CSS_MAP.ui_bg_blur(map.ui_bg_blur);
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
  updateSettingsHint();
}

function updateSettingsHint() {
  const hint = document.querySelector('#hint-bar span');
  if (!hint) return;
  if (activeTab === 'advanced') {
    hint.textContent = 'Save Config applies changes immediately. Ctrl+Shift+; is only needed after editing .look.config manually.';
  } else if (activeTab === 'shortcuts') {
    hint.textContent = 'Tips: t"word for web EN/VI/JA translation \u2022 /kill to force quit apps';
  } else {
    hint.textContent = 'Tab switch tabs \u2022 Esc back';
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
    document.getElementById('settings-scan-depth').value = map.file_scan_depth || '4';
    document.getElementById('settings-file-limit').value = map.file_scan_limit || '8000';
    document.getElementById('settings-lazy-indexing').checked = map.lazy_indexing_enabled !== 'false';
    document.getElementById('settings-arch-disable-gpu').checked = map.arch_disable_gpu === 'true';
    document.getElementById('settings-arch-disable-blur').checked = map.arch_disable_blur === 'true';
    if (map.arch_disable_blur === 'true') {
      document.documentElement.setAttribute('data-disable-blur', '');
    } else {
      document.documentElement.removeAttribute('data-disable-blur');
    }

    // Dir lists
    configCache.file_scan_extra_roots = map.file_scan_extra_roots || '';
    configCache.file_exclude_paths = map.file_exclude_paths || '';
    renderDirList(document.getElementById('settings-extra-dirs'), 'file_scan_extra_roots');
    renderDirList(document.getElementById('settings-skip-dirs'), 'file_exclude_paths');
    renderDriveChips();

    // Background image
    const bgPath = map.ui_bg_image || '';
    document.getElementById('settings-bg-path').textContent = bgPath || 'No background image';

    // Background layout
    const bgLayout = map.ui_bg_layout || 'fill';
    const bgLayoutDD = document.getElementById('settings-bg-layout');
    const bgLayoutItem = bgLayoutDD.querySelector(`.settings-dropdown-item[data-value="${bgLayout}"]`);
    if (bgLayoutItem) {
      bgLayoutDD.querySelector('.settings-dropdown-label').textContent = bgLayoutItem.textContent;
      for (const el of bgLayoutDD.querySelector('.settings-dropdown-menu').children) el.classList.remove('settings-dropdown-active');
      bgLayoutItem.classList.add('settings-dropdown-active');
    }

    // Log level
    const logLevel = map.backend_log_level || 'error';
    const logDD = document.getElementById('settings-log-level');
    const logItem = logDD.querySelector(`.settings-dropdown-item[data-value="${logLevel}"]`);
    if (logItem) {
      logDD.querySelector('.settings-dropdown-label').textContent = logItem.textContent;
      for (const el of logDD.querySelector('.settings-dropdown-menu').children) el.classList.remove('settings-dropdown-active');
      logItem.classList.add('settings-dropdown-active');
    }

    // Launch at login — read actual system state
    try {
      const autostartEnabled = await getAutostart();
      document.getElementById('settings-launch-login').checked = autostartEnabled;
    } catch {
      document.getElementById('settings-launch-login').checked = map.launch_at_login === 'true';
    }
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
  // Hyprland (auto) and Arch toggle (manual): backdrop-filter is disabled
  // (CSS) due to WebKitGTK ghosting. Force a near-opaque alpha so themes
  // still pick the color while the window stays readable without blur.
  if (platform.compositor() === 'hyprland'
      || document.documentElement.hasAttribute('data-disable-blur')) {
    document.documentElement.style.setProperty('--bg-tint', `rgba(${r}, ${g}, ${b}, 0.97)`);
    return;
  }
  const tintA = getSliderVal('ui_tint_opacity');
  const blurA = getSliderVal('ui_blur_opacity') || 0.95;
  const settingsBlur = active ? (getSliderVal('settings_blur_multiplier') || 0.5) : 1.0;
  const a = tintA * blurA * settingsBlur;
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
  if (platform.compositor() === 'hyprland'
      || document.documentElement.hasAttribute('data-disable-blur')) {
    document.documentElement.style.setProperty('--bg-tint', `rgba(${r}, ${g}, ${b}, 0.97)`);
    return;
  }
  const tintA = parseFloat(map.ui_tint_opacity ?? 0.95);
  const blurA = parseFloat(map.ui_blur_opacity ?? 0.95);
  const settingsBlur = parseFloat(map.settings_blur_multiplier ?? 0.5);
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

// --- Background image ---

function applyBackgroundImage(path) {
  if (!path) return;
  // Use Tauri's convertFileSrc instead of hand-rolling asset://: it
  // normalizes path separators (`C:\Users\…` → `C:/Users/…`), URL-encodes
  // correctly per platform, and emits the right protocol scheme
  // (`asset://localhost/` on most platforms, `http://asset.localhost/`
  // on Windows so WebView2 doesn't reject the custom scheme).
  const src = window.__TAURI__.core.convertFileSrc(path);
  document.documentElement.style.setProperty('--bg-image', `url("${src}")`);
}

function clearBackgroundImage() {
  document.documentElement.style.removeProperty('--bg-image');
}

function applyBgLayout(layout) {
  const sizeMap = { center: 'auto', fill: 'cover', stretch: '100% 100%', duplicate: 'auto' };
  const repeatMap = { center: 'no-repeat', fill: 'no-repeat', stretch: 'no-repeat', duplicate: 'repeat' };
  document.documentElement.style.setProperty('--bg-size', sizeMap[layout] || 'cover');
  document.documentElement.style.setProperty('--bg-repeat', repeatMap[layout] || 'no-repeat');
}

function formatValue(key, v) {
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

// --- Dir list helpers (extra scan dirs, skip folders) ---

let configCache = {};

// Escape/unescape commas in CSV config values
function csvEscape(s) { return s.replace(/,/g, '\\,'); }
function csvSplit(raw) {
  if (!raw) return [];
  const parts = [];
  let current = '';
  for (let i = 0; i < raw.length; i++) {
    if (raw[i] === '\\' && raw[i + 1] === ',') {
      current += ',';
      i++;
    } else if (raw[i] === ',') {
      const trimmed = current.trim();
      if (trimmed) parts.push(trimmed);
      current = '';
    } else {
      current += raw[i];
    }
  }
  const trimmed = current.trim();
  if (trimmed) parts.push(trimmed);
  return parts;
}
function csvJoin(arr) { return arr.map(csvEscape).join(','); }

async function addDirToConfig(key, folder) {
  const map = await loadConfigMap();
  const current = csvSplit(map[key] || '');
  if (current.includes(folder)) return;
  current.push(folder);
  const joined = csvJoin(current);
  await saveConfig({ [key]: joined });
  configCache[key] = joined;
}

async function removeDirFromConfig(key, folder) {
  const map = await loadConfigMap();
  const current = csvSplit(map[key] || '');
  const updated = current.filter(d => d !== folder);
  const joined = csvJoin(updated);
  await saveConfig({ [key]: joined });
  configCache[key] = joined;
}

async function renderDriveChips() {
  const container = document.getElementById('settings-detected-drives');
  if (!container) return;
  let drives = [];
  try {
    drives = await listCandidateDrives();
  } catch {
    return;
  }
  container.innerHTML = '';
  const extraRoots = csvSplit(configCache.file_scan_extra_roots || '');
  const has = (root) => extraRoots.some(r => r.toLowerCase() === root.toLowerCase());

  for (const drive of drives) {
    const chip = document.createElement('label');
    chip.className = 'settings-drive-chip';
    const cb = document.createElement('input');
    cb.type = 'checkbox';
    cb.checked = has(drive.root);
    if (cb.checked) chip.classList.add('is-active');
    const text = document.createElement('span');
    text.textContent = drive.root;
    chip.appendChild(cb);
    chip.appendChild(text);

    cb.addEventListener('change', async () => {
      if (cb.checked) {
        await addDirToConfig('file_scan_extra_roots', drive.root);
        chip.classList.add('is-active');
      } else {
        await removeDirFromConfig('file_scan_extra_roots', drive.root);
        chip.classList.remove('is-active');
      }
      renderDirList(document.getElementById('settings-extra-dirs'), 'file_scan_extra_roots');
    });
    container.appendChild(chip);
  }
}

function renderDirList(container, configKey) {
  const val = configCache[configKey] || '';
  const dirs = csvSplit(val);
  container.innerHTML = '';
  for (const dir of dirs) {
    const item = document.createElement('div');
    item.className = 'settings-dir-item';
    const path = document.createElement('span');
    path.className = 'settings-dir-path';
    path.textContent = dir;
    const btn = document.createElement('button');
    btn.className = 'settings-dir-remove';
    btn.textContent = '✕';
    btn.addEventListener('click', async () => {
      await removeDirFromConfig(configKey, dir);
      renderDirList(container, configKey);
    });
    item.appendChild(path);
    item.appendChild(btn);
    container.appendChild(item);
  }
}
