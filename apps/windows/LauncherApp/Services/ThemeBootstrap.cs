using System;
using System.Collections.Generic;
using System.Globalization;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;
using Windows.UI;

namespace LauncherApp.Services;

// Reads the persisted ui_* theme keys from ~/.look.config and applies them to
// Application.Resources BEFORE the main window is constructed. Without this, the app boots
// from XAML defaults every time and user-saved tint / font / border / blur-opacity values
// are only visible in the Appearance settings session - they disappear on next launch.
public static class ThemeBootstrap
{
    public static void ApplyFromConfig()
    {
        if (Application.Current?.Resources is not ResourceDictionary resources)
        {
            return;
        }

        Dictionary<string, string> cfg;
        try
        {
            cfg = LookConfig.Read();
        }
        catch
        {
            return;
        }

        // Tint (panel background)
        double tintRp = GetPercent(cfg, "ui_tint_red", 8);
        double tintGp = GetPercent(cfg, "ui_tint_green", 10);
        double tintBp = GetPercent(cfg, "ui_tint_blue", 12);
        double tintAp = GetPercent(cfg, "ui_tint_opacity", 55);

        Color panel = ToColor(tintRp, tintGp, tintBp, tintAp);
        double blurOpacityPercent = GetPercent(cfg, "ui_blur_opacity", 42);
        Color panelAlt = ToColor(tintRp + 8, tintGp + 8, tintBp + 8, blurOpacityPercent);

        // Text
        double textRp = GetPercent(cfg, "ui_font_red", 96);
        double textGp = GetPercent(cfg, "ui_font_green", 96);
        double textBp = GetPercent(cfg, "ui_font_blue", 98);
        double textAp = GetPercent(cfg, "ui_font_opacity", 96);
        Color text = ToColor(textRp, textGp, textBp, textAp);
        // Secondary and muted: when the user picked a built-in theme (Tokyo Night,
        // Dracula, etc.), AppearanceSettings persists ui_text_secondary_* / ui_text_muted_*
        // tokens that carry each theme's signature tint (e.g. Tokyo Night's #8FA3C7 muted
        // blue-grey). When absent (custom themes / pre-existing configs), fall back to
        // luminance-aware dimming. Mirrors macOS ThemeStore.mutedTextColor() which checks
        // `activeAppearanceStyle()?.textMuted` before falling back to dimmableColor().
        // Alpha multipliers: macOS gets visual "lift" for free from NSVisualEffectView
        // translucency, so 0.78/0.90 reads fine there. On Windows's opaque panel, the dark
        // tint bleeds through and washes the theme's muted RGB toward grey - bumping the
        // multipliers lets each theme's color signature carry through composition.
        Color secondary = TryGetRgb(cfg, "ui_text_secondary", out double sR, out double sG, out double sB)
            ? ToColor(sR, sG, sB, textAp * 0.94)
            : DimmableColor(textRp, textGp, textBp, 0.82, textAp * 0.94);
        Color muted = TryGetRgb(cfg, "ui_text_muted", out double mR, out double mG, out double mB)
            ? ToColor(mR, mG, mB, textAp * 0.88)
            : DimmableColor(textRp, textGp, textBp, 0.64, textAp * 0.88);

        // Border
        double borderRp = GetPercent(cfg, "ui_border_red", 100);
        double borderGp = GetPercent(cfg, "ui_border_green", 100);
        double borderBp = GetPercent(cfg, "ui_border_blue", 100);
        double borderAp = GetPercent(cfg, "ui_border_opacity", 12);
        Color border = ToColor(borderRp, borderGp, borderBp, borderAp);
        double borderThickness = GetDouble(cfg, "ui_border_thickness", 1.0);

        Color accent = ToColor(tintRp + 40, tintGp + 45, tintBp + 65, 100);

        UpdateColor(resources, "LauncherColorPanel", panel);
        UpdateColor(resources, "LauncherColorPanelAlt", panelAlt);
        UpdateColor(resources, "LauncherColorText", text);
        UpdateColor(resources, "LauncherColorSecondary", secondary);
        UpdateColor(resources, "LauncherColorMuted", muted);
        UpdateColor(resources, "LauncherColorBorder", border);
        UpdateColor(resources, "LauncherColorAccent", accent);

        UpdateBrush(resources, "LauncherPanelBrush", panel);
        UpdateBrush(resources, "LauncherPanelAltBrush", panelAlt);
        UpdateBrush(resources, "LauncherTextBrush", text);
        UpdateBrush(resources, "LauncherSecondaryTextBrush", secondary);
        UpdateBrush(resources, "LauncherMutedTextBrush", muted);
        UpdateBrush(resources, "LauncherBorderBrush", border);
        UpdateBrush(resources, "LauncherAccentBrush", accent);

        resources["LauncherBorderThickness"] = new Thickness(borderThickness);

        // Typography
        string fontName = cfg.GetValueOrDefault("ui_font_name", string.Empty).Trim();
        if (!string.IsNullOrWhiteSpace(fontName))
        {
            var family = new FontFamily(fontName);
            resources["ContentControlThemeFontFamily"] = family;
            resources["TextControlThemeFontFamily"] = family;
        }

        double fontSize = GetDouble(cfg, "ui_font_size", 14);
        resources["ContentControlThemeFontSize"] = fontSize;
    }

