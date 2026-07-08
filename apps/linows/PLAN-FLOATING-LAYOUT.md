# Floating "inner-gap" layout — linows implementation handoff

This spec ports a launcher UI redesign already shipped on **macOS**
(`apps/macos/LauncherApp/look-app/`) to the **linows** Tauri build
(`apps/linows/src/` — vanilla HTML/CSS/JS). It describes the *behavior and visual
target*, the exact numbers, and — most importantly — the non-obvious
correctness/perf traps we already hit and fixed on macOS. Read the "Gotchas"
section before writing code; two of those bugs are easy to reproduce and painful
to diagnose.

The macOS source of truth is `Views/Launcher/LauncherView.swift` (plus
`LauncherSubviews.swift`, `LauncherView+Background.swift`, `AppConstants.swift`,
`Models/ThemeSettings.swift`, `Support/ThemeStore.swift`,
`Views/Settings/ThemeSettingsView+Appearance.swift`).

---

## 1. What we're building

A user setting **Inner Gap** (0–24 px, default **0**) turns the launcher's home
screen from one framed panel into **i3-style floating tiles**:

- At **gap = 0** → the current classic look is preserved *exactly* (framed panel,
  hairline divider between list and preview, full-width hint bar). Do not
  regress this path.
- At **gap > 0** → the window backdrop box is dropped and the home screen becomes
  **3 floating frosted tiles** on the bare desktop:
  1. **Top bar** = search field **+** running-apps strip, merged into one tile.
  2. **Left column** = results list.
  3. **Right column** = preview / picked panel.
  Separated by a uniform gap equal to the setting.

Plus several always-on refinements (independent of the gap): a unified top bar, a
shorter top bar, empty-query "just the bar" resting state, and hint-bar tweaks.

---

## 2. New setting: `inner_gap`

- **Config key:** `inner_gap` in `.look.config` (shared with macOS — same file,
  same key name). Integer points, clamp **0–24**, default **0**.
- **linows wiring:**
  - Add to the settings screen (`js/screens/settings.js`, `html/screens/settings.html`)
    as a slider, next to the existing running-apps toggle / scan-depth inputs.
    Persist with `saveConfig({ inner_gap: value })` (same pattern as
    `file_scan_depth`, `running_apps_placement`).
  - On config load/reload (`js/app.js` `setOnConfigReload`, and the startup read
    around `js/app.js:210`), read `inner_gap` and expose it to the layout — the
    cleanest lever is a CSS variable `--inner-gap` on `.launcher-window` and a
    boolean class (see §4).
- macOS reference: `ThemeSettings.innerGap` (default 0); parsed/saved in
  `ThemeStore.swift` (`case "inner_gap"` clamps 0–24; `upsertConfigLine(... "inner_gap" ...)`);
  slider in `ThemeSettingsView+Appearance.swift` ("Layout" section,
  `LabeledSlider(range: 0...24)`).

---

## 3. Target layout (DOM / grid restructure)

Today linows uses a 2×2 grid in `.main-pane` (`css/layout.css`):
search-bar (top-left) · running-apps (top-right) / results (bottom-left) ·
preview (bottom-right).

The redesign needs **row 1 to be a single full-width bar** containing *both* the
search field and the running-apps strip, then **row 2** as the two columns.
Recommended structure (keep element IDs stable so `search.html` handlers/JS keep
working — see the focus gotcha in §6):

```
.main-pane
  .top-bar            (row 1, spans full width)   <-- search-bar + running-apps
    .search-bar #search-bar   ( #query input )
    .running-apps-strip #running-apps-strip
  .results-row         (row 2)
    .results-area #results-area   ( .ai-card + .results-list )
    .preview-panel #preview-panel
  .hint-bar                    (only in classic / single-panel states — see §5)
```

- **Gap** between the 3 tiles = `--inner-gap`. Apply it as the grid `gap`
  (row gap between `.top-bar` and `.results-row`, column gap between
  `.results-area` and `.preview-panel`). Inside `.top-bar`, search↔running-apps
  keep a **fixed 10px** gap (not the inner gap).
- At gap 0, `--inner-gap: 0` and the classic divider returns (see §7).

---

## 4. Visual spec — the frosted tile

Each floating tile (top bar, left column, right column, and the single-panel and
AI cards) shares one "card" treatment. macOS `paneCard` values → CSS:

