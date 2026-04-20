using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.IO;
using System.Linq;
using Microsoft.UI.Xaml.Controls;
using Windows.Storage.Pickers;
using WinRT.Interop;

namespace LauncherApp.Views.Settings.Tabs;

public sealed partial class AdvancedSettingsTabView : UserControl
{
    private readonly ObservableCollection<string> _excludedFolders = [];
    private string? _backgroundImagePath;

    private const string DefaultConfigContents = "# look configuration\n"
        + "# Generated on first launch. Edit values and press Cmd+Shift+; to reload.\n\n"
        + "# Backend indexing (file_scan_depth: 1-12, file_scan_limit: 500-50000)\n"
        + "app_scan_roots=\n"
        + "app_scan_depth=3\n"
        + "app_exclude_paths=\n"
        + "app_exclude_names=\n"
        + "file_scan_roots=Desktop,Documents,Downloads\n"
        + "file_scan_depth=4\n"
        + "file_scan_limit=8000\n"
        + "file_exclude_paths=\n"
        + "lazy_indexing_enabled=true\n"
        + "backend_log_level=error\n"
        + "launch_at_login=true\n"
        + "skip_dir_names=node_modules,target,build,dist,library,applications,old firefox data,deriveddata,pods,vendor,out,coverage,tmp,cache,venv\n\n"
        + "# UI theme\n"
        + "ui_tint_red=0.08\n"
        + "ui_tint_green=0.10\n"
        + "ui_tint_blue=0.12\n"
        + "ui_tint_opacity=0.55\n"
        + "ui_blur_material=hudWindow\n"
        + "ui_blur_opacity=0.95\n"
        + "ui_font_name=SF Pro Text\n"
        + "ui_font_size=14\n"
        + "ui_font_red=0.96\n"
        + "ui_font_green=0.96\n"
        + "ui_font_blue=0.98\n"
        + "ui_font_opacity=0.96\n"
        + "ui_border_thickness=1.0\n"
        + "ui_border_red=1.0\n"
        + "ui_border_green=1.0\n"
        + "ui_border_blue=1.0\n"
        + "ui_border_opacity=0.12\n";

    public AdvancedSettingsTabView()
    {
        InitializeComponent();
        ExcludedFoldersList.ItemsSource = _excludedFolders;
        LoadFromConfig();
        RefreshExcludedFoldersState();
    }

    public void SaveToConfig()
    {
        string path = ResolveConfigPath();
        EnsureDefaultConfigFileExists(path);

        List<string> lines = LoadLines(path);

        UpsertConfigLine(lines, "file_scan_depth", ((int)Math.Round(FileScanDepthBox.Value)).ToString());
        UpsertConfigLine(lines, "file_scan_limit", ((int)Math.Round(FileScanLimitBox.Value)).ToString());
        UpsertConfigLine(lines, "lazy_indexing_enabled", LazyIndexingToggle.IsOn ? "true" : "false");
        UpsertConfigLine(lines, "file_exclude_paths", string.Join(",", _excludedFolders.Select(EscapeCsvToken)));
        UpsertConfigLine(lines, "backend_log_level", SelectedTag(BackendLogLevelCombo, "error"));
        UpsertConfigLine(lines, "launch_at_login", LaunchAtLoginToggle.IsOn ? "true" : "false");

        if (string.IsNullOrWhiteSpace(_backgroundImagePath))
        {
            RemoveConfigLine(lines, "ui_background_image");
            RemoveConfigLine(lines, "ui_background_image_mode");
            RemoveConfigLine(lines, "ui_background_image_opacity");
            RemoveConfigLine(lines, "ui_background_image_blur");
        }
        else
        {
            UpsertConfigLine(lines, "ui_background_image", _backgroundImagePath!);
            UpsertConfigLine(lines, "ui_background_image_mode", SelectedTag(BackgroundImageModeCombo, "fill"));
            UpsertConfigLine(lines, "ui_background_image_opacity", (BackgroundImageOpacitySlider.Value / 100d).ToString("0.00"));
            UpsertConfigLine(lines, "ui_background_image_blur", BackgroundImageBlurSlider.Value.ToString("0.0"));
        }

        File.WriteAllText(path, string.Join("\n", lines) + "\n");
    }

