using System;
using System.Diagnostics;
using LauncherApp.Bridge;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace LauncherApp.Views.Settings;

public sealed partial class SettingsTabsView : UserControl
{
    private readonly Brush _selectedTabBrush;
    private readonly Brush _idleTabBrush;

    public event EventHandler? CloseRequested;

    public SettingsTabsView()
    {
        this.InitializeComponent();
        _selectedTabBrush = ResolveBrush("LauncherAccentBrush", Windows.UI.Color.FromArgb(170, 86, 126, 173));
        _idleTabBrush = ResolveBrush("LauncherPanelAltBrush", Windows.UI.Color.FromArgb(120, 35, 50, 69));
        SelectTab("appearance");
    }

    private static Brush ResolveBrush(string key, Windows.UI.Color fallback)
    {
        if (Application.Current?.Resources is not null
            && Application.Current.Resources.ContainsKey(key)
            && Application.Current.Resources[key] is Brush brush)
        {
            return brush;
        }

        return new SolidColorBrush(fallback);
    }

    private void BackToLauncherButton_OnClick(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        CloseRequested?.Invoke(this, EventArgs.Empty);
    }

    private void SaveConfigButton_OnClick(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        bool ok = true;
        try
        {
            AppearanceTabContent.ApplyCurrentSettings();
            AppearanceTabContent.SaveToConfig();
            AdvancedTabContent.SaveToConfig();
        }
        catch (Exception ex)
        {
            ok = false;
            Debug.WriteLine($"[SettingsTabsView] save failed: {ex.Message}");
        }

        // macOS posts `lookReloadConfigRequested` after save so the Rust engine re-reads the
        // config without requiring an app restart. Mirror that here so scan-root / depth /
        // exclude / theme changes take effect immediately instead of waiting until next launch.
        try
        {
            FfiBindings.look_reload_config();
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"[SettingsTabsView] look_reload_config failed: {ex.Message}");
        }

        if (global::LauncherApp.App.MainAppWindow is global::LauncherApp.MainWindow window)
        {
            window.ShowBanner(
                ok ? "Settings saved" : "Save failed",
                ok ? global::LauncherApp.MainWindow.BannerStyle.Success : global::LauncherApp.MainWindow.BannerStyle.Error);
        }
    }

    private void AppearanceTabButton_OnClick(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        SelectTab("appearance");
    }

    private void AdvancedTabButton_OnClick(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        SelectTab("advanced");
    }

    private void ShortcutsTabButton_OnClick(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        SelectTab("shortcuts");
    }

    private void SelectTab(string tab)
    {
        bool isAppearance = tab == "appearance";
        bool isAdvanced = tab == "advanced";
        bool isShortcuts = tab == "shortcuts";

        AppearanceTabContent.Visibility = isAppearance ? Microsoft.UI.Xaml.Visibility.Visible : Microsoft.UI.Xaml.Visibility.Collapsed;
        AdvancedTabContent.Visibility = isAdvanced ? Microsoft.UI.Xaml.Visibility.Visible : Microsoft.UI.Xaml.Visibility.Collapsed;
        ShortcutsTabContent.Visibility = isShortcuts ? Microsoft.UI.Xaml.Visibility.Visible : Microsoft.UI.Xaml.Visibility.Collapsed;

        AppearanceTabButton.Background = isAppearance ? _selectedTabBrush : _idleTabBrush;
        AdvancedTabButton.Background = isAdvanced ? _selectedTabBrush : _idleTabBrush;
        ShortcutsTabButton.Background = isShortcuts ? _selectedTabBrush : _idleTabBrush;
    }
}
