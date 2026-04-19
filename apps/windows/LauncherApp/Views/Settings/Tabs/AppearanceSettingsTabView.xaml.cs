using System.Collections.Generic;
using System.Linq;
using Microsoft.Win32;
using Microsoft.UI.Xaml.Controls;

namespace LauncherApp.Views.Settings.Tabs;

public sealed partial class AppearanceSettingsTabView : UserControl
{
    private List<string> _allFonts = [];

    public AppearanceSettingsTabView()
    {
        InitializeComponent();
        InitializeDefaults();
        LoadInstalledFonts();
    }

    private void InitializeDefaults()
    {
        TintRedSlider.Value = 16;
        TintGreenSlider.Value = 24;
        TintBlueSlider.Value = 42;
        TintOpacitySlider.Value = 42;

        BlurOpacitySlider.Value = 55;
        SettingsBlurSlider.Value = 90;

        FontSizeSlider.Value = 14;

        TextRedSlider.Value = 88;
        TextGreenSlider.Value = 90;
        TextBlueSlider.Value = 95;
        TextOpacitySlider.Value = 96;

        BorderThicknessSlider.Value = 15;
        BorderRedSlider.Value = 38;
        BorderGreenSlider.Value = 43;
        BorderBlueSlider.Value = 58;
        BorderOpacitySlider.Value = 62;
    }

    private void LoadInstalledFonts()
    {
        const string keyPath = @"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts";
        using RegistryKey? fontsKey = Registry.LocalMachine.OpenSubKey(keyPath);

        _allFonts = (fontsKey?.GetValueNames() ?? [])
            .Select(CleanFontName)
            .Where(name => !string.IsNullOrWhiteSpace(name))
            .Distinct()
            .OrderBy(name => name)
            .ToList();

        FontNameInput.ItemsSource = _allFonts.Take(30).ToList();
        FontNameInput.Text = _allFonts.Contains("Segoe UI") ? "Segoe UI" : _allFonts.FirstOrDefault() ?? string.Empty;
    }

    private static string CleanFontName(string raw)
    {
        return raw
            .Replace(" (TrueType)", string.Empty)
            .Replace(" (OpenType)", string.Empty)
            .Replace(" (All res)", string.Empty)
            .Trim();
    }

    private void FontNameInput_OnTextChanged(AutoSuggestBox sender, AutoSuggestBoxTextChangedEventArgs args)
    {
        if (args.Reason != AutoSuggestionBoxTextChangeReason.UserInput)
        {
            return;
        }

        string query = sender.Text?.Trim() ?? string.Empty;
        if (query.Length == 0)
        {
            sender.ItemsSource = _allFonts.Take(30).ToList();
            return;
        }

        sender.ItemsSource = _allFonts
            .Where(name => name.Contains(query, System.StringComparison.OrdinalIgnoreCase))
            .Take(30)
            .ToList();
    }

    private void FontNameInput_OnQuerySubmitted(AutoSuggestBox sender, AutoSuggestBoxQuerySubmittedEventArgs args)
    {
        if (args.ChosenSuggestion is string selected)
        {
            sender.Text = selected;
        }
    }

}
