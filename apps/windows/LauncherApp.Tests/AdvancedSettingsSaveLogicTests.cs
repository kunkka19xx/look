using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using LauncherApp.Services;
using Xunit;

namespace LauncherApp.Tests;

[Collection("ConfigFileSerial")]
public class AdvancedSettingsSaveLogicTests
{
    [Fact]
    public void BuildSavePayload_EmitsUserSetDepthAndLimit()
    {
        var dto = new AdvancedSettingsDto
        {
            FileScanDepth = 8,
            FileScanLimit = 15000,
            LazyIndexingEnabled = true,
            ExtraScanRoots =["Desktop", "C:\\Users\\haong\\Documents"],
            ExcludedPaths = [],
            BackendLogLevel = "info",
            LaunchAtLogin = true,
            BackgroundImagePath = null,
        };

        (var updates, var removals) = AdvancedSettingsSaveLogic.BuildSavePayload(dto);

        Assert.Equal("8", updates["file_scan_depth"]);
        Assert.Equal("15000", updates["file_scan_limit"]);
        Assert.Equal("true", updates["lazy_indexing_enabled"]);
        Assert.Equal("Desktop,C:\\Users\\haong\\Documents", updates["file_scan_extra_roots"]);
        Assert.Equal("", updates["file_exclude_paths"]);
        Assert.Equal("info", updates["backend_log_level"]);
        Assert.Equal("true", updates["launch_at_login"]);
        Assert.NotNull(removals);
        Assert.Contains("ui_background_image", removals!);
    }

    [Fact]
    public void BuildSavePayload_WithBackgroundImage_EmitsImageKeys_AndDropsRemovals()
    {
        var dto = new AdvancedSettingsDto
        {
            FileScanDepth = 6,
            FileScanLimit = 8000,
            ExtraScanRoots =["Desktop"],
            BackgroundImagePath = "C:\\pics\\bg.png",
            BackgroundImageMode = "fill",
            BackgroundImageOpacityFraction = 0.22,
            BackgroundImageBlur = 3.0,
        };

        (var updates, var removals) = AdvancedSettingsSaveLogic.BuildSavePayload(dto);

        Assert.Null(removals);
        Assert.Equal("C:\\pics\\bg.png", updates["ui_background_image"]);
        Assert.Equal("fill", updates["ui_background_image_mode"]);
        Assert.Equal("0.22", updates["ui_background_image_opacity"]);
        Assert.Equal("3.0", updates["ui_background_image_blur"]);
    }

    [Fact]
    public void SaveThroughLookConfig_PersistsUserSetDepth()
    {
        // End-to-end: a DTO with depth=8 must write file_scan_depth=8 to disk via UpsertMany.
        // Reproduces what SaveToConfig does minus the UI read. If this passes and the user
        // still sees depth reset, the loss is in reading the UI control value (e.g. NumberBox
        // Value not committed), not in the save pipeline.
        string configPath = CreateTempConfig(
            "file_scan_depth=4",
            "file_scan_limit=8000",
            "file_scan_roots=Desktop,Documents,Downloads",
            "lazy_indexing_enabled=true",
            "launch_at_login=true",
            "ui_blur_material=high_contrast");

        try
        {
            Environment.SetEnvironmentVariable("LOOK_CONFIG_PATH", configPath);

            var dto = new AdvancedSettingsDto
            {
                FileScanDepth = 8,
                FileScanLimit = 8000,
                LazyIndexingEnabled = true,
                ExtraScanRoots =["Desktop", "C:\\Users\\haong\\Documents"],
                ExcludedPaths = [],
                BackendLogLevel = "info",
                LaunchAtLogin = true,
                BackgroundImagePath = null,
            };
            (var updates, var removals) = AdvancedSettingsSaveLogic.BuildSavePayload(dto);
            LookConfig.UpsertMany(updates, removals);

            string after = File.ReadAllText(configPath);

            Assert.Contains("file_scan_depth=8", after);
            Assert.DoesNotContain("file_scan_depth=4", after);
            // Baseline file_scan_roots stays Rust-managed and is NOT overwritten by Save.
            Assert.Contains("file_scan_roots=Desktop,Documents,Downloads", after);
            // User additions go to file_scan_extra_roots.
            Assert.Contains("file_scan_extra_roots=Desktop,C:\\Users\\haong\\Documents", after);
            Assert.Contains("ui_blur_material=high_contrast", after);
        }
        finally
        {
            Environment.SetEnvironmentVariable("LOOK_CONFIG_PATH", null);
            File.Delete(configPath);
        }
    }

    [Fact]
    public void BuildSavePayload_EscapesCsvTokens_WithCommasOrQuotes()
    {
        var dto = new AdvancedSettingsDto
        {
            FileScanDepth = 4,
            FileScanLimit = 8000,
            ExtraScanRoots =["C:\\Users\\name, surname\\Projects", "C:\\path\\with\"quote"],
        };

        (var updates, _) = AdvancedSettingsSaveLogic.BuildSavePayload(dto);

        Assert.Equal(
            "\"C:\\Users\\name, surname\\Projects\",\"C:\\path\\with\"\"quote\"",
            updates["file_scan_extra_roots"]);
    }

    [Fact]
    public void BuildSavePayload_EmptyScanRoots_WritesEmptyValue()
    {
        // Pins the known clobber: if the UI's _scanRoots list is empty at save time, the
        // config's file_scan_roots line is replaced with an empty value. This is *not* the
        // depth bug but is the same class of "UI feeds wrong input to save" regression.
        var dto = new AdvancedSettingsDto
        {
            FileScanDepth = 4,
            FileScanLimit = 8000,
            ExtraScanRoots =[],
        };

        (var updates, _) = AdvancedSettingsSaveLogic.BuildSavePayload(dto);

        Assert.Equal("", updates["file_scan_extra_roots"]);
    }

    [Fact]
    public void Save_DoesNotTouch_FileScanRoots_ButWrites_FileScanExtraRoots()
    {
        // The Extra Scan Dirs UI now owns `file_scan_extra_roots`. The baseline list under
        // `file_scan_roots` (Rust-managed Desktop/Documents/Downloads) must survive a Save.
        string configPath = CreateTempConfig(
            "file_scan_roots=Desktop,Documents,Downloads",
            "file_scan_extra_roots=",
            "file_scan_depth=6",
            "file_scan_limit=8000");

        try
        {
            Environment.SetEnvironmentVariable("LOOK_CONFIG_PATH", configPath);

            var dto = new AdvancedSettingsDto
            {
                FileScanDepth = 6,
                FileScanLimit = 8000,
                ExtraScanRoots = ["C:\\Users\\haong\\Projects"],
            };
            (var updates, var removals) = AdvancedSettingsSaveLogic.BuildSavePayload(dto);
            LookConfig.UpsertMany(updates, removals);

            string after = File.ReadAllText(configPath);
            Assert.Contains("file_scan_roots=Desktop,Documents,Downloads", after);
            Assert.Contains("file_scan_extra_roots=C:\\Users\\haong\\Projects", after);
            Assert.DoesNotContain("file_scan_roots=C:", after); // did not overwrite baseline
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