| Property        | Value                                            |
|-----------------|--------------------------------------------------|
| corner radius   | **12px** (tiles) / 10px (classic search fill)    |
| fill            | `var(--control-fill)` **over** `rgba(0,0,0,0.30)` |
| backdrop blur   | `backdrop-filter: blur(var(--blur-radius))` clipped to the radius |
| border          | `1px solid rgba(255,255,255,0.10)`               |
| shadow          | `0 3px 7px rgba(0,0,0,0.25)`                      |
| inner padding   | **6px** (columns / single-panels) · **0** (top bar) |

The blur + `rgba(0,0,0,0.30)` fill is what keeps tiles legible **without** the
window backdrop. Do not skip the dark fill — a pure translucent tile is unreadable
on a busy desktop (we hit this).

**Classic (gap 0):** no per-tile card. The panel keeps `--bg-tint` backdrop, the
1px border, and the hairline divider. This is the `barFloatsFree = false` branch.

**Background image when floating — crop it into each tile (do NOT fill the gaps).**
A full-window image behind the tiles reads as one panel again, which defeats the
separated-floating look. Instead: when floating with an image set, each tile shows
its **own aligned slice** of the image — the image is conceptually sized to the
whole panel and each tile reveals the region at its window position, so adjacent
tiles look like one continuous image cut apart by the (transparent, desktop-
showing) gaps. On top of each slice sits the dark scrim + tint for legibility;
with no image the tile falls back to a blurred-desktop frost.

- macOS: `tileBackground()` renders `croppedBackgroundImage()` = the panel-sized
  image `.offset(-tileOrigin)` inside a per-tile `GeometryReader`, using a named
  panel coordinate space (`panelCoordinateSpace`) and a captured `panelSize`.
