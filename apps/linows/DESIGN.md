# linows — Design Reference

All visual specs derived from the macOS SwiftUI app (source of truth).

---

## Window

| Property | Value |
|----------|-------|
| Size | 620x600 (min) |
| Corner radius | 16px |
| Decorations | None (borderless) |
| Background | Blur material + tint color overlay |
| Position | Centered, always on top |
| Behavior | Auto-hide on focus loss |

---

## Color System

### Architecture

The macOS app uses a **two-tier color system**:

1. **Built-in themes** define explicit RGB values for every semantic color
2. **Custom/user themes** use a luminance-aware dimming formula as fallback

For linows, each theme provides **all** color values explicitly (no runtime derivation needed).
The dimming formula is only used when users customize font color without setting secondary/muted.

### Luminance-Aware Dimming (Fallback Formula)

Used to derive secondary/muted text from a base font color:

```
luminance = (0.2126 * R) + (0.7152 * G) + (0.0722 * B)

If luminance > 0.5 (light text on dark bg):
  dimmed = RGB * factor

If luminance <= 0.5 (dark text on light bg):
  dimmed = RGB + (1.0 - RGB) * (1.0 - factor)
```

### Text Hierarchy Derivation

| Level | Dimming Factor | Opacity Multiplier | Usage |
|-------|---------------|-------------------|-------|
| Primary | 1.0 (unchanged) | 1.0 | Titles, main content |
| Secondary | 0.82 | 1.0 | Subtitles, paths |
| Muted | 0.64 | 0.78 | Metadata, hints, captions |

### Contrast Detection (text-on-color)

For accent/colored backgrounds, determine text color:
```
luminance = (0.2126 * R) + (0.7152 * G) + (0.0722 * B)
text = luminance > 0.62 ? black(0.90) : white(1.0)
```

### Surface Opacity Ranges

Each theme defines explicit RGB for surfaces. Consistent opacity ranges across all themes:

| Surface | Opacity Range | Purpose |
|---------|---------------|---------|
| Panel fill | 0.34–0.38 | Background of side panels, cards |
| Control fill | 0.34 | Buttons, inputs, interactive elements |
| Divider | 0.28–0.32 | Separators between sections |
| Selection | 0.25–0.28 | Highlighted/selected row background |

### CSS Custom Properties Structure

```css
:root {
  /* Background tint (window overlay) */
  --tint-r: 0.08;
  --tint-g: 0.10;
  --tint-b: 0.12;
  --tint-opacity: 0.55;
  --bg-tint: rgba(20, 25, 30, 0.55);

  /* Text hierarchy */
  --font-color: rgba(242, 240, 250, 0.97);
  --font-secondary: rgba(204, 214, 245, 1.0);
  --font-muted: rgba(171, 178, 199, 0.78);

  /* Border */
  --border-color: rgba(255, 255, 255, 0.12);
  --border-thickness: 1px;

  /* Surfaces (explicit per theme, NOT derived) */
  --panel-fill: rgba(30, 28, 46, 0.34);
  --control-fill: rgba(48, 51, 69, 0.34);
  --divider-color: rgba(102, 102, 112, 0.28);
  --selection-fill: rgba(148, 153, 178, 0.28);

  /* Accent & semantic */
  --accent-color: rgba(138, 181, 250, 1.0);
  --on-accent-color: rgba(0, 0, 0, 0.90);
  --color-success: #a6e3a1;
  --color-warning: #fab387;
  --color-danger: #f38ba8;
  --color-info: #89b4fa;
}
```

---

## Themes (Complete Specs)

Each built-in theme defines ALL semantic colors explicitly.

### Catppuccin (Default)

| Token | RGB | Opacity |
|-------|-----|---------|
| Tint | (0.08, 0.10, 0.12) | 0.55 |
| Font (primary) | (0.95, 0.94, 0.98) | 0.97 |
| Font (secondary) | (0.80, 0.84, 0.96) | 1.0 |
| Font (muted) | (0.67, 0.70, 0.78) | 0.78 |
| Panel fill | (0.12, 0.11, 0.18) | 0.34 |
| Control fill | (0.19, 0.20, 0.27) | 0.34 |
| Divider | (0.40, 0.40, 0.44) | 0.28 |
| Selection | (0.58, 0.60, 0.70) | 0.28 |
| Accent | (0.54, 0.71, 0.98) | 1.0 |
| Border | (1.0, 1.0, 1.0) | 0.12 |

### Tokyo Night

