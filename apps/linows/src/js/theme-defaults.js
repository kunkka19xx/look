/**
 * Every linows theme default. Colours belong to a preset, the rest to
 * THEME_DEFAULTS, and `defaultFor` is how the app asks for either.
 */

export const BLUR_PRESETS = {
    high_contrast: { ui_blur_opacity: 0.95 },
    balanced: { ui_blur_opacity: 0.8 },
    soft: { ui_blur_opacity: 0.6 },
};

const DEFAULT_BLUR_STYLE = 'high_contrast';

export const DEFAULT_THEME_ID = 'kanagawa';

const preset = (
    [tintR, tintG, tintB],
    [fontR, fontG, fontB],
    [bordR, bordG, bordB],
    opacities,
) => ({
    ui_tint_red: tintR,
    ui_tint_green: tintG,
    ui_tint_blue: tintB,
    ui_font_red: fontR,
    ui_font_green: fontG,
    ui_font_blue: fontB,
    ui_border_red: bordR,
    ui_border_green: bordG,
    ui_border_blue: bordB,
    ...opacities,
});

// Opacities are the user's (USER_CONTROLLED_KEYS), so only the two presets
// whose transparency is their whole look declare them.
export const THEME_PRESETS = {
    catppuccin: preset([0.12, 0.12, 0.18], [0.81, 0.8, 0.9], [0.58, 0.58, 0.65]),
    'tokyo-night': preset([0.1, 0.11, 0.15], [0.84, 0.87, 0.96], [0.66, 0.69, 0.84]),
    'rose-pine': preset([0.1, 0.09, 0.14], [0.95, 0.93, 0.91], [0.88, 0.87, 0.96]),
    gruvbox: preset([0.16, 0.16, 0.16], [0.93, 0.89, 0.79], [0.92, 0.86, 0.7]),
    dracula: preset([0.16, 0.16, 0.21], [0.97, 0.97, 0.98], [0.97, 0.97, 0.95]),
    // Font runs warmer than Kanagawa's fujiWhite, towards carpYellow.
    kanagawa: preset([0.09, 0.09, 0.11], [0.94, 0.85, 0.57], [0.86, 0.84, 0.73]),
    kindle: preset([0.97, 0.95, 0.9], [0.13, 0.12, 0.1], [0.42, 0.38, 0.32], {
        ui_tint_opacity: 0.93,
        ui_font_opacity: 1.0,
        ui_border_opacity: 0.26,
    }),
    liquid: preset([0.1, 0.13, 0.2], [0.98, 0.98, 1.0], [1.0, 1.0, 1.0], {
        ui_tint_opacity: 0.78,
        ui_font_opacity: 1.0,
        ui_border_opacity: 0.1,
    }),
};

export const THEME_SURFACES = { liquid: 'liquid' };

export const THEME_DEFAULTS = {
    ui_theme: DEFAULT_THEME_ID,
    ui_surface: THEME_SURFACES[DEFAULT_THEME_ID] ?? '',
    ui_tint_opacity: 0.96,
    ui_font_opacity: 0.96,
    ui_border_opacity: 0.5,
    ui_border_thickness: 2,
    ui_surface_radius: 1.5,
    // "Let the theme decide", not a family.
    ui_font_name: 'system-ui',
    ui_font_size: 14,
    ui_blur_style: DEFAULT_BLUR_STYLE,
    ui_blur_opacity: BLUR_PRESETS[DEFAULT_BLUR_STYLE].ui_blur_opacity,
    settings_blur_multiplier: 1,
    ui_bg_image: '',
    ui_bg_layout: 'fill',
    ui_bg_opacity: 0.62,
    ui_bg_blur: 10.3,
    inner_gap: 7,
};

// Kept across theme switches, so a custom transparency survives trying a palette.
export const USER_CONTROLLED_KEYS = new Set([
    'ui_tint_opacity',
    'ui_font_opacity',
    'ui_border_opacity',
    'ui_border_thickness',
    'ui_surface_radius',
]);

// Switching to one takes its opacities back and persists them.
export const OPACITY_OWNING_THEMES = new Set(['kindle', 'liquid']);
export const OPACITY_SWITCH_KEYS = ['ui_tint_opacity', 'ui_font_opacity', 'ui_border_opacity'];

export function defaultFor(key) {
    return THEME_DEFAULTS[key] ?? THEME_PRESETS[DEFAULT_THEME_ID][key];
}

// Absent and empty both mean "never chosen"; "custom" is a real answer.
export function resolveThemeId(map) {
    return isSet(map?.ui_theme) ? map.ui_theme : DEFAULT_THEME_ID;
}

// `key=` in the file means unset, not "".
export function isSet(v) {
    return v !== undefined && v !== '';
}

export function valueOr(map, key) {
    return isSet(map?.[key]) ? map[key] : defaultFor(key);
}