- linows: give each tile `background-image` of the full image with
  `background-size: <panelW> <panelH>` and `background-position: -<tileX>px -<tileY>px`
  (the tile's offset within `.main-pane`). That yields the same aligned-slice
  effect natively — no masking/canvas needed. Gaps get no background (transparent).
  Recompute tile offsets on resize.

---

## 5. The state model (translate this carefully)

All layout decisions on macOS route through a few **computed booleans**. Port
them as small pure functions of current state; drive CSS via classes on
`.launcher-window` / `.main-pane`. Names below match the macOS source.

- **`usesPanes`** = `inner_gap > 0`.
- **`showsFloatingCards`** (the floating gate) =
  `usesPanes && !commandMode && !settingsOpen && !helpOpen`.
  **KEEP THIS CHEAP AND STABLE** — see Gotcha A. It must **not** depend on the
  query text, result count, translation/clipboard state, or AI state.
- **`showsResultsGrid` / `showsFloatingGrid`** = we're showing the **two-card grid**
  (normal results OR clipboard-empty), not a single panel:
  `showsFloatingCards && !translationQuery && !(recentQuery && resultsEmpty)`.
  Used only to decide **where the hint bar lives** (may read live state — it only
  toggles text, never a blur).
- **`hidesResultsForEmptyQuery`** = `queryEmpty && !commandMode && !settingsOpen && !helpOpen`.
  Empty-query resting state (§6).
- **`barFloatsFree`** = `showsFloatingCards || hidesResultsForEmptyQuery`.
  "No backdrop box; tiles float on desktop." Drives: hide window backdrop/border,
  and make the **top bar** a frosted tile.

Hint-bar placement rule:
- `showsFloatingGrid` → hints in the **left card footer**, copyright in the
  **right card footer**.
- otherwise (classic, translation, recent-empty, single AI card) → the classic
  **full-width hint bar** at the bottom.
- `hidesResultsForEmptyQuery` → **no** hint bar or copyright at all.

Per-screen behavior when floating (`gap > 0`):

| Screen (query)                     | Layout when floating                                    |
|------------------------------------|---------------------------------------------------------|
| empty                              | **top bar only** (hide everything below) — §6           |
| app/file results                   | 3 tiles: top bar + list card + preview card             |
| clipboard results (`c"x`)          | same two-card grid                                      |
| clipboard empty (`c"`)             | two-card grid: **left = History info, right = How-to** — §5a |
| recent empty (`rc"`)               | single floating card (genuinely one column) + bottom hint |
| translation (`t"…`, `tw"…`)        | single floating card + bottom hint                      |
| AI answer only                     | one card (answer) + in-card hints                       |
| AI knowledge lookup                | two-card grid: **left = answer, right = suggestion list** |
| AI answer + local results          | answer card capped on top, results grid below           |

### 5a. Clipboard/AI must reuse the SAME two-card grid
The clipboard-empty "History | How-to" screen and the AI knowledge-lookup
"answer | suggestions" screen must render through the **same** two-card grid
component as normal results — same gap, same in-card hint/copyright footers — so
they look identical to the app list. On macOS this is the shared `twoPaneGrid`
helper. Do **not** special-case them into a single wide card with a full-width
hint bar (that inconsistency was explicitly rejected).

---

## 6. Empty-query resting state

When the query is empty on the home screen (`hidesResultsForEmptyQuery`), show
**only the top bar** — hide the results columns, the hint bar, the copyright, the
window backdrop, and the border. This applies in **both** modes (gap 0 and gap>0).

- The lone top bar must use the **frosted tile** treatment even at gap 0
  (that's why `barFloatsFree` includes this state) — otherwise the classic
  translucent search fill is nearly invisible on the desktop.
- The window stays its normal fixed size; the area below the bar is transparent
  (desktop shows through). Optional future step: shrink the window to the bar.

---

## 7. Gotchas — read before coding

### A. The floating gate MUST be stable (perf / freeze) — highest priority
On macOS, the gate first (wrongly) depended on `translationQuery`,
`resultsEmpty`, and `aiAnswer.isActive`. Because those flip **while you type**,
the layout toggled floating↔classic on nearly every keystroke, which
created/destroyed the blur surfaces (`NSVisualEffectView`) every time and
**froze typing** (`c"`, `t"`, and AI/web search all beach-balled).

Fix: the gate depends only on coarse mode (`usesPanes`, command/settings/help).
The **content** inside the tiles changes as you type; the **tile frame does not**.

linows analog: toggling `backdrop-filter`/blur containers in/out of the DOM per
keystroke is just as expensive in WebKitGTK (note the existing hyprland/blur
workarounds in `layout.css`). **Toggle a class** (`.floating`) on a **persistent**
`.launcher-window`; never rebuild the tile DOM per keystroke. Only swap the inner
list/preview content.

### B. Don't run a search / AI for prefix screens
Translation/clipboard/recent/prefix/command queries render their own panels and
must **not** trigger the backend file search or the AI answer — otherwise a
background AI activation flips the gate and flashes the old UI (this was the `t"`
"backs to old ui while typing" bug). On macOS the `onChange(of: query)` handler
short-circuits: `if isClipboardQuery || isPrefixSuggestionQuery ||
isCommandSuggestionQuery || isTranslationQuery { cancelAI(); reseedSelection() }
else { search() }`. Mirror this in `js/search.js` / `js/app.js`: translation only
resolves on Enter; don't fire search/AI on each keystroke for those prefixes.

### C. Preserve input focus across the classic↔floating flip (view identity)
At gap 0, typing the first char flips `barFloatsFree` (empty-rest → results),
which changed the top bar's DOM subtree and **recreated the `#query` input →
focus lost**. Fix on macOS: apply the bar's chrome as **one stable modifier**
whose *values* change, never an `if/else` that swaps the input's subtree.

linows analog (easier in the DOM, but don't regress it): keep the **same**
`#query` element and `.search-bar`/`.top-bar` nodes mounted at all times; switch
looks by **toggling classes** (`.floating`, `.resting`) — never `innerHTML`-replace
or remove/re-add the search bar when the query goes empty↔non-empty. If you
rebuild it, the input blurs.

### D. Uniform gaps, not additive
The vertical gap (top bar → columns) must **equal** the horizontal gap
(column → column) = `inner_gap`. Original macOS bug: it was `12 + inner_gap`
(too large). Use one value everywhere; search↔running-apps inside the top bar is
the only fixed exception (10px).

---

## 8. Exact values (macOS → CSS)

| Thing                              | Value |
|------------------------------------|-------|
| `inner_gap` range / default / step | 0–24 / **0** / 1 |
| tile corner radius                 | 12px |
| classic search-fill radius         | 10px |
| tile fill                          | `var(--control-fill)` over `rgba(0,0,0,0.30)` |
| tile border                        | `1px solid rgba(255,255,255,0.10)` |
| tile shadow                        | `0 3px 7px rgba(0,0,0,0.25)` |
| tile inner padding                 | 6px columns / 0 top bar |
| gap (all three)                    | `inner_gap` |
| search ↔ running-apps gap          | 10px (fixed) |
| classic divider                    | 1px `rgba(255,255,255,0.08)`, 4px vertical inset |
| running-app icon size              | **30px** (was 34) |
| running-apps edge slack            | **6px** (was 8) |
| search input padding               | 12px x / **8px** y (was 10) |
| home hint items                    | `["Enter open", "Cmd+H help"]` (removed "Cmd+F reveal") |
| clipboard hint items               | `["Enter copy clip", "Delete remove clip"]` (trimmed from 4) |

Top-bar height reduction (icon 34→30, slack 8→6, input y-pad 10→8) is an
always-on change; apply it to `--icon-size`/running-apps CSS and `--input-padding-y`
regardless of gap.

---

## 9. Hint bar content changes (always-on)
- Remove **"Cmd+F reveal"** from the home-screen hint (`js/…` hint builder;
  macOS: `hintItems` home case). Cmd+F still works and still appears in
  Settings → Shortcuts / help.
- Clipboard screen hint → only the first two: **"Enter copy clip" ·
  "Delete remove clip"** (drop "Cmd+H help" and "Cmd+/ command mode") so it fits
  one line in the left card footer.

---

## 10. macOS → linows file map

| Concern                    | macOS                                             | linows |
|----------------------------|---------------------------------------------------|--------|
| setting model + persist    | `Models/ThemeSettings.swift`, `Support/ThemeStore.swift` | `.look.config` via `js/screens/settings.js` `saveConfig`, read in `js/app.js` |
| settings slider UI         | `ThemeSettingsView+Appearance.swift`              | `html/screens/settings.html` + `js/screens/settings.js` |
| panel / tiles / gaps       | `LauncherView.swift` (`paneCard`, `twoPaneGrid`, `topRowBar`, `borderedPanel`) | `css/layout.css` (`.launcher-window`, `.main-pane`, new `.top-bar`/`.results-row`) + `html/screens/search.html` |
| backdrop / blur            | `LauncherView+Background.swift` (`themedBackground`) | `.launcher-window` `--bg-tint` + `backdrop-filter` (see hyprland/blur guards) |
| running-apps sizing        | `AppConstants.swift` `RunningAppsStrip`           | `css/components/running-apps.css`, `--icon-size` |
| hint bar                   | `LauncherSubviews.swift` `HintBar`, `hintItems`   | `css/layout.css` `.hint-bar`, hint builder in `js/` |
| clipboard empty split      | `LauncherSubviews.swift` `ClipboardEmpty{Info,Help}View` | `js/components/results.js` / clipboard empty markup |
| AI answer layouts          | `resultsRow` / `aiKnowledgeLookupRow` etc.        | `css/layout.css` `.ai-mode-*`, `js/components/ai-answer*.js` |
| gate / state toggles       | computed vars in `LauncherView.swift`             | class toggles in `js/app.js` |
| focus stability            | `TopBarChrome` modifier                           | keep `#query` mounted; class-toggle only |

---

## 11. Test checklist (do all at gap 0 AND gap 8)

1. Empty query → **just the top bar** (frosted), no box, no columns, no hint bar.
2. Type a char → results appear **and the cursor stays in the search box**
   (focus not lost). Delete back to empty → bar-only again.
3. `c"` (empty clipboard) → **two cards** (History | How-to), in-card hints,
   matching the app-list look. No freeze while typing.
4. `t"hello` / `tw"hello` → single floating card, **no flicker/old-UI flash**,
   no per-keystroke search; result only after Enter.
5. AI/web query (a person's name) → answer + suggestions as a two-card grid;
   stays floating and **smooth** as the answer streams in (no freeze/flash).
6. Running apps visible → search + icons read as **one** bar (shared background,
   no inner search box), a bit shorter than before.
7. Gap 0 everywhere → classic look byte-for-byte (framed panel, hairline divider,
   full-width hint bar).
8. Vertical gap (bar→columns) **equals** the column gap.
9. Set a **background image** + gap 8 → each tile shows its **aligned slice** of
   the image (continuous across tiles), and the **gaps are transparent** (desktop,
   not image). Tiles read as clearly separated.
