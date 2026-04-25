using System;
using System.Collections.Generic;
using System.Globalization;

namespace LauncherApp.Services;

// Slider values are stored 0..100 (percent) on the UI but the config writes 0..1 fractions
// to stay compatible with macOS's ThemeSettings format.
public sealed class AppearanceSettingsDto
{
    public double TintRedPercent { get; set; }
    public double TintGreenPercent { get; set; }
    public double TintBluePercent { get; set; }
    public double TintOpacityPercent { get; set; }

    public double BlurOpacityPercent { get; set; }
    public double SettingsBlurPercent { get; set; }
    public string BlurMaterial { get; set; } = "balanced";

    public double FontSize { get; set; }
    public string FontName { get; set; } = string.Empty;
    public double FontRedPercent { get; set; }
    public double FontGreenPercent { get; set; }
    public double FontBluePercent { get; set; }
    public double FontOpacityPercent { get; set; }

    public double BorderThicknessTenths { get; set; }
    public double BorderRedPercent { get; set; }
    public double BorderGreenPercent { get; set; }
    public double BorderBluePercent { get; set; }
    public double BorderOpacityPercent { get; set; }
}

public static class AppearanceSettingsSaveLogic
{
    public static Dictionary<string, string> BuildSavePayload(AppearanceSettingsDto dto)
    {
        var culture = CultureInfo.InvariantCulture;

        return new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase)
        {
            ["ui_tint_red"] = FormatFraction(dto.TintRedPercent, culture),
            ["ui_tint_green"] = FormatFraction(dto.TintGreenPercent, culture),
            ["ui_tint_blue"] = FormatFraction(dto.TintBluePercent, culture),
            ["ui_tint_opacity"] = FormatFraction(dto.TintOpacityPercent, culture),

            ["ui_blur_material"] = string.IsNullOrWhiteSpace(dto.BlurMaterial) ? "balanced" : dto.BlurMaterial,
            ["ui_blur_opacity"] = FormatFraction(dto.BlurOpacityPercent, culture),
            ["ui_settings_blur"] = FormatFraction(dto.SettingsBlurPercent, culture),

            ["ui_font_name"] = dto.FontName ?? string.Empty,
            ["ui_font_size"] = dto.FontSize.ToString("0.#", culture),
            ["ui_font_red"] = FormatFraction(dto.FontRedPercent, culture),
            ["ui_font_green"] = FormatFraction(dto.FontGreenPercent, culture),
            ["ui_font_blue"] = FormatFraction(dto.FontBluePercent, culture),
            ["ui_font_opacity"] = FormatFraction(dto.FontOpacityPercent, culture),

            ["ui_border_thickness"] = (dto.BorderThicknessTenths / 10d).ToString("0.#", culture),
            ["ui_border_red"] = FormatFraction(dto.BorderRedPercent, culture),
            ["ui_border_green"] = FormatFraction(dto.BorderGreenPercent, culture),
            ["ui_border_blue"] = FormatFraction(dto.BorderBluePercent, culture),
            ["ui_border_opacity"] = FormatFraction(dto.BorderOpacityPercent, culture),
        };
    }

    private static string FormatFraction(double percent, CultureInfo culture)
    {
        double clamped = Math.Clamp(percent, 0d, 100d) / 100d;
        return clamped.ToString("0.00", culture);
    }
}
