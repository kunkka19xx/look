using System;
using System.Collections.Generic;
using System.IO;
using LauncherApp.Services;
using Xunit;

namespace LauncherApp.Tests;

[CollectionDefinition("ConfigFileSerial", DisableParallelization = true)]
public class ConfigFileSerialCollection { }

[Collection("ConfigFileSerial")]
public class LookConfigTests
{
    [Fact]
    public void ApplyUpsert_ReplacesExistingLine_WithSameKey()
    {
        var lines = new List<string>
        {
            "# comment",
            "file_scan_depth=6",
            "file_scan_limit=8000",
        };

        LookConfig.ApplyUpsert(lines, "file_scan_depth", "4");

        Assert.Equal(
            new[] { "# comment", "file_scan_depth=4", "file_scan_limit=8000" },
            lines);
    }

    [Fact]
    public void ApplyUpsert_AppendsLine_WhenKeyMissing()
    {
        var lines = new List<string> { "file_scan_depth=6" };

        LookConfig.ApplyUpsert(lines, "new_key", "hello");

        Assert.Equal(
            new[] { "file_scan_depth=6", "new_key=hello" },
            lines);
    }

    [Fact]
    public void ApplyUpsert_IsIdempotent_WhenValueUnchanged()
    {
        var lines = new List<string>
        {
            "file_scan_roots=Desktop,C:\\Users\\haong\\Documents",
            "file_scan_depth=6",
        };
        var snapshot = new List<string>(lines);

        LookConfig.ApplyUpsert(lines, "file_scan_depth", "6");
        LookConfig.ApplyUpsert(lines, "file_scan_roots", "Desktop,C:\\Users\\haong\\Documents");

        Assert.Equal(snapshot, lines);
    }

    [Fact]
    public void ApplyRemove_DeletesMatchingLine()
    {
        var lines = new List<string>
        {
            "ui_background_image=C:/a.jpg",
            "ui_background_image_mode=fill",
            "file_scan_depth=6",
        };

        LookConfig.ApplyRemove(lines, "ui_background_image");

        Assert.Equal(
            new[] { "ui_background_image_mode=fill", "file_scan_depth=6" },
            lines);
    }