| Token | RGB | Opacity |
|-------|-----|---------|
| Tint | (0.10, 0.11, 0.15) | 0.60 |
| Font (primary) | (0.84, 0.87, 0.96) | 0.98 |
| Font (secondary) | (0.74, 0.80, 0.90) | 1.0 |
| Font (muted) | (0.56, 0.64, 0.78) | 0.78 |
| Panel fill | (0.06, 0.09, 0.18) | 0.38 |
| Control fill | (0.14, 0.17, 0.28) | 0.34 |
| Divider | (0.40, 0.40, 0.44) | 0.28 |
| Selection | (0.38, 0.47, 0.72) | 0.28 |
| Accent | (0.52, 0.72, 0.98) | 1.0 |
| Border | (0.66, 0.69, 0.84) | 0.10 |

### Rose Pine

| Token | RGB | Opacity |
|-------|-----|---------|
| Tint | (0.10, 0.09, 0.14) | 0.58 |
| Font (primary) | (0.95, 0.93, 0.91) | 0.98 |
| Font (secondary) | (0.88, 0.84, 0.81) | 1.0 |
| Font (muted) | (0.74, 0.70, 0.66) | 0.78 |
| Panel fill | (0.13, 0.11, 0.17) | 0.38 |
| Control fill | (0.20, 0.18, 0.25) | 0.34 |
| Divider | (0.40, 0.40, 0.44) | 0.28 |
| Selection | (0.58, 0.52, 0.66) | 0.28 |
| Accent | (0.72, 0.61, 0.78) | 1.0 |
| Border | (0.88, 0.87, 0.96) | 0.10 |

### Gruvbox

| Token | RGB | Opacity |
|-------|-----|---------|
| Tint | (0.16, 0.16, 0.16) | 0.60 |
| Font (primary) | (0.93, 0.89, 0.79) | 0.98 |
| Font (secondary) | (0.87, 0.80, 0.64) | 1.0 |
| Font (muted) | (0.72, 0.64, 0.48) | 0.78 |
| Panel fill | (0.14, 0.11, 0.09) | 0.38 |
| Control fill | (0.21, 0.17, 0.13) | 0.34 |
| Divider | (0.40, 0.40, 0.44) | 0.28 |
| Selection | (0.74, 0.54, 0.26) | 0.28 |
| Accent | (0.86, 0.72, 0.40) | 1.0 |
| Border | (0.92, 0.86, 0.70) | 0.10 |

### Dracula

| Token | RGB | Opacity |
|-------|-----|---------|
| Tint | (0.16, 0.16, 0.21) | 0.58 |
| Font (primary) | (0.97, 0.97, 0.98) | 0.98 |
| Font (secondary) | (0.92, 0.87, 0.98) | 1.0 |
| Font (muted) | (0.77, 0.74, 0.85) | 0.78 |
| Panel fill | (0.11, 0.10, 0.18) | 0.38 |
| Control fill | (0.21, 0.20, 0.30) | 0.34 |
| Divider | (0.40, 0.40, 0.44) | 0.28 |
| Selection | (0.62, 0.52, 0.79) | 0.28 |
| Accent | (0.64, 0.75, 0.98) | 1.0 |
| Border | (0.97, 0.97, 0.95) | 0.10 |

### Kanagawa

| Token | RGB | Opacity |
|-------|-----|---------|
| Tint | (0.09, 0.09, 0.11) | 0.60 |
| Font (primary) | (0.87, 0.86, 0.79) | 0.98 |
| Font (secondary) | (0.80, 0.78, 0.66) | 1.0 |
| Font (muted) | (0.66, 0.63, 0.50) | 0.78 |
| Panel fill | (0.10, 0.12, 0.14) | 0.38 |
| Control fill | (0.18, 0.20, 0.24) | 0.34 |
| Divider | (0.40, 0.40, 0.44) | 0.28 |
| Selection | (0.50, 0.48, 0.38) | 0.28 |
| Accent | (0.46, 0.65, 0.82) | 1.0 |
| Border | (0.86, 0.84, 0.73) | 0.10 |

---

## Color Application in Views

### Result Row
- Title text: `--font-color`
- Path/subtitle: `--font-muted`
- Kind badge text: `--font-secondary`
- Selected row background: `--selection-fill`
- Selected row border: `--divider-color` (1px)
- Row divider: `--divider-color` at 0.8 opacity

### Preview Panel
- Title: `--font-color`, semibold
- Metadata labels: `--font-muted`
- Metadata values: `--font-secondary`
- Panel background: `--panel-fill`

