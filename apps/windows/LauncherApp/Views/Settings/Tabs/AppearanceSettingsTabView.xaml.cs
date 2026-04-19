using System.Collections.Generic;
using System.Linq;
using Microsoft.Win32;
using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Media;
using Windows.UI;

namespace LauncherApp.Views.Settings.Tabs;

public sealed partial class AppearanceSettingsTabView : UserControl
{
    private List<string> _allFonts = [];
    private bool _isInitializing;

    public AppearanceSettingsTabView()
    {
        _isInitializing = true;
        InitializeComponent();
        InitializeDefaults();
        LoadInstalledFonts();
        HookLiveEvents();
        _isInitializing = false;
        ApplyThemePreview();
    }

    private void HookLiveEvents()
    {
        TintRedSlider.ValueChanged += Slider_OnValueChanged;
        TintGreenSlider.ValueChanged += Slider_OnValueChanged;
        TintBlueSlider.ValueChanged += Slider_OnValueChanged;
        TintOpacitySlider.ValueChanged += Slider_OnValueChanged;
        BlurOpacitySlider.ValueChanged += Slider_OnValueChanged;
        SettingsBlurSlider.ValueChanged += Slider_OnValueChanged;
        FontSizeSlider.ValueChanged += Slider_OnValueChanged;
        TextRedSlider.ValueChanged += Slider_OnValueChanged;
        TextGreenSlider.ValueChanged += Slider_OnValueChanged;
        TextBlueSlider.ValueChanged += Slider_OnValueChanged;
        TextOpacitySlider.ValueChanged += Slider_OnValueChanged;
        BorderThicknessSlider.ValueChanged += Slider_OnValueChanged;
        BorderRedSlider.ValueChanged += Slider_OnValueChanged;
        BorderGreenSlider.ValueChanged += Slider_OnValueChanged;
        BorderBlueSlider.ValueChanged += Slider_OnValueChanged;
        BorderOpacitySlider.ValueChanged += Slider_OnValueChanged;
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

        if (!_isInitializing)
        {
            ApplyThemePreview();
        }
    }

    private void FontNameInput_OnQuerySubmitted(AutoSuggestBox sender, AutoSuggestBoxQuerySubmittedEventArgs args)
    {
        if (args.ChosenSuggestion is string selected)
        {
            sender.Text = selected;
        }

        if (!_isInitializing)
        {
            ApplyThemePreview();
        }
    }

    private void BackdropModeCombo_OnSelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_isInitializing)
        {
            return;
        }

        ApplyThemePreview();
    }

    private void Slider_OnValueChanged(object sender, RangeBaseValueChangedEventArgs e)
    {
        if (_isInitializing)
        {
            return;
        }

        ApplyThemePreview();
    }

    private void ApplyThemePreview()
    {
        if (Application.Current?.Resources is not ResourceDictionary resources)
        {
            return;
        }

        UpdateBrush(resources, "LauncherPanelBrush", ToColor(TintRedSlider.Value, TintGreenSlider.Value, TintBlueSlider.Value, TintOpacitySlider.Value));
        UpdateBrush(resources, "LauncherPanelAltBrush", ToColor(TintRedSlider.Value + 8, TintGreenSlider.Value + 8, TintBlueSlider.Value + 8, BlurOpacitySlider.Value));
        UpdateBrush(resources, "LauncherTextBrush", ToColor(TextRedSlider.Value, TextGreenSlider.Value, TextBlueSlider.Value, TextOpacitySlider.Value));
        UpdateBrush(resources, "LauncherMutedTextBrush", ToColor(TextRedSlider.Value - 24, TextGreenSlider.Value - 24, TextBlueSlider.Value - 24, TextOpacitySlider.Value - 26));
        UpdateBrush(resources, "LauncherBorderBrush", ToColor(BorderRedSlider.Value, BorderGreenSlider.Value, BorderBlueSlider.Value, BorderOpacitySlider.Value));
        UpdateBrush(resources, "LauncherAccentBrush", ToColor(TintRedSlider.Value + 40, TintGreenSlider.Value + 45, TintBlueSlider.Value + 65, 100));

        UpdateColor(resources, "LauncherColorPanel", ToColor(TintRedSlider.Value, TintGreenSlider.Value, TintBlueSlider.Value, TintOpacitySlider.Value));
        UpdateColor(resources, "LauncherColorPanelAlt", ToColor(TintRedSlider.Value + 8, TintGreenSlider.Value + 8, TintBlueSlider.Value + 8, BlurOpacitySlider.Value));
        UpdateColor(resources, "LauncherColorText", ToColor(TextRedSlider.Value, TextGreenSlider.Value, TextBlueSlider.Value, TextOpacitySlider.Value));
        UpdateColor(resources, "LauncherColorMuted", ToColor(TextRedSlider.Value - 24, TextGreenSlider.Value - 24, TextBlueSlider.Value - 24, TextOpacitySlider.Value - 26));
        UpdateColor(resources, "LauncherColorBorder", ToColor(BorderRedSlider.Value, BorderGreenSlider.Value, BorderBlueSlider.Value, BorderOpacitySlider.Value));
        UpdateThickness(resources, "LauncherBorderThickness", BorderThicknessSlider.Value / 10d);

        string fontName = FontNameInput.Text?.Trim() ?? string.Empty;
        if (!string.IsNullOrWhiteSpace(fontName))
        {
            var family = new FontFamily(fontName);
            resources["ContentControlThemeFontFamily"] = family;
            resources["TextControlThemeFontFamily"] = family;
            ApplyFontFamilyToVisualTree(XamlRoot?.Content, family);
        }

        string mode = (BackdropModeCombo.SelectedItem as ComboBoxItem)?.Content?.ToString() ?? "Mica";
        if (global::LauncherApp.App.MainAppWindow is global::LauncherApp.MainWindow window)
        {
            window.SetBackdropMode(mode);
        }
    }

    private static void UpdateBrush(ResourceDictionary resources, string key, Color color)
    {
        if (resources.ContainsKey(key) && resources[key] is SolidColorBrush brush)
        {
            brush.Color = color;
        }
    }

    private static void UpdateColor(ResourceDictionary resources, string key, Color color)
    {
        if (resources.ContainsKey(key))
        {
            resources[key] = color;
        }
    }

    private static void UpdateThickness(ResourceDictionary resources, string key, double value)
    {
        if (resources.ContainsKey(key))
        {
            resources[key] = new Thickness(value);
        }
    }

    private static void ApplyFontFamilyToVisualTree(DependencyObject? root, FontFamily family)
    {
        if (root is null)
        {
            return;
        }

        if (root is Control control)
        {
            control.FontFamily = family;
        }
        else if (root is TextBlock text)
        {
            text.FontFamily = family;
        }

        int count = VisualTreeHelper.GetChildrenCount(root);
        for (int i = 0; i < count; i++)
        {
            ApplyFontFamilyToVisualTree(VisualTreeHelper.GetChild(root, i), family);
        }
    }

    private static Color ToColor(double r, double g, double b, double a)
    {
        byte red = ClampToByte(r / 100d * 255d);
        byte green = ClampToByte(g / 100d * 255d);
        byte blue = ClampToByte(b / 100d * 255d);
        byte alpha = ClampToByte(a / 100d * 255d);
        return Color.FromArgb(alpha, red, green, blue);
    }

    private static byte ClampToByte(double value)
    {
        if (value < 0) return 0;
        if (value > 255) return 255;
        return (byte)value;
    }

}
