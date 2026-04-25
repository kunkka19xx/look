using System;
using System.Collections.Generic;
using System.IO;
using LauncherApp.Services;
using Xunit;

namespace LauncherApp.Tests;

[Collection("ConfigFileSerial")]
public class AppearanceSettingsSaveLogicTests
{
    [Fact]
    public void BuildSavePayload_EmitsTintAndFontFractions_FromPercentSliders()
    {
        var dto = new AppearanceSettingsDto
        {
            TintRedPercent = 8,
            TintGreenPercent = 10,
            TintBluePercent = 12,
            TintOpacityPercent = 55,
            BlurOpacityPercent = 95,
            SettingsBlurPercent = 75,
            BlurMaterial = "high_contrast",
            FontSize = 14,
            FontName = "Segoe UI",
            FontRedPercent = 96,
            FontGreenPercent = 96,
            FontBluePercent = 98,
            FontOpacityPercent = 96,
            BorderThicknessTenths = 10,
            BorderRedPercent = 100,
            BorderGreenPercent = 100,
            BorderBluePercent = 100,
            BorderOpacityPercent = 12,
        };

        var updates = AppearanceSettingsSaveLogic.BuildSavePayload(dto);

        Assert.Equal("0.08", updates["ui_tint_red"]);
        Assert.Equal("0.10", updates["ui_tint_green"]);
        Assert.Equal("0.12", updates["ui_tint_blue"]);
        Assert.Equal("0.55", updates["ui_tint_opacity"]);
        Assert.Equal("0.95", updates["ui_blur_opacity"]);
        Assert.Equal("0.75", updates["ui_settings_blur"]);
        Assert.Equal("high_contrast", updates["ui_blur_material"]);
        Assert.Equal("Segoe UI", updates["ui_font_name"]);
        Assert.Equal("14", updates["ui_font_size"]);
        Assert.Equal("0.96", updates["ui_font_red"]);
        Assert.Equal("0.98", updates["ui_font_blue"]);
        Assert.Equal("1", updates["ui_border_thickness"]);
        Assert.Equal("1.00", updates["ui_border_red"]);
        Assert.Equal("0.12", updates["ui_border_opacity"]);
    }

    [Fact]
    public void BuildSavePayload_ClampsOutOfRangePercents()
    {
        var dto = new AppearanceSettingsDto
        {
            TintRedPercent = 250,  // over 100
            TintOpacityPercent = -15,
            BlurMaterial = "",     // empty -> balanced fallback
        };

        var updates = AppearanceSettingsSaveLogic.BuildSavePayload(dto);

        Assert.Equal("1.00", updates["ui_tint_red"]);
        Assert.Equal("0.00", updates["ui_tint_opacity"]);
        Assert.Equal("balanced", updates["ui_blur_material"]);
    }

    [Fact]
    public void SaveThroughLookConfig_PersistsThemeKeys_AndDoesNotClobberAdvancedKeys()
    {
        // Reproduces the observed regression: Save previously only persisted advanced keys, so
        // theme changes (tint, font, border, blur_material) disappeared on restart. This test
        // writes a realistic pre-save config, applies an appearance save, and asserts both
        // theme keys and unrelated keys survive.
        string configPath = CreateTempConfig(
            "file_scan_roots=Desktop,C:\\Users\\haong\\Documents",
            "file_scan_depth=10",
            "file_scan_limit=8000",
            "launch_at_login=true",
            "ui_tint_red=0.08",
            "ui_blur_material=balanced");

        try
        {
            Environment.SetEnvironmentVariable("LOOK_CONFIG_PATH", configPath);

            var dto = new AppearanceSettingsDto
            {
                TintRedPercent = 20,
                TintGreenPercent = 30,
                TintBluePercent = 40,
                TintOpacityPercent = 70,
                BlurOpacityPercent = 85,
                SettingsBlurPercent = 65,
                BlurMaterial = "high_contrast",
                FontSize = 15,
                FontName = "Cascadia Code",
                FontRedPercent = 96,
                FontGreenPercent = 96,
                FontBluePercent = 98,
                FontOpacityPercent = 96,
                BorderThicknessTenths = 15,
                BorderRedPercent = 50,
                BorderGreenPercent = 60,
                BorderBluePercent = 70,
                BorderOpacityPercent = 25,
            };
            LookConfig.UpsertMany(AppearanceSettingsSaveLogic.BuildSavePayload(dto));

            string after = File.ReadAllText(configPath);

            // Theme keys persisted with new values.
            Assert.Contains("ui_tint_red=0.20", after);
            Assert.Contains("ui_blur_material=high_contrast", after);
            Assert.Contains("ui_settings_blur=0.65", after);
            Assert.Contains("ui_font_name=Cascadia Code", after);
            Assert.Contains("ui_border_thickness=1.5", after);

            // Unrelated keys untouched.
            Assert.Contains("file_scan_roots=Desktop,C:\\Users\\haong\\Documents", after);
            Assert.Contains("file_scan_depth=10", after);
            Assert.Contains("launch_at_login=true", after);
        }
        finally
        {
            Environment.SetEnvironmentVariable("LOOK_CONFIG_PATH", null);
            File.Delete(configPath);
        }
    }

    private static string CreateTempConfig(params string[] lines)
    {
        string path = Path.Combine(
            Path.GetTempPath(),
            $"look-test-config-{Guid.NewGuid():N}.cfg");
        File.WriteAllText(path, string.Join("\n", lines) + "\n");
        return path;
    }
}
