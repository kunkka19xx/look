using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;

namespace LauncherApp.Services;

public sealed class AdvancedSettingsDto
{
    public int FileScanDepth { get; set; }
    public int FileScanLimit { get; set; }
    public bool LazyIndexingEnabled { get; set; }
    // User-added extra roots only. Rust-managed baseline roots (Desktop / Documents /
    // Downloads) live in `file_scan_roots` and are never written from the UI.
    public List<string> ExtraScanRoots { get; set; } = [];
    public List<string> ExcludedPaths { get; set; } = [];
    public string BackendLogLevel { get; set; } = "error";
    public bool LaunchAtLogin { get; set; }
    public string? BackgroundImagePath { get; set; }
    public string BackgroundImageMode { get; set; } = "fill";
    public double BackgroundImageOpacityFraction { get; set; }
    public double BackgroundImageBlur { get; set; }
}

public static class AdvancedSettingsSaveLogic
{
    // Pure mapping from the typed DTO to the updates/removals shape that LookConfig.UpsertMany
    // expects. Kept free of any UI dependency so tests can validate save output without a UI
    // thread. The UI's SaveToConfig reads the current UI state into an AdvancedSettingsDto and
    // calls this helper before handing the result to LookConfig.UpsertMany.
    public static (Dictionary<string, string> Updates, List<string>? Removals) BuildSavePayload(
        AdvancedSettingsDto dto)
    {
        var culture = CultureInfo.InvariantCulture;

        var updates = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase)
        {
            ["file_scan_depth"] = dto.FileScanDepth.ToString(culture),
            ["file_scan_limit"] = dto.FileScanLimit.ToString(culture),
            ["lazy_indexing_enabled"] = dto.LazyIndexingEnabled ? "true" : "false",
            ["file_scan_extra_roots"] = string.Join(",", dto.ExtraScanRoots.Select(EscapeCsvToken)),
            ["file_exclude_paths"] = string.Join(",", dto.ExcludedPaths.Select(EscapeCsvToken)),
            ["backend_log_level"] = dto.BackendLogLevel,
            ["launch_at_login"] = dto.LaunchAtLogin ? "true" : "false",
        };

        List<string>? removals = null;
        if (string.IsNullOrWhiteSpace(dto.BackgroundImagePath))
        {
            removals =
            [
                "ui_background_image",
                "ui_background_image_mode",
                "ui_background_image_opacity",
                "ui_background_image_blur",
            ];
        }
        else
        {
            updates["ui_background_image"] = dto.BackgroundImagePath!;
            updates["ui_background_image_mode"] = dto.BackgroundImageMode;
            updates["ui_background_image_opacity"] = dto.BackgroundImageOpacityFraction.ToString("0.00", culture);
            updates["ui_background_image_blur"] = dto.BackgroundImageBlur.ToString("0.0", culture);
        }

        return (updates, removals);
    }

    internal static string EscapeCsvToken(string value)
    {
        if (value.IndexOfAny([',', '"']) < 0)
        {
            return value;
        }

        return '"' + value.Replace("\"", "\"\"") + '"';
    }
}