    private static double GetPercent(Dictionary<string, string> cfg, string key, double fallback)
    {
        // Config stores fractions (0..1); sliders use percent (0..100).
        if (cfg.TryGetValue(key, out string? raw)
            && double.TryParse(raw, NumberStyles.Float, CultureInfo.InvariantCulture, out double fraction))
        {
            return Math.Clamp(fraction * 100d, 0, 100);
        }
        return fallback;
    }

    private static bool TryGetRgb(Dictionary<string, string> cfg, string keyPrefix, out double rPct, out double gPct, out double bPct)
    {
        rPct = 0;
        gPct = 0;
        bPct = 0;
        if (!cfg.TryGetValue(keyPrefix + "_red", out string? rRaw) || string.IsNullOrWhiteSpace(rRaw)) return false;
        if (!cfg.TryGetValue(keyPrefix + "_green", out string? gRaw) || string.IsNullOrWhiteSpace(gRaw)) return false;
        if (!cfg.TryGetValue(keyPrefix + "_blue", out string? bRaw) || string.IsNullOrWhiteSpace(bRaw)) return false;
        if (!double.TryParse(rRaw, NumberStyles.Float, CultureInfo.InvariantCulture, out double rFraction)) return false;
        if (!double.TryParse(gRaw, NumberStyles.Float, CultureInfo.InvariantCulture, out double gFraction)) return false;
        if (!double.TryParse(bRaw, NumberStyles.Float, CultureInfo.InvariantCulture, out double bFraction)) return false;
        rPct = Math.Clamp(rFraction * 100d, 0, 100);
        gPct = Math.Clamp(gFraction * 100d, 0, 100);
        bPct = Math.Clamp(bFraction * 100d, 0, 100);
        return true;
    }

    private static double GetDouble(Dictionary<string, string> cfg, string key, double fallback)
    {
        if (cfg.TryGetValue(key, out string? raw)
            && double.TryParse(raw, NumberStyles.Float, CultureInfo.InvariantCulture, out double value))
        {
            return value;
        }
        return fallback;
    }

    private static Color ToColor(double rPct, double gPct, double bPct, double aPct)
    {
        byte r = (byte)Math.Clamp(Math.Round(rPct / 100d * 255d), 0, 255);
        byte g = (byte)Math.Clamp(Math.Round(gPct / 100d * 255d), 0, 255);
        byte b = (byte)Math.Clamp(Math.Round(bPct / 100d * 255d), 0, 255);
        byte a = (byte)Math.Clamp(Math.Round(aPct / 100d * 255d), 0, 255);
        return Color.FromArgb(a, r, g, b);
    }

    private static Color DimmableColor(double baseRp, double baseGp, double baseBp, double factor, double opacityPercent)
    {
        double r = baseRp / 100d;
        double g = baseGp / 100d;
        double b = baseBp / 100d;
        double luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;

        double outR, outG, outB;
        if (luminance > 0.5)
        {
            outR = r * factor;
            outG = g * factor;
            outB = b * factor;
        }
        else
        {
            double invFactor = 1.0 - factor;
            outR = r + (1.0 - r) * invFactor;
            outG = g + (1.0 - g) * invFactor;
            outB = b + (1.0 - b) * invFactor;
        }

        return ToColor(outR * 100d, outG * 100d, outB * 100d, opacityPercent);
    }

    private static void UpdateColor(ResourceDictionary resources, string key, Color value)
    {
        if (resources.ContainsKey(key))
        {
            resources[key] = value;
        }
        else
        {
            resources.Add(key, value);
        }
    }

    private static void UpdateBrush(ResourceDictionary resources, string key, Color value)
    {
        if (resources.ContainsKey(key) && resources[key] is SolidColorBrush brush)
        {
            brush.Color = value;
        }
        else
        {
            resources[key] = new SolidColorBrush(value);
        }
    }
}