### Search Input
- Text: `--font-color`
- Placeholder: `--font-muted`
- Background: `--control-fill`
- Border: `--border-color`
- Icon (magnifying glass): `--font-muted`

### Settings Panel
- Tab active: `--accent-color`
- Tab inactive: `--font-muted`
- Slider track: `--divider-color`
- Slider thumb: `--accent-color`
- Section headers: `--font-secondary`

### Hint Bar
- Text: `--font-muted`
- Background: transparent

### Banner Notifications
- Background: `--control-fill`
- Text: `--font-color`
- Border: `--border-color`

---

## Typography

| Property | Value |
|----------|-------|
| Font family | -apple-system, "Segoe UI", system-ui, sans-serif |
| Base size | 14px (configurable via uiScale 0.7–1.8) |
| Title | base + 2px, semibold |
| Subtitle | base + 1px, medium |
| Normal | base, regular |
| Label | base - 1px, regular |
| Caption | base - 2px, regular |
| Monospace | "SF Mono", "Cascadia Code", "JetBrains Mono", monospace |

---

## Spacing

| Token | Value |
|-------|-------|
| Content padding | 14px |
| Result row spacing | 4px |
| Row height | 38px |
| Icon size | 22px (results), 48px (preview) |
| Input padding | 12px horizontal, 10px vertical |
| Section gap | 12px |
| Corner radius (controls) | 8px |

---

## Blur Materials

| Name | Description | Opacity scale |
|------|-------------|---------------|
| hudWindow | Darkest, most opaque | 1.12x |
| sidebar | Soft translucency | 0.92x |
| menu | Balanced default | 1.00x |
| underWindowBackground | Subtlest | 0.72x |

On Windows: map to Mica (hudWindow) or Acrylic (others) via Tauri WindowEffectsConfig.
On Linux: use solid `--bg-tint` as fallback when compositor doesn't support blur.

---

## Platform Visual Parity

macOS (SwiftUI) is the design source of truth. This table documents what each platform can and cannot achieve.

**Legend:** ✅ native support, ⚠️ partial/workaround, ❌ not possible

| Feature | macOS | Windows | Linux (GNOME/KDE) | Linux (i3/sway/X11 bare) |
|---------|-------|---------|-------------------|--------------------------|
| Window blur | ✅ NSVisualEffectView | ✅ Mica (Win11) / Acrylic (Win10) | ⚠️ KDE supports blur; GNOME no native API | ❌ No compositor blur |
| Transparency | ✅ Native | ✅ DWM composition | ⚠️ Wayland compositors support it; X11 needs picom/compton | ❌ Solid background fallback |
| Rounded corners | ✅ Native window mask | ✅ DWM auto-rounds (Win11) | ⚠️ CSS border-radius works but no window-level mask | ⚠️ CSS border-radius, visible square edges underneath |
| Window shadow | ✅ Native macOS shadow | ✅ DWM shadow | ⚠️ Compositor-dependent | ❌ No shadow |
| Font: SF Pro | ✅ System font | ❌ Segoe UI fallback | ❌ System sans-serif fallback | ❌ System sans-serif fallback |
| Font: SF Mono | ✅ System font | ❌ Cascadia Code fallback | ❌ JetBrains Mono / monospace fallback | ❌ monospace fallback |
| Icon quality | ✅ NSImage, crisp at all DPIs | ✅ SHGetFileInfo, DPI-aware | ⚠️ freedesktop icon theme, quality varies | ⚠️ Same as GNOME/KDE |
| Scrollbars | ✅ Native thin overlay | ⚠️ Can be styled with CSS | ⚠️ WebKitGTK scrollbars, CSS styling | ⚠️ Same |
| Animations | ✅ SwiftUI spring/easeOut | ⚠️ CSS transitions (simpler) | ⚠️ CSS transitions | ⚠️ CSS transitions |
| Vibrancy materials | ✅ hudWindow, sidebar, menu | ⚠️ Mica/Acrylic (fewer options) | ❌ Faked with rgba tint | ❌ Solid dark background |
| Native file dialog | ✅ NSOpenPanel | ✅ Win32 dialog | ✅ xdg-desktop-portal | ⚠️ Needs portal service running |
| Audio playback | ✅ AVFoundation | ✅ WASAPI via rodio | ✅ ALSA via rodio (PulseAudio/PipeWire compat) | ✅ Same |
| Global hotkey | ✅ NSEvent | ✅ RegisterHotKey | ✅ X11 grab | ⚠️ Works on X11; Wayland limited |
| Tray icon | ✅ NSStatusBar | ✅ Shell_NotifyIcon | ✅ libappindicator | ⚠️ Depends on tray support |

