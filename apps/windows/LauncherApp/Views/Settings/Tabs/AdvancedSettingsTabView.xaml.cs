using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.IO;
using System.Linq;
using LauncherApp.Services;
using Microsoft.UI.Xaml.Controls;
using Windows.Storage.Pickers;
using WinRT.Interop;

namespace LauncherApp.Views.Settings.Tabs;

public sealed partial class AdvancedSettingsTabView : UserControl
{
    public event System.EventHandler? FreshConfigRequested;

    private readonly ObservableCollection<string> _excludedFolders = [];
    private readonly ObservableCollection<string> _scanRoots = [];
    // Pills view of _scanRoots that hides bare drive roots (e.g. "D:\") because those are
    // owned by the Detected Drives checkbox UI above. Without this filter the same drive
    // would appear twice - once as a checkbox, once as a removable pill - and removing
    // either would leave the other in a stale state.
    private readonly ObservableCollection<string> _scanRootPills = [];
    private readonly ObservableCollection<CandidateDrive> _detectedDrives = [];
    private string? _backgroundImagePath;

    private const string DefaultConfigContents = "# look configuration\n"
        + "# Generated on first launch. Edit values and press Ctrl+Shift+; to reload.\n\n"
        + "# Backend indexing (file_scan_depth: 1-12, file_scan_limit: 500-50000)\n"
        + "app_scan_roots=\n"
        + "app_scan_depth=3\n"
        + "app_exclude_paths=\n"
        + "app_exclude_names=\n"
        + "file_scan_roots=Desktop,Documents,Downloads\n"
        + "file_scan_extra_roots=\n"
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
        + "ui_blur_material=balanced\n"
        + "ui_blur_opacity=0.95\n"
        + "ui_font_name=Segoe UI\n"
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
        ScanRootsList.ItemsSource = _scanRootPills;
        DetectedDrivesList.ItemsSource = _detectedDrives;
        // Both views derive from _scanRoots: the pills collection filters out bare drive
        // roots, and the detected-drives list re-runs DriveDiscoveryService.Filter so that
        // adding e.g. "D:\Projects" via Add Folder immediately hides D: (now partially
        // covered) and removing it restores D: as a candidate. Without the second refresh,
        // the two views would disagree about D:'s coverage until the next reload.
        _scanRoots.CollectionChanged += (_, _) =>
        {
            RefreshScanRootPills();
            RefreshDetectedDrives();
        };
        LoadFromConfig();
        RefreshExcludedFoldersState();
        RefreshScanRootsState();
        RefreshDetectedDrives();
        HookBackgroundLiveEvents();
        ApplyBackgroundImageLive();
    }

    private void HookBackgroundLiveEvents()
    {
        BackgroundImageOpacitySlider.ValueChanged += BackgroundSlider_OnValueChanged;
        BackgroundImageBlurSlider.ValueChanged += BackgroundSlider_OnValueChanged;
        BackgroundImageModeCombo.SelectionChanged += BackgroundMode_OnSelectionChanged;
    }

    private void BackgroundSlider_OnValueChanged(object sender, Microsoft.UI.Xaml.Controls.Primitives.RangeBaseValueChangedEventArgs e)
    {
        if (!IsLoaded)
        {
            return;
        }
        ApplyBackgroundImageLive();
    }

    private void BackgroundMode_OnSelectionChanged(object sender, Microsoft.UI.Xaml.Controls.SelectionChangedEventArgs e)
    {
        if (!IsLoaded)
        {
            return;
        }
        ApplyBackgroundImageLive();
    }

    private void ApplyBackgroundImageLive()
    {
        if (global::LauncherApp.App.MainAppWindow is global::LauncherApp.MainWindow window)
        {
            window.ApplyBackgroundImage(
                _backgroundImagePath,
                SelectedTag(BackgroundImageModeCombo, "fill"),
                BackgroundImageOpacitySlider.Value,
                BackgroundImageBlurSlider.Value);
        }
    }

    public void SaveToConfig()
    {
        EnsureDefaultConfigFileExists(ResolveConfigPath());

        var dto = new AdvancedSettingsDto
        {
            // NumberBox.Value only commits on Enter/focus-loss; reading .Text first picks up
            // the user's most recent typing even if they click Save while the field is
            // still focused (which was silently discarding `file_scan_depth` changes).
            FileScanDepth = ReadNumberBoxInt(FileScanDepthBox, fallback: 4, min: 1, max: 12),
            FileScanLimit = ReadNumberBoxInt(FileScanLimitBox, fallback: 8000, min: 500, max: 50000),
            LazyIndexingEnabled = LazyIndexingToggle.IsOn,
            ExtraScanRoots = _scanRoots.ToList(),
            ExcludedPaths = _excludedFolders.ToList(),
            BackendLogLevel = SelectedTag(BackendLogLevelCombo, "error"),
            LaunchAtLogin = LaunchAtLoginToggle.IsOn,
            BackgroundImagePath = string.IsNullOrWhiteSpace(_backgroundImagePath) ? null : _backgroundImagePath,
            BackgroundImageMode = SelectedTag(BackgroundImageModeCombo, "fill"),
            BackgroundImageOpacityFraction = BackgroundImageOpacitySlider.Value / 100d,
            BackgroundImageBlur = BackgroundImageBlurSlider.Value,
        };

        (var updates, var removals) = AdvancedSettingsSaveLogic.BuildSavePayload(dto);
        LookConfig.UpsertMany(updates, removals);

        StartupRegistration.Sync(LaunchAtLoginToggle.IsOn);
    }

    private static int ReadNumberBoxInt(NumberBox box, int fallback, int min, int max)
    {
        string text = box.Text?.Trim() ?? string.Empty;
        if (int.TryParse(text, System.Globalization.NumberStyles.Integer, System.Globalization.CultureInfo.InvariantCulture, out int parsed))
        {
            return Math.Clamp(parsed, min, max);
        }

        double value = box.Value;
        if (!double.IsNaN(value))
        {
            return (int)Math.Clamp(Math.Round(value), min, max);
        }

        return fallback;
    }

    private void LoadFromConfig()
    {
        string path = ResolveConfigPath();
        EnsureDefaultConfigFileExists(path);

        Dictionary<string, string> values = ParseConfig(path);

        _backgroundImagePath = values.GetValueOrDefault("ui_background_image");
        BackgroundImagePathText.Text = string.IsNullOrWhiteSpace(_backgroundImagePath) ? "No image selected" : _backgroundImagePath;

        SelectComboByTag(BackgroundImageModeCombo, values.GetValueOrDefault("ui_background_image_mode"), "fill");

        // AdvancedSettingsSaveLogic writes these as InvariantCulture fractions ("0.22", "8");
        // parsing without invariant culture silently fails on locales that use ',' as the
        // decimal separator (German, French, Vietnamese, etc.), then the slider falls back
        // to default and the next save overwrites the config with the wrong value.
        if (double.TryParse(values.GetValueOrDefault("ui_background_image_opacity"), System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out double bgOpacity))
        {
            BackgroundImageOpacitySlider.Value = Math.Clamp(bgOpacity * 100d, 0, 100);
        }

        if (double.TryParse(values.GetValueOrDefault("ui_background_image_blur"), System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out double bgBlur))
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

        // The "Extra Scan Dirs" pills are user-added only. The baseline roots (Desktop /
        // Documents / Downloads) are Rust-managed via `file_scan_roots` and must not appear
        // here - otherwise the UI would surface built-ins as pills and Save would clobber
        // the base list. We persist user additions under `file_scan_extra_roots`.
        _scanRoots.Clear();
        foreach (string pathValue in ParseCsvTokens(values.GetValueOrDefault("file_scan_extra_roots")))
        {
            if (!_scanRoots.Contains(pathValue))
            {
                _scanRoots.Add(pathValue);
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
            // Hand off to SettingsTabsView so it can also re-bootstrap the theme resources,
            // reload the Appearance tab, clear any background image, and reload the backend.
            // Falling back to a local-only reload would leave Appearance sliders and the
            // running window's brushes / borders / fonts pinned to the pre-reset values
            // even though the file on disk is the new default.
            FreshConfigRequested?.Invoke(this, System.EventArgs.Empty);
            ReloadFromConfig();
            FreshConfigStatusText.Text = "Fresh config created.";
        }
        catch
        {
            FreshConfigStatusText.Text = "Failed to recreate config.";
        }
    }

    public void ReloadFromConfig()
    {
        LoadFromConfig();
        RefreshExcludedFoldersState();
        RefreshScanRootsState();
        RefreshDetectedDrives();
        ApplyBackgroundImageLive();
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

        using (global::LauncherApp.App.MainAppWindow?.SuppressAutoHide())
        {
            var file = await picker.PickSingleFileAsync();
            if (file is null)
            {
                return;
            }

            _backgroundImagePath = file.Path;
            BackgroundImagePathText.Text = _backgroundImagePath;
            ApplyBackgroundImageLive();
        }
    }

    private void ClearBackgroundImageButton_OnClick(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        _backgroundImagePath = null;
        BackgroundImagePathText.Text = "No image selected";
        ApplyBackgroundImageLive();
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

        using (global::LauncherApp.App.MainAppWindow?.SuppressAutoHide())
        {
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
    }

    private void ExcludedFolderRemove_OnClick(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        if (sender is Button { Tag: string path })
        {
            _excludedFolders.Remove(path);
            RefreshExcludedFoldersState();
        }
    }

    private void RefreshExcludedFoldersState()
    {
        ExcludedFoldersEmptyText.Visibility = _excludedFolders.Count == 0
            ? Microsoft.UI.Xaml.Visibility.Visible
            : Microsoft.UI.Xaml.Visibility.Collapsed;
    }

    private async void AddScanRootButton_OnClick(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        var picker = new FolderPicker();
        picker.FileTypeFilter.Add("*");

        IntPtr hwnd = WindowNative.GetWindowHandle(global::LauncherApp.App.MainAppWindow);
        if (hwnd != IntPtr.Zero)
        {
            InitializeWithWindow.Initialize(picker, hwnd);
        }

        using (global::LauncherApp.App.MainAppWindow?.SuppressAutoHide())
        {
            var folder = await picker.PickSingleFolderAsync();
            if (folder is null)
            {
                return;
            }

            if (_scanRoots.Contains(folder.Path))
            {
                ShowScanRootsNotice($"\"{folder.Path}\" is already in the list.");
                return;
            }

            string? coveringEntry = FindCoveringScanRoot(folder.Path);
            if (coveringEntry is not null)
            {
                ShowScanRootsNotice($"\"{folder.Path}\" is already covered by \"{coveringEntry}\" - entry not added.");
                return;
            }

            _scanRoots.Add(folder.Path);
            ClearScanRootsNotice();
            RefreshScanRootsState();
        }
    }

    private void ScanRootRemove_OnClick(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        if (sender is Button { Tag: string path })
        {
            _scanRoots.Remove(path);
            ClearScanRootsNotice();
            RefreshScanRootsState();
        }
    }

    private void RefreshScanRootsState()
    {
        ScanRootsEmptyText.Visibility = _scanRootPills.Count == 0
            ? Microsoft.UI.Xaml.Visibility.Visible
            : Microsoft.UI.Xaml.Visibility.Collapsed;
    }

    private void RefreshScanRootPills()
    {
        _scanRootPills.Clear();
        foreach (string entry in _scanRoots)
        {
            if (!DriveDiscoveryService.IsBareDriveRoot(entry))
            {
                _scanRootPills.Add(entry);
            }
        }
        RefreshScanRootsState();
    }

    private void RefreshDetectedDrives()
    {
        _detectedDrives.Clear();

        // Treat both the Rust-managed baseline (file_scan_roots) and the user's extra
        // additions (the in-memory _scanRoots list) as "already covered" so we don't suggest
        // a drive the user has already opted into via any path on it.
        var existingRoots = new List<string>(ParseCsvTokens(LookConfig.Get("file_scan_roots")));
        existingRoots.AddRange(_scanRoots);

        foreach (CandidateDrive drive in DriveDiscoveryService.Discover(existingRoots))
        {
            _detectedDrives.Add(drive);
        }

        DetectedDrivesEmptyText.Visibility = _detectedDrives.Count == 0
            ? Microsoft.UI.Xaml.Visibility.Visible
            : Microsoft.UI.Xaml.Visibility.Collapsed;
    }

    private void DetectedDriveCheckbox_OnClick(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        if (sender is not CheckBox cb || cb.Tag is not string letter || string.IsNullOrWhiteSpace(letter))
        {
            return;
        }

        string root = letter + ":\\";
        bool nowChecked = cb.IsChecked == true;

        if (nowChecked)
        {
            if (!_scanRoots.Any(r => string.Equals(r, root, StringComparison.OrdinalIgnoreCase)))
            {
                _scanRoots.Add(root);
            }
        }
        else
        {
            string? match = _scanRoots.FirstOrDefault(r => string.Equals(r, root, StringComparison.OrdinalIgnoreCase));
            if (match is not null)
            {
                _scanRoots.Remove(match);
            }
        }

        ClearScanRootsNotice();
        RefreshScanRootsState();
    }

    private void ShowScanRootsNotice(string message)
    {
        ScanRootsNoticeText.Text = message;
        ScanRootsNoticeText.Visibility = Microsoft.UI.Xaml.Visibility.Visible;
    }

    private void ClearScanRootsNotice()
    {
        ScanRootsNoticeText.Text = string.Empty;
        ScanRootsNoticeText.Visibility = Microsoft.UI.Xaml.Visibility.Collapsed;
    }

    private string? FindCoveringScanRoot(string candidate)
    {
        string candidateResolved = ResolveHomeRelativePath(candidate);
        if (string.IsNullOrEmpty(candidateResolved))
        {
            return null;
        }

        foreach (string existing in _scanRoots)
        {
            string existingResolved = ResolveHomeRelativePath(existing);
            if (string.IsNullOrEmpty(existingResolved))
            {
                continue;
            }

            if (string.Equals(existingResolved, candidateResolved, StringComparison.OrdinalIgnoreCase))
            {
                return existing;
            }

            string prefix = existingResolved + Path.DirectorySeparatorChar;
            if (candidateResolved.StartsWith(prefix, StringComparison.OrdinalIgnoreCase))
            {
                return existing;
            }
        }

        return null;
    }

    private static string ResolveHomeRelativePath(string entry)
    {
        if (string.IsNullOrWhiteSpace(entry))
        {
            return string.Empty;
        }

        string trimmed = entry.Trim();
        string resolved = Path.IsPathFullyQualified(trimmed)
            ? trimmed
            : Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), trimmed);

        return resolved.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
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
