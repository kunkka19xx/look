using System.Collections.Generic;
using System.Linq;
using LauncherApp.Services;
using LauncherApp.Views;
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
    private string _appliedBlurStyle = string.Empty;
    private string _appliedFontName = string.Empty;
    private double _appliedFontSize = -1;
    private (double R, double G, double B)? _textSecondaryOverride;
    private (double R, double G, double B)? _textMutedOverride;

    public AppearanceSettingsTabView()
    {
        _isInitializing = true;
        InitializeComponent();
        LoadInstalledFonts();
        InitializeFromCurrentTheme();
        // Seed typography trackers to match what's already on screen so the first Save
        // doesn't see the -1 / "" sentinels as "user changed font" and walk the visual
        // tree forcing FontSize on every Control/TextBlock - which would clobber explicit
        // XAML sizes like the FontSize="11" hint text. ReloadFromConfig deliberately
        // re-resets these to the sentinels so a fresh-config reset still does fire the
        // tree walk and clear stale per-instance overrides; that path is unaffected.
        _appliedFontSize = FontSizeSlider.Value;
        _appliedFontName = FontNameInput.Text ?? string.Empty;
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
            BlurStyleCombo.SelectedIndex = 2;
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

        // Restore preset overrides so live preview keeps the theme's signature secondary
        // / muted tint after a restart. Without this, opening the settings panel would
        // reset the in-memory overrides and the next ApplyThemePreview would fall back
        // to DimmableColor, dropping the saved theme's color signature.
        if (resources.ContainsKey("LauncherColorSecondary") && resources["LauncherColorSecondary"] is Color secondaryColor)
        {
            _textSecondaryOverride = (ToPercent(secondaryColor.R), ToPercent(secondaryColor.G), ToPercent(secondaryColor.B));
        }

        if (resources.ContainsKey("LauncherColorMuted") && resources["LauncherColorMuted"] is Color mutedColor)
        {
            _textMutedOverride = (ToPercent(mutedColor.R), ToPercent(mutedColor.G), ToPercent(mutedColor.B));
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

        // SettingsBlur (0..100) isn't encoded in a XAML resource, read directly from config
        // so the slider matches what MainWindow's _settingsBlurPercent is using.
        var cfg = LookConfig.Read();
        if (cfg.TryGetValue("ui_settings_blur", out string? settingsBlurRaw)
            && double.TryParse(settingsBlurRaw, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out double settingsBlurFraction))
        {
            SettingsBlurSlider.Value = System.Math.Clamp(settingsBlurFraction * 100d, 40, 100);
        }

        string style = mainWindow?.CurrentBlurStyle ?? "balanced";
        BlurStyleCombo.SelectedIndex = BlurStyleIndexFromId(style);
        _appliedBlurStyle = style;

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
            mainWindow.UpdateSettingsBlur(SettingsBlurSlider.Value);
        }
    }

    private static void SetColorSliders(Color color, LabeledSliderView red, LabeledSliderView green, LabeledSliderView blue, LabeledSliderView alpha)
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

    private void BlurStyleCombo_OnSelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_isInitializing || !IsLoaded)
        {
            return;
        }

        ApplyBlurStyle();
    }

    private static readonly string[] BlurStyleIds = ["high_contrast", "soft", "balanced", "subtle"];

    private static int BlurStyleIndexFromId(string id)
    {
        string lowered = (id ?? string.Empty).Trim().ToLowerInvariant().Replace('-', '_').Replace(' ', '_');
        return lowered switch
        {
            "high_contrast" or "hudwindow" or "hud_window" => 0,
            "soft" or "sidebar" => 1,
            "subtle" or "underwindowbackground" or "under_window_background" => 3,
            _ => 2,
        };
    }

    private void ApplyBlurStyle()
    {
        int index = BlurStyleCombo.SelectedIndex;
        if (index < 0 || index >= BlurStyleIds.Length)
        {
            return;
        }
        string id = BlurStyleIds[index];

        if (id.Equals(_appliedBlurStyle, System.StringComparison.OrdinalIgnoreCase))
        {
            return;
        }

        if (global::LauncherApp.App.MainAppWindow is global::LauncherApp.MainWindow window)
        {
            window.SetBlurStyle(id);
            _appliedBlurStyle = id;
            LookConfig.Upsert("ui_blur_material", id);
        }
    }

    private readonly record struct ThemePreset(
        double TintR, double TintG, double TintB, double TintOpacity,
        double TextR, double TextG, double TextB, double TextOpacity,
        double BorderR, double BorderG, double BorderB, double BorderOpacity,
        double BlurOpacity,
        double TextSecondaryR, double TextSecondaryG, double TextSecondaryB,
        double TextMutedR, double TextMutedG, double TextMutedB);

    private static readonly Dictionary<string, ThemePreset> ThemePresets = new()
    {
        ["Catppuccin"]  = new(11,  11, 18, 58, 95, 94, 98, 97, 80, 75, 93, 20, 94, 80, 84, 96, 67, 70, 78),
        ["Tokyo Night"] = new( 5,   8, 16, 58, 84, 87, 96, 98, 45, 54, 86, 24, 95, 74, 80, 90, 56, 64, 78),
        ["Rose Pine"]   = new(12,  10, 16, 57, 95, 93, 91, 98, 68, 63, 76, 22, 94, 88, 84, 81, 74, 70, 66),
        ["Gruvbox"]     = new(13,  10,  7, 60, 93, 89, 79, 98, 84, 54, 26, 24, 95, 87, 80, 64, 72, 64, 48),
        ["Dracula"]     = new( 9,   8, 15, 58, 97, 97, 98, 98, 74, 55, 89, 24, 94, 92, 87, 98, 77, 74, 85),
        ["Kanagawa"]    = new( 9,  10, 12, 58, 87, 86, 79, 98, 58, 52, 42, 22, 95, 80, 78, 66, 66, 63, 50),
    };

    private void PresetCombo_OnSelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_isInitializing || !IsLoaded)
        {
            return;
        }

        string name = (PresetCombo.SelectedItem as ComboBoxItem)?.Content?.ToString() ?? string.Empty;
        if (!ThemePresets.TryGetValue(name, out ThemePreset preset))
        {
            return;
        }

        _isInitializing = true;
        try
        {
            TintRedSlider.Value = preset.TintR;
            TintGreenSlider.Value = preset.TintG;
            TintBlueSlider.Value = preset.TintB;
            TintOpacitySlider.Value = preset.TintOpacity;

            TextRedSlider.Value = preset.TextR;
            TextGreenSlider.Value = preset.TextG;
            TextBlueSlider.Value = preset.TextB;
            TextOpacitySlider.Value = preset.TextOpacity;

            BorderRedSlider.Value = preset.BorderR;
            BorderGreenSlider.Value = preset.BorderG;
            BorderBlueSlider.Value = preset.BorderB;
            BorderOpacitySlider.Value = preset.BorderOpacity;

            BlurOpacitySlider.Value = preset.BlurOpacity;

            _textSecondaryOverride = (preset.TextSecondaryR, preset.TextSecondaryG, preset.TextSecondaryB);
            _textMutedOverride = (preset.TextMutedR, preset.TextMutedG, preset.TextMutedB);
        }
        finally
        {
            _isInitializing = false;
        }

        ApplyThemePreview();
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
        ApplyBlurStyle();
    }

    // Re-reads the current Application.Resources state into the controls so that after
    // ThemeBootstrap.ApplyFromConfig() rewrites resources from a fresh config, the
    // Appearance sliders / blur style / typography reflect the new defaults instead of
    // the stale pre-reset values. Reset the "applied" trackers so ApplyTypographyPreview
    // and ApplyBlurStyle won't short-circuit when the new value happens to match the
    // last one we pushed to MainWindow.
    public void ReloadFromConfig()
    {
        _isInitializing = true;
        try
        {
            _appliedFontName = string.Empty;
            _appliedFontSize = -1;
            _appliedBlurStyle = string.Empty;
            _textSecondaryOverride = null;
            _textMutedOverride = null;
            InitializeFromCurrentTheme();
        }
        finally
        {
            _isInitializing = false;
        }

        ApplyThemePreview();
        ApplyBlurStyle();
    }

    // Writes all appearance/theme keys to ~/.look.config. Mirrors macOS's ThemeStore save path
    // so custom tints, fonts, borders, and blur opacity survive restarts. Called by the Save
    // Config button in SettingsTabsView.
    public void SaveToConfig()
    {
        if (!IsLoaded)
        {
            return;
        }

        var dto = new AppearanceSettingsDto
        {
            TintRedPercent = TintRedSlider.Value,
            TintGreenPercent = TintGreenSlider.Value,
            TintBluePercent = TintBlueSlider.Value,
            TintOpacityPercent = TintOpacitySlider.Value,

            BlurOpacityPercent = BlurOpacitySlider.Value,
            SettingsBlurPercent = SettingsBlurSlider.Value,
            BlurMaterial = BlurStyleIds[System.Math.Clamp(BlurStyleCombo.SelectedIndex, 0, BlurStyleIds.Length - 1)],

            FontSize = FontSizeSlider.Value,
            FontName = FontNameInput.Text ?? string.Empty,
            FontRedPercent = TextRedSlider.Value,
            FontGreenPercent = TextGreenSlider.Value,
            FontBluePercent = TextBlueSlider.Value,
            FontOpacityPercent = TextOpacitySlider.Value,

            BorderThicknessTenths = BorderThicknessSlider.Value,
            BorderRedPercent = BorderRedSlider.Value,
            BorderGreenPercent = BorderGreenSlider.Value,
            BorderBluePercent = BorderBlueSlider.Value,
            BorderOpacityPercent = BorderOpacitySlider.Value,

            TextSecondaryRedPercent = _textSecondaryOverride?.R,
            TextSecondaryGreenPercent = _textSecondaryOverride?.G,
            TextSecondaryBluePercent = _textSecondaryOverride?.B,
            TextMutedRedPercent = _textMutedOverride?.R,
            TextMutedGreenPercent = _textMutedOverride?.G,
            TextMutedBluePercent = _textMutedOverride?.B,
        };

        LookConfig.UpsertMany(AppearanceSettingsSaveLogic.BuildSavePayload(dto));
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

        Color textColor = ToColor(TextRedSlider.Value, TextGreenSlider.Value, TextBlueSlider.Value, TextOpacitySlider.Value);
        // Alpha multipliers (0.94 / 0.88) match ThemeBootstrap. Higher than macOS's
        // 0.90/0.78 because Windows's opaque panel composition would otherwise grey out
        // the theme's muted/secondary RGB; keeping these in sync keeps live preview and
        // post-restart rendering visually identical.
        Color secondaryColor = _textSecondaryOverride is { } sec
            ? ToColor(sec.R, sec.G, sec.B, TextOpacitySlider.Value * 0.94)
            : DimmableColor(0.82, TextOpacitySlider.Value * 0.94);
        Color mutedColor = _textMutedOverride is { } muted
            ? ToColor(muted.R, muted.G, muted.B, TextOpacitySlider.Value * 0.88)
            : DimmableColor(0.64, TextOpacitySlider.Value * 0.88);

        UpdateBrush(resources, "LauncherTextBrush", textColor);
        UpdateBrush(resources, "LauncherSecondaryTextBrush", secondaryColor);
        UpdateBrush(resources, "LauncherMutedTextBrush", mutedColor);
        Color borderColor = ToColor(BorderRedSlider.Value, BorderGreenSlider.Value, BorderBlueSlider.Value, BorderOpacitySlider.Value);
        UpdateBrush(resources, "LauncherBorderBrush", borderColor);
        UpdateBrush(resources, "LauncherAccentBrush", ToColor(TintRedSlider.Value + 40, TintGreenSlider.Value + 45, TintBlueSlider.Value + 65, 100));

        UpdateColor(resources, "LauncherColorPanel", panelColor);
        UpdateColor(resources, "LauncherColorPanelAlt", panelAltColor);
        UpdateColor(resources, "LauncherColorText", textColor);
        UpdateColor(resources, "LauncherColorSecondary", secondaryColor);
        UpdateColor(resources, "LauncherColorMuted", mutedColor);
        UpdateColor(resources, "LauncherColorBorder", borderColor);
        double borderThicknessValue = BorderThicknessSlider.Value / 10d;
        UpdateThickness(resources, "LauncherBorderThickness", borderThicknessValue);

        if (global::LauncherApp.App.MainAppWindow is global::LauncherApp.MainWindow window)
        {
            window.UpdateAcrylicOpacity(BlurOpacitySlider.Value);
            window.UpdateSettingsBlur(SettingsBlurSlider.Value);
            window.UpdateFrameBorder(borderColor, borderThicknessValue);
            window.UpdateTopEdgeMask(panelColor, borderColor);
            window.UpdateFrameCaptionColor(panelColor);
        }

        ApplyTypographyPreview(resources);
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

    private Color DimmableColor(double factor, double opacityPercent)
    {
        double r = TextRedSlider.Value / 100d;
        double g = TextGreenSlider.Value / 100d;
        double b = TextBlueSlider.Value / 100d;
        double luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;

        double outR, outG, outB;
        if (luminance > 0.5)
        {
            outR = r * factor;
            outG = g * factor;
            outB = b * factor;
        }
        else
        {
            double invFactor = 1.0 - factor;
            outR = r + (1.0 - r) * invFactor;
            outG = g + (1.0 - g) * invFactor;
            outB = b + (1.0 - b) * invFactor;
        }

        return ToColor(outR * 100d, outG * 100d, outB * 100d, opacityPercent);
    }

    private static byte ClampToByte(double value)
    {
        if (value < 0) return 0;
        if (value > 255) return 255;
        return (byte)value;
    }

}
