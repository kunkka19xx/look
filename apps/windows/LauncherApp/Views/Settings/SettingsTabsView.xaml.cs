using System;
using Microsoft.UI;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Windows.UI;

namespace LauncherApp.Views.Settings;

public sealed partial class SettingsTabsView : UserControl
{
    private readonly SolidColorBrush _selectedTabBrush = new(Color.FromArgb(255, 86, 126, 173));
    private readonly SolidColorBrush _idleTabBrush = new(Color.FromArgb(255, 35, 50, 69));

    public event EventHandler? CloseRequested;

    public SettingsTabsView()
    {
        this.InitializeComponent();
        SelectTab("appearance");
    }

    private void BackToLauncherButton_OnClick(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        CloseRequested?.Invoke(this, EventArgs.Empty);
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