    [Fact]
    public void UpsertMany_PreservesUnknownKeysAndComments_AndIsIdempotent()
    {
        // Matches a realistic post-first-save user config - every SaveToConfig-managed key is
        // already present, plus extra comments / unknown keys we must not touch.
        string configPath = CreateTempConfig(
            "# look configuration",
            "",
            "# Backend indexing",
            "file_scan_roots=Desktop,C:\\Users\\haong\\Documents",
            "file_scan_depth=6",
            "file_scan_limit=8000",
            "file_exclude_paths=",
            "lazy_indexing_enabled=true",
            "backend_log_level=info",
            "launch_at_login=true",
            "skip_dir_names=node_modules,target,build,dist",
            "",
            "# UI theme",
            "ui_tint_red=0.08",
            "ui_blur_material=high_contrast",
            "",
            "# Added by look update",
            "alias_code=Visual Studio Code|Cursor",
            "ui_background_image=C:\\Users\\haong\\OneDrive\\Desktop\\wp.jpg",
            "ui_background_image_mode=fill",
            "ui_background_image_opacity=0.22",
            "ui_background_image_blur=3.0");

        try
        {
            Environment.SetEnvironmentVariable("LOOK_CONFIG_PATH", configPath);

            string before = NormalizeNewlines(File.ReadAllText(configPath));

            var updates = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase)
            {
                ["file_scan_depth"] = "6",
                ["file_scan_limit"] = "8000",
                ["lazy_indexing_enabled"] = "true",
                ["file_scan_roots"] = "Desktop,C:\\Users\\haong\\Documents",
                ["file_exclude_paths"] = "",
                ["backend_log_level"] = "info",
                ["launch_at_login"] = "true",
                ["ui_background_image"] = "C:\\Users\\haong\\OneDrive\\Desktop\\wp.jpg",
                ["ui_background_image_mode"] = "fill",
                ["ui_background_image_opacity"] = "0.22",
                ["ui_background_image_blur"] = "3.0",
            };
            LookConfig.UpsertMany(updates);

            string after = NormalizeNewlines(File.ReadAllText(configPath));

            // Unknown lines, comments, blank lines must all still be present.
            Assert.Contains("skip_dir_names=node_modules,target,build,dist", after);
            Assert.Contains("alias_code=Visual Studio Code|Cursor", after);
            Assert.Contains("ui_tint_red=0.08", after);
            Assert.Contains("ui_blur_material=high_contrast", after);
            Assert.Contains("# Backend indexing", after);
            Assert.Contains("# Added by look update", after);

            // User-set values must round-trip exactly.
            Assert.Contains("file_scan_depth=6", after);
            Assert.Contains("file_scan_limit=8000", after);
            Assert.Contains("file_scan_roots=Desktop,C:\\Users\\haong\\Documents", after);
            Assert.DoesNotContain("file_scan_depth=4", after);

            // Full-file idempotency: writing identical values must not change the file.
            Assert.Equal(before, after);
        }
        finally
        {
            Environment.SetEnvironmentVariable("LOOK_CONFIG_PATH", null);
            File.Delete(configPath);
        }
    }

    [Fact]
    public void UpsertMany_FirstSave_OnRustWrittenConfig_AppendsMissingKeys_ButPreservesExistingValues()
    {
        // Rust writes the initial config template via ensure_default_config_file. It uses
        // slightly different keys than the Windows UI expects. This test pins down what the
        // very first Save-click does to a fresh Rust-written config - specifically that the
        // user's indexing values are preserved even though we append a few new keys.
        string configPath = CreateTempConfig(
            "# look configuration",
            "app_scan_roots=",
            "file_scan_roots=Desktop,C:\\Users\\haong\\Documents",
            "file_scan_depth=6",
            "file_scan_limit=8000",
            "lazy_indexing_enabled=true",
            "ui_blur_material=high_contrast");

        try
        {
            Environment.SetEnvironmentVariable("LOOK_CONFIG_PATH", configPath);

            LookConfig.UpsertMany(new Dictionary<string, string>
            {
                ["file_scan_depth"] = "6",
                ["file_scan_limit"] = "8000",
                ["lazy_indexing_enabled"] = "true",
                ["file_scan_roots"] = "Desktop,C:\\Users\\haong\\Documents",
                ["file_exclude_paths"] = "",
                ["backend_log_level"] = "error",
                ["launch_at_login"] = "true",
            });

            string after = File.ReadAllText(configPath);
            // Critical: the user's file_scan_depth / scan_roots are preserved, not reset to UI defaults.
            Assert.Contains("file_scan_depth=6", after);
            Assert.Contains("file_scan_roots=Desktop,C:\\Users\\haong\\Documents", after);
            Assert.Contains("ui_blur_material=high_contrast", after); // untouched
            Assert.Contains("app_scan_roots=", after);               // Rust-only key untouched
        }
        finally
        {
            Environment.SetEnvironmentVariable("LOOK_CONFIG_PATH", null);
            File.Delete(configPath);
        }
    }

    [Fact]
    public void UpsertMany_AppliesRemovals_AndLeavesOtherKeysIntact()
    {
        string configPath = CreateTempConfig(
            "file_scan_depth=6",
            "ui_background_image=C:/a.jpg",
            "ui_background_image_mode=fill",
            "ui_background_image_opacity=0.22",
            "ui_background_image_blur=3.0",
            "ui_tint_red=0.08");

        try
        {
            Environment.SetEnvironmentVariable("LOOK_CONFIG_PATH", configPath);

            LookConfig.UpsertMany(
                new Dictionary<string, string> { ["file_scan_depth"] = "6" },
                new[]
                {
                    "ui_background_image",
                    "ui_background_image_mode",
                    "ui_background_image_opacity",
                    "ui_background_image_blur",
                });

            string after = File.ReadAllText(configPath);
            Assert.DoesNotContain("ui_background_image", after);
            Assert.Contains("file_scan_depth=6", after);
            Assert.Contains("ui_tint_red=0.08", after);
        }
        finally
        {
            Environment.SetEnvironmentVariable("LOOK_CONFIG_PATH", null);
            File.Delete(configPath);
        }
    }

    [Fact]
    public void UpsertMany_DoesNotAppendBlankScanRoots_WhenConfigHasEmptyRootsLine()
    {
        // Reproduces the worst-case clobber: config has `file_scan_roots=Desktop,...`, UI save
        // writes an empty value because _scanRoots was never populated. This test records the
        // current behavior: the empty value replaces the populated one, which is exactly the
        // bug class the user is hitting if LoadFromConfig failed to populate the list.
        string configPath = CreateTempConfig("file_scan_roots=Desktop,C:\\Users\\haong\\Documents");
        try
        {
            Environment.SetEnvironmentVariable("LOOK_CONFIG_PATH", configPath);
            LookConfig.UpsertMany(new Dictionary<string, string> { ["file_scan_roots"] = "" });
            string after = File.ReadAllText(configPath).TrimEnd();
            Assert.Equal("file_scan_roots=", after);
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

    private static string NormalizeNewlines(string text) => text.Replace("\r\n", "\n");
}