### Unsolvable Gaps

These are platform limitations that cannot be fixed in app code:

1. **No blur on i3/sway/X11 bare** — There is no compositor blur API. The app uses a solid dark `--bg-tint` background instead. This is the expected look on minimal WMs.

2. **No SF Pro / SF Mono on non-macOS** — Apple's fonts are not redistributable. Each platform uses its best system font. The visual weight and spacing will differ slightly.

3. **No true vibrancy on Linux** — macOS vibrancy shows desktop content through the window with a tinted blur. Linux has no equivalent API. GNOME/KDE compositors may support basic transparency but not the material effect.

4. **Rounded window mask on X11** — CSS `border-radius` rounds the content, but the actual window shape remains rectangular on X11. A thin gap may be visible at corners. Wayland compositors handle this better.

### Recommendations per Platform

- **Windows (Win11):** Enable `"effects": ["mica"]` in tauri.conf.json for near-macOS appearance.
- **Windows (Win10):** Use `"effects": ["acrylic"]` — less refined but still translucent.
- **Linux (GNOME on Wayland):** Enable transparency in tauri.conf.json. No blur but transparent tint works.
- **Linux (KDE):** KDE compositor supports blur hints — may work with additional config.
- **Linux (i3/sway):** Solid background mode. Add picom for X11 transparency. No blur possible.

---

## Screens

### 1. Search (Default)
```
+------------------------------------------+
| [icon] Search...                         |
+------------------------------------------+
| [icon] Result Title         [kind badge] |
|        /path/to/item                     |
| [icon] Result Title         [kind badge] |
|        /path/to/item                     |
| ...                                      |
+------------------------------------------+
| hint text                                |
+------------------------------------------+
```

### 2. Search + Preview (with selection)
```
+------------------------------------------+
| [icon] Search...                         |
+------------------------------------------+
| Results List    |  Preview Panel         |
|                 |  [48px icon]           |
| > Selected      |  Title                 |
|   Item 2        |  Kind: App             |
|   Item 3        |  Size: 12.3 MB         |
|   ...           |  Modified: 2024-01-15  |
|                 |  /full/path/to/item    |
+------------------------------------------+
| hint text                                |
+------------------------------------------+
```

### 3. Command Mode (Ctrl+/)
```
+------------------------------------------+
| [calc] [shell] [kill] [sys]              |
+------------------------------------------+
| Command input...                         |
+------------------------------------------+
| Output area                              |
|                                          |
|                                          |
+------------------------------------------+
```

### 4. Clipboard History (c")
```
+------------------------------------------+
| [icon] c"query...                        |
+------------------------------------------+
| [clip] First line of text...    2m ago   |
|        38 chars, 1 line                  |
| [clip] Another entry...        1h ago   |
|        124 chars, 3 lines                |
+------------------------------------------+
```

### 5. Settings (Ctrl+Shift+,)
```
+------------------------------------------+
| [Appearance] [Shortcuts] [Advanced]      |
+------------------------------------------+
| Theme:     [Catppuccin v]                |
| Tint:      [color picker]               |
| Font:      [slider 12-18]               |
| Blur:      [balanced v]                 |
| Background: [Choose Image]              |
+------------------------------------------+
```

### 6. Help (Ctrl+H)
```
+------------------------------------------+
| Keyboard Shortcuts                       |
+------------------------------------------+
| Enter        Open selected item          |
| Ctrl+F       Reveal in file manager      |
| Ctrl+C       Copy path                   |
| Ctrl+/       Command mode                |
| Ctrl+P       Pick/unpick item            |
| Ctrl+H       This help screen            |
| Escape       Hide window                 |
| Alt+Space    Toggle window               |
+------------------------------------------+
```

### 7. Translation (t")
```
+------------------------------------------+
| [icon] t"hello world                     |
+------------------------------------------+
| Source: hello world                      |
|                                          |
| [EN] [VI] [JP]                           |
|                                          |
| Translation result here                  |
|                                          |
| [Copy] [Open in Browser]                |
+------------------------------------------+
```

---

## Animations

| Element | Type | Duration | Easing |
|---------|------|----------|--------|
| Selection scroll | Transform | 120ms | ease-out |
| Banner show/hide | Translate + opacity | 200ms | ease-in-out |
| Panel transitions | Opacity | 150ms | ease |
| Focus recovery | Delayed (0, 40, 100ms) | — | — |