    private void LoadFromConfig()
    {
        string path = ResolveConfigPath();
        EnsureDefaultConfigFileExists(path);

        Dictionary<string, string> values = ParseConfig(path);

        _backgroundImagePath = values.GetValueOrDefault("ui_background_image");
        BackgroundImagePathText.Text = string.IsNullOrWhiteSpace(_backgroundImagePath) ? "No image selected" : _backgroundImagePath;

        SelectComboByTag(BackgroundImageModeCombo, values.GetValueOrDefault("ui_background_image_mode"), "fill");

        if (double.TryParse(values.GetValueOrDefault("ui_background_image_opacity"), out double bgOpacity))
        {
            BackgroundImageOpacitySlider.Value = Math.Clamp(bgOpacity * 100d, 0, 100);
        }

        if (double.TryParse(values.GetValueOrDefault("ui_background_image_blur"), out double bgBlur))
        {
            BackgroundImageBlurSlider.Value = Math.Clamp(bgBlur, 0, 30);
        }

        if (int.TryParse(values.GetValueOrDefault("file_scan_depth"), out int fileDepth))
        {
            FileScanDepthBox.Value = Math.Clamp(fileDepth, 1, 12);
        }

        if (int.TryParse(values.GetValueOrDefault("file_scan_limit"), out int fileLimit))
        {
            FileScanLimitBox.Value = Math.Clamp(fileLimit, 500, 50000);
        }

        if (TryParseBool(values.GetValueOrDefault("lazy_indexing_enabled"), out bool lazy))
        {
            LazyIndexingToggle.IsOn = lazy;
        }

        if (TryParseBool(values.GetValueOrDefault("launch_at_login"), out bool launchAtLogin))
        {
            LaunchAtLoginToggle.IsOn = launchAtLogin;
        }

        SelectComboByTag(BackendLogLevelCombo, values.GetValueOrDefault("backend_log_level"), "error");

        _excludedFolders.Clear();
        foreach (string pathValue in ParseCsvTokens(values.GetValueOrDefault("file_exclude_paths")))
        {
            if (!_excludedFolders.Contains(pathValue))
            {
                _excludedFolders.Add(pathValue);
            }
        }
    }

    private void CreateFreshConfigButton_OnClick(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        string path = ResolveConfigPath();
        try
        {
            if (File.Exists(path))
            {
                File.Delete(path);
            }

            EnsureDefaultConfigFileExists(path);
            LoadFromConfig();
            RefreshExcludedFoldersState();
            FreshConfigStatusText.Text = "Fresh config created.";
        }
        catch
        {
            FreshConfigStatusText.Text = "Failed to recreate config.";
        }
    }

    private async void ChooseBackgroundImageButton_OnClick(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        var picker = new FileOpenPicker();
        picker.FileTypeFilter.Add(".png");
        picker.FileTypeFilter.Add(".jpg");
        picker.FileTypeFilter.Add(".jpeg");
        picker.FileTypeFilter.Add(".bmp");
        picker.FileTypeFilter.Add(".webp");

        IntPtr hwnd = WindowNative.GetWindowHandle(global::LauncherApp.App.MainAppWindow);
        if (hwnd != IntPtr.Zero)
        {
            InitializeWithWindow.Initialize(picker, hwnd);
        }

        var file = await picker.PickSingleFileAsync();
        if (file is null)
        {
            return;
        }

        _backgroundImagePath = file.Path;
        BackgroundImagePathText.Text = _backgroundImagePath;
    }

    private void ClearBackgroundImageButton_OnClick(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        _backgroundImagePath = null;
        BackgroundImagePathText.Text = "No image selected";
    }

    private async void AddExcludedFolderButton_OnClick(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        var picker = new FolderPicker();
        picker.FileTypeFilter.Add("*");

        IntPtr hwnd = WindowNative.GetWindowHandle(global::LauncherApp.App.MainAppWindow);
        if (hwnd != IntPtr.Zero)
        {
            InitializeWithWindow.Initialize(picker, hwnd);
        }

        var folder = await picker.PickSingleFolderAsync();
        if (folder is null)
        {
            return;
        }

        if (!_excludedFolders.Contains(folder.Path))
        {
            _excludedFolders.Add(folder.Path);
            RefreshExcludedFoldersState();
        }
    }

    private void RemoveExcludedFolderButton_OnClick(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        if (ExcludedFoldersList.SelectedItem is not string value)
        {
            return;
        }

        _excludedFolders.Remove(value);
        RefreshExcludedFoldersState();
    }

    private void RefreshExcludedFoldersState()
    {
        ExcludedFoldersEmptyText.Visibility = _excludedFolders.Count == 0
            ? Microsoft.UI.Xaml.Visibility.Visible
            : Microsoft.UI.Xaml.Visibility.Collapsed;
    }

    private static string ResolveConfigPath()
    {
        string? custom = Environment.GetEnvironmentVariable("LOOK_CONFIG_PATH");
        if (!string.IsNullOrWhiteSpace(custom))
        {
            return custom;
        }

        string profile = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
        return Path.Combine(profile, ".look.config");
    }

