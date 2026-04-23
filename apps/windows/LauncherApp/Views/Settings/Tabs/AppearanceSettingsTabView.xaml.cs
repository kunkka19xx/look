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
    private string _appliedBackdropMode = string.Empty;
    private string _appliedFontName = string.Empty;
    private double _appliedFontSize = -1;

    public AppearanceSettingsTabView()
    {
        _isInitializing = true;
        InitializeComponent();
        LoadInstalledFonts();
        InitializeFromCurrentTheme();
        HookLiveEvents();
        _isInitializing = false;
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
        TintOpacitySlider.Value = 28;

        BlurOpacitySlider.Value = 42;
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
        BorderOpacitySlider.Value = 45;
    }

    private void InitializeFromCurrentTheme()
    {
        InitializeDefaults();

        if (Application.Current?.Resources is not ResourceDictionary resources)
        {
            BackdropModeCombo.SelectedIndex = 1;
            return;
        }

        Color? initialPanelColor = null;
        Color? initialBorderColor = null;
        var mainWindow = global::LauncherApp.App.MainAppWindow as global::LauncherApp.MainWindow;

        if (resources.ContainsKey("LauncherColorPanel") && resources["LauncherColorPanel"] is Color panelColor)
        {
            SetColorSliders(panelColor, TintRedSlider, TintGreenSlider, TintBlueSlider, TintOpacitySlider);
            initialPanelColor = panelColor;
        }

        if (resources.ContainsKey("LauncherColorPanelAlt") && resources["LauncherColorPanelAlt"] is Color panelAltColor)
        {
            BlurOpacitySlider.Value = ToPercent(panelAltColor.A);
        }

        if (resources.ContainsKey("LauncherColorText") && resources["LauncherColorText"] is Color textColor)
        {
            SetColorSliders(textColor, TextRedSlider, TextGreenSlider, TextBlueSlider, TextOpacitySlider);
        }

        if (resources.ContainsKey("LauncherColorBorder") && resources["LauncherColorBorder"] is Color borderColor)
        {
            SetColorSliders(borderColor, BorderRedSlider, BorderGreenSlider, BorderBlueSlider, BorderOpacitySlider);
            initialBorderColor = borderColor;
        }

        if (resources.ContainsKey("LauncherBorderThickness") && resources["LauncherBorderThickness"] is Thickness thickness)
        {
            BorderThicknessSlider.Value = thickness.Left * 10d;
        }

        if (resources.ContainsKey("ContentControlThemeFontSize") && resources["ContentControlThemeFontSize"] is double fontSize)
        {
            FontSizeSlider.Value = fontSize;
        }

        if (resources.ContainsKey("ContentControlThemeFontFamily") && resources["ContentControlThemeFontFamily"] is FontFamily family)
        {
            FontNameInput.Text = family.Source;
        }

        string mode = mainWindow?.CurrentBackdropMode ?? "Acrylic";

        BackdropModeCombo.SelectedIndex = mode.Equals("Solid", System.StringComparison.OrdinalIgnoreCase)
            ? 2
            : mode.Equals("Mica", System.StringComparison.OrdinalIgnoreCase)
                ? 0
                : 1;

        if (mainWindow is not null)
        {
            Color appliedPanelColor = initialPanelColor ?? ToColor(
                TintRedSlider.Value,
                TintGreenSlider.Value,
                TintBlueSlider.Value,
                TintOpacitySlider.Value);
            Color appliedBorderColor = initialBorderColor ?? ToColor(
                BorderRedSlider.Value,
                BorderGreenSlider.Value,
                BorderBlueSlider.Value,
                BorderOpacitySlider.Value);
            double borderThickness = BorderThicknessSlider.Value / 10d;
            mainWindow.UpdateFrameBorder(appliedBorderColor, borderThickness);
            mainWindow.UpdateTopEdgeMask(appliedPanelColor, appliedBorderColor);
        }
    }

    private static void SetColorSliders(Color color, Slider red, Slider green, Slider blue, Slider alpha)
    {
        red.Value = ToPercent(color.R);
        green.Value = ToPercent(color.G);
        blue.Value = ToPercent(color.B);
        alpha.Value = ToPercent(color.A);
    }

    private static double ToPercent(byte value)
    {
        return System.Math.Round(value / 255d * 100d);
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
        if (!IsLoaded)
        {
            return;
        }

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
        if (!IsLoaded)
        {
            return;
        }

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
        if (_isInitializing || !IsLoaded)
        {
            return;
        }

        ApplyBackdropMode();
    }

    private void Slider_OnValueChanged(object sender, RangeBaseValueChangedEventArgs e)
    {
        if (_isInitializing || !IsLoaded)
        {
            return;
        }

        ApplyThemePreview();
    }

    public void ApplyCurrentSettings()
    {
        if (_isInitializing || !IsLoaded)
        {
            return;
        }

        ApplyThemePreview();
        ApplyBackdropMode();
    }

    private void ApplyThemePreview()
    {
        if (Application.Current?.Resources is not ResourceDictionary resources)
        {
            return;
        }

        Color panelColor = ToColor(TintRedSlider.Value, TintGreenSlider.Value, TintBlueSlider.Value, TintOpacitySlider.Value);
        Color panelAltColor = ToColor(TintRedSlider.Value + 8, TintGreenSlider.Value + 8, TintBlueSlider.Value + 8, BlurOpacitySlider.Value);

        UpdateBrush(resources, "LauncherPanelBrush", panelColor);
        UpdateBrush(resources, "LauncherPanelAltBrush", panelAltColor);
        UpdateBrush(resources, "LauncherTextBrush", ToColor(TextRedSlider.Value, TextGreenSlider.Value, TextBlueSlider.Value, TextOpacitySlider.Value));
        double mutedOpacity = System.Math.Max(TextOpacitySlider.Value - 26, 20);
        UpdateBrush(resources, "LauncherMutedTextBrush", ToColor(
            System.Math.Max(TextRedSlider.Value - 24, 20),
            System.Math.Max(TextGreenSlider.Value - 24, 20),
            System.Math.Max(TextBlueSlider.Value - 24, 20),
            mutedOpacity));
        Color borderColor = ToColor(BorderRedSlider.Value, BorderGreenSlider.Value, BorderBlueSlider.Value, BorderOpacitySlider.Value);
        UpdateBrush(resources, "LauncherBorderBrush", borderColor);
        UpdateBrush(resources, "LauncherAccentBrush", ToColor(TintRedSlider.Value + 40, TintGreenSlider.Value + 45, TintBlueSlider.Value + 65, 100));

        UpdateColor(resources, "LauncherColorPanel", panelColor);
        UpdateColor(resources, "LauncherColorPanelAlt", panelAltColor);
        UpdateColor(resources, "LauncherColorText", ToColor(TextRedSlider.Value, TextGreenSlider.Value, TextBlueSlider.Value, TextOpacitySlider.Value));
        UpdateColor(resources, "LauncherColorMuted", ToColor(TextRedSlider.Value - 24, TextGreenSlider.Value - 24, TextBlueSlider.Value - 24, TextOpacitySlider.Value - 26));
        UpdateColor(resources, "LauncherColorBorder", borderColor);
        double borderThicknessValue = BorderThicknessSlider.Value / 10d;
        UpdateThickness(resources, "LauncherBorderThickness", borderThicknessValue);

        if (global::LauncherApp.App.MainAppWindow is global::LauncherApp.MainWindow window)
        {
            window.UpdateAcrylicOpacity(BlurOpacitySlider.Value);
            window.UpdateFrameBorder(borderColor, borderThicknessValue);
            window.UpdateTopEdgeMask(panelColor, borderColor);
            window.UpdateFrameCaptionColor(panelColor);
        }

        ApplyTypographyPreview(resources);
    }

    private void ApplyBackdropMode()
    {
        string mode = (BackdropModeCombo.SelectedItem as ComboBoxItem)?.Content?.ToString() ?? "Acrylic";
        if (mode.Equals(_appliedBackdropMode, System.StringComparison.OrdinalIgnoreCase))
        {
            return;
        }

        if (global::LauncherApp.App.MainAppWindow is global::LauncherApp.MainWindow window)
        {
            window.SetBackdropMode(mode);
            _appliedBackdropMode = mode;
        }
    }

    private void ApplyTypographyPreview(ResourceDictionary resources)
    {
        string fontName = FontNameInput.Text?.Trim() ?? string.Empty;
        bool hasFontName = !string.IsNullOrWhiteSpace(fontName);
        bool fontChanged = hasFontName && !fontName.Equals(_appliedFontName, System.StringComparison.OrdinalIgnoreCase);

        double fontSize = FontSizeSlider.Value;
        bool sizeChanged = System.Math.Abs(fontSize - _appliedFontSize) > 0.1;

        if (!fontChanged && !sizeChanged)
        {
            return;
        }

        if (fontChanged)
        {
            var family = new FontFamily(fontName);
            resources["ContentControlThemeFontFamily"] = family;
            resources["TextControlThemeFontFamily"] = family;
            ApplyFontFamilyToVisualTree(XamlRoot?.Content, family);
            _appliedFontName = fontName;
        }

        if (sizeChanged)
        {
            resources["ContentControlThemeFontSize"] = fontSize;
            resources["ControlContentThemeFontSize"] = fontSize;
            ApplyFontSizeToVisualTree(XamlRoot?.Content, fontSize);
            _appliedFontSize = fontSize;
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

    private static readonly string[] IconFontFamilies = ["Segoe MDL2 Assets", "Segoe Fluent Icons", "Segoe UI Symbol"];

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
            string? currentFont = text.FontFamily?.Source;
            if (string.IsNullOrEmpty(currentFont) || !IconFontFamilies.Any(iconFont =>
                currentFont.Contains(iconFont, System.StringComparison.OrdinalIgnoreCase)))
            {
                text.FontFamily = family;
            }
        }

        int count = VisualTreeHelper.GetChildrenCount(root);
        for (int i = 0; i < count; i++)
        {
            ApplyFontFamilyToVisualTree(VisualTreeHelper.GetChild(root, i), family);
        }
    }

    private static void ApplyFontSizeToVisualTree(DependencyObject? root, double size)
    {
        if (root is null)
        {
            return;
        }

        if (root is Control control)
        {
            control.FontSize = size;
        }
        else if (root is TextBlock text)
        {
            string? currentFont = text.FontFamily?.Source;
            if (string.IsNullOrEmpty(currentFont) || !IconFontFamilies.Any(iconFont =>
                currentFont.Contains(iconFont, System.StringComparison.OrdinalIgnoreCase)))
            {
                text.FontSize = size;
            }
        }

        int count = VisualTreeHelper.GetChildrenCount(root);
        for (int i = 0; i < count; i++)
        {
            ApplyFontSizeToVisualTree(VisualTreeHelper.GetChild(root, i), size);
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