    private static void EnsureDefaultConfigFileExists(string path)
    {
        if (File.Exists(path))
        {
            return;
        }

        string? dir = Path.GetDirectoryName(path);
        if (!string.IsNullOrWhiteSpace(dir))
        {
            Directory.CreateDirectory(dir);
        }

        File.WriteAllText(path, DefaultConfigContents);
    }

    private static List<string> LoadLines(string path)
    {
        if (!File.Exists(path))
        {
            return [];
        }

        return File.ReadAllLines(path).ToList();
    }

    private static Dictionary<string, string> ParseConfig(string path)
    {
        var values = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        foreach (string rawLine in LoadLines(path))
        {
            string line = StripComment(rawLine).Trim();
            if (line.Length == 0)
            {
                continue;
            }

            int split = line.IndexOf('=');
            if (split <= 0)
            {
                continue;
            }

            string key = line[..split].Trim();
            string value = line[(split + 1)..].Trim();
            values[key] = value;
        }

        return values;
    }

    private static string StripComment(string line)
    {
        int idx = line.IndexOf('#');
        return idx >= 0 ? line[..idx] : line;
    }

    private static void UpsertConfigLine(List<string> lines, string key, string value)
    {
        string wanted = key + "=";
        for (int i = 0; i < lines.Count; i++)
        {
            string trimmed = StripComment(lines[i]).Trim();
            if (trimmed.StartsWith(wanted, StringComparison.OrdinalIgnoreCase))
            {
                lines[i] = key + "=" + value;
                return;
            }
        }

        lines.Add(key + "=" + value);
    }

    private static void RemoveConfigLine(List<string> lines, string key)
    {
        string wanted = key + "=";
        lines.RemoveAll(line => StripComment(line).Trim().StartsWith(wanted, StringComparison.OrdinalIgnoreCase));
    }

    private static bool TryParseBool(string? raw, out bool value)
    {
        switch (raw?.Trim().ToLowerInvariant())
        {
            case "1":
            case "true":
            case "yes":
            case "on":
                value = true;
                return true;
            case "0":
            case "false":
            case "no":
            case "off":
                value = false;
                return true;
            default:
                value = false;
                return false;
        }
    }

    private static string SelectedTag(ComboBox combo, string fallback)
    {
        if (combo.SelectedItem is ComboBoxItem item && item.Tag is string value && !string.IsNullOrWhiteSpace(value))
        {
            return value;
        }

        return fallback;
    }

    private static void SelectComboByTag(ComboBox combo, string? requestedTag, string fallbackTag)
    {
        string wanted = string.IsNullOrWhiteSpace(requestedTag) ? fallbackTag : requestedTag;
        for (int i = 0; i < combo.Items.Count; i++)
        {
            if (combo.Items[i] is ComboBoxItem item
                && item.Tag is string tag
                && tag.Equals(wanted, StringComparison.OrdinalIgnoreCase))
            {
                combo.SelectedIndex = i;
                return;
            }
        }

        for (int i = 0; i < combo.Items.Count; i++)
        {
            if (combo.Items[i] is ComboBoxItem item
                && item.Tag is string tag
                && tag.Equals(fallbackTag, StringComparison.OrdinalIgnoreCase))
            {
                combo.SelectedIndex = i;
                return;
            }
        }
    }

    private static string EscapeCsvToken(string value)
    {
        if (value.IndexOfAny([',', '"']) < 0)
        {
            return value;
        }

        return '"' + value.Replace("\"", "\"\"") + '"';
    }

    private static IEnumerable<string> ParseCsvTokens(string? raw)
    {
        if (string.IsNullOrWhiteSpace(raw))
        {
            yield break;
        }

        int i = 0;
        while (i < raw.Length)
        {
            while (i < raw.Length && char.IsWhiteSpace(raw[i]))
            {
                i++;
            }

            if (i >= raw.Length)
            {
                yield break;
            }

            bool quoted = raw[i] == '"';
            if (quoted)
            {
                i++;
            }

            var token = new System.Text.StringBuilder();
            while (i < raw.Length)
            {
                char ch = raw[i++];
                if (quoted)
                {
                    if (ch == '"')
                    {
                        if (i < raw.Length && raw[i] == '"')
                        {
                            token.Append('"');
                            i++;
                            continue;
                        }
                        break;
                    }

                    token.Append(ch);
                    continue;
                }

                if (ch == ',')
                {
                    break;
                }

                token.Append(ch);
            }

            string value = token.ToString().Trim();
            if (value.Length > 0)
            {
                yield return value;
            }

            while (i < raw.Length && raw[i] != ',')
            {
                i++;
            }

            if (i < raw.Length && raw[i] == ',')
            {
                i++;
            }
        }
    }
}
