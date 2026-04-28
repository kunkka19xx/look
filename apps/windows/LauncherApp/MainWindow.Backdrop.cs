using System;
using System.Collections.Generic;
using System.IO;
using System.Runtime.InteropServices;
using System.Threading.Tasks;
using LauncherApp.Services;
using Microsoft.UI.Composition;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.UI.Xaml.Media;
using WinRT.Interop;
using WinUIEx;

namespace LauncherApp;

// Backdrop, blur, tint, background image, and DWM frame management for the launcher window.
// Centralizes everything that paints the window chrome so the rest of MainWindow can stay
// focused on app logic.
public sealed partial class MainWindow
{
    private string _blurStyle = "balanced";
    private double _blurTintScale = 1.0;
    private double _blurRadiusScale = 1.0;
    private double _blurOpacityPercent = 42;
    private double _settingsBlurPercent = 90;
    private Windows.UI.Color _acrylicTint = Windows.UI.Color.FromArgb(45, 21, 28, 38);
    private TransparentTintBackdrop _transparentBackdrop = null!;
    private Windows.UI.Color _frameBorderColor = Windows.UI.Color.FromArgb(0x66, 0x27, 0x34, 0x46);
    private double _frameBorderThickness = 0.15;
    private bool _frameBorderHidden;
    private Microsoft.Graphics.Canvas.CanvasDevice? _bgCanvasDevice;
    private Microsoft.Graphics.Canvas.CanvasBitmap? _bgCachedBitmap;
    private string? _bgCachedBitmapPath;
    private CompositionEffectBrush? _backdropBlurBrush;

    public string CurrentBlurStyle => _blurStyle;

    private void ApplySettingsBlurFromConfig()
    {
        string? raw = LookConfig.Get("ui_settings_blur");
        if (!string.IsNullOrWhiteSpace(raw)
            && double.TryParse(raw, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out double fraction))
        {
            UpdateSettingsBlur(fraction * 100d);
        }
    }

    private void ApplyBlurOpacityFromConfig()
    {
        string? raw = LookConfig.Get("ui_blur_opacity");
        if (!string.IsNullOrWhiteSpace(raw)
            && double.TryParse(raw, System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out double fraction))
        {
            UpdateAcrylicOpacity(fraction * 100d);
        }
    }

    private void ApplyAcrylicBackdrop()
    {
        SystemBackdrop = _transparentBackdrop;
        ApplyFrameStyle(removeRoundedCorners: true, removeBorder: true);
    }

    public void SetBlurStyle(string rawId)
    {
        string id = NormalizeBlurStyle(rawId);
        _blurStyle = id;
        (double tintScale, double radiusScale) = ResolveBlurStyleScales(id);
        _blurTintScale = tintScale;
        _blurRadiusScale = radiusScale;
        ApplyBlurTint();
    }

    private static string NormalizeBlurStyle(string raw)
    {
        string v = (raw ?? string.Empty).Trim().ToLowerInvariant().Replace('-', '_').Replace(' ', '_');
        return v switch
        {
            "high_contrast" or "highcontrast" or "hudwindow" or "hud_window" => "high_contrast",
            "soft" or "sidebar" => "soft",
            "balanced" or "balance" or "menu" => "balanced",
            "subtle" or "underwindowbackground" or "under_window_background" => "subtle",
            _ => "balanced",
        };
    }

    private static (double TintScale, double RadiusScale) ResolveBlurStyleScales(string id)
    {
        // Scales mirror the macOS LauncherBlurMaterial multipliers so visual intent stays
        // aligned across platforms: high-contrast darkens/intensifies, subtle thins toward
        // fully transparent, soft lightens, balanced is neutral.
        return id switch
        {
            "high_contrast" => (1.16, 1.12),
            "soft" => (0.84, 0.86),
            "subtle" => (0.68, 0.72),
            _ => (1.0, 1.0),
        };
    }

    private void ApplyRuntimeIcon()
    {
        string iconPath = Path.Combine(AppContext.BaseDirectory, "Assets", "look.ico");
        if (File.Exists(iconPath))
        {
            this.SetIcon(iconPath);
        }
    }

    public void UpdateAcrylicOpacity(double opacityPercent)
    {
        _blurOpacityPercent = Math.Clamp(opacityPercent, 0, 100);
        ApplyBlurTint();
    }

    public void UpdateSettingsBlur(double settingsBlurPercent)
    {
        _settingsBlurPercent = Math.Clamp(settingsBlurPercent, 40, 100);
        ApplyBlurTint();
    }

    private void ApplyBlurTint()
    {
        // Blur Opacity: how much the tint blocks what's behind (low = more transparent = more blur visible)
        // Settings Blur: additional opacity boost (high = more opaque = less background visible)
        // _blurTintScale comes from the user's Blur Style preset and multiplies the final alpha.
        double blurFactor = _blurOpacityPercent / 100d;
        double settingsFactor = (_settingsBlurPercent - 40d) / 60d;
        double combined = (blurFactor * 0.65 + settingsFactor * 0.35) * _blurTintScale;
        byte alpha = (byte)Math.Clamp((int)Math.Round(combined * 200d), 5, 230);
        _acrylicTint = Windows.UI.Color.FromArgb(alpha, _acrylicTint.R, _acrylicTint.G, _acrylicTint.B);
        _transparentBackdrop.TintColor = _acrylicTint;
        UpdateBlurRadius();
    }

    private void InitializeBlurLayer()
    {
        if (BlurLayer is null)
        {
            return;
        }

        try
        {
            Compositor compositor = ElementCompositionPreview.GetElementVisual(BlurLayer).Compositor;
            CompositionBackdropBrush backdropBrush = compositor.CreateBackdropBrush();

            using var blurEffect = new Microsoft.Graphics.Canvas.Effects.GaussianBlurEffect
            {
                Name = "Blur",
                BlurAmount = 0f,
                BorderMode = Microsoft.Graphics.Canvas.Effects.EffectBorderMode.Hard,
                Source = new CompositionEffectSourceParameter("backdrop"),
            };

            CompositionEffectFactory factory = compositor.CreateEffectFactory(blurEffect, new[] { "Blur.BlurAmount" });
            CompositionEffectBrush effectBrush = factory.CreateBrush();
            effectBrush.SetSourceParameter("backdrop", backdropBrush);

            SpriteVisual spriteVisual = compositor.CreateSpriteVisual();
            spriteVisual.Brush = effectBrush;
            spriteVisual.RelativeSizeAdjustment = System.Numerics.Vector2.One;

            ElementCompositionPreview.SetElementChildVisual(BlurLayer, spriteVisual);
            _backdropBlurBrush = effectBrush;

            UpdateBlurRadius();
        }
        catch
        {
            _backdropBlurBrush = null;
        }
    }

    private void UpdateBlurRadius()
    {
        if (_backdropBlurBrush is null)
        {
            return;
        }

        double primary = Math.Clamp(_blurOpacityPercent, 0, 100) / 100d * 40d;
        double secondary = Math.Clamp(_settingsBlurPercent - 40d, 0, 60d) / 60d * 20d;
        float amount = (float)Math.Max(0, (primary + secondary) * _blurRadiusScale);
        _backdropBlurBrush.Properties.InsertScalar("Blur.BlurAmount", amount);
    }

    private void ApplyImmersiveDarkModeFrame()
    {
        IntPtr hwnd = WindowNative.GetWindowHandle(this);
        if (hwnd == IntPtr.Zero)
        {
            return;
        }

        int enabled = 1;
        _ = DwmSetWindowAttribute(
            hwnd,
            (int)DwmWindowAttribute.UseImmersiveDarkMode,
            ref enabled,
            Marshal.SizeOf<int>());
    }

    public void UpdateFrameBorder(Windows.UI.Color color, double thickness)
    {
        _frameBorderColor = color;
        _frameBorderThickness = thickness;

        if (LauncherSurface is not null)
        {
            LauncherSurface.BorderThickness = new Thickness(thickness);
        }

        ApplyDwmBorderColor();
    }

    public void UpdateTopEdgeMask(Windows.UI.Color panelColor, Windows.UI.Color borderColor)
    {
        ApplyDwmCaptionColor(panelColor);
        ApplyDwmBorderColor();
    }

    public void UpdateFrameCaptionColor(Windows.UI.Color panelColor)
    {
        ApplyDwmCaptionColor(panelColor);
    }

    public void UpdateBorderThickness(Thickness thickness)
    {
        UpdateFrameBorder(_frameBorderColor, thickness.Left);
    }

    public void ApplyBackgroundImage(string? path, string mode, double opacityPercent, double blurAmount)
    {
        _ = ApplyBackgroundImageAsync(path, mode, opacityPercent, blurAmount);
    }

    private async Task ApplyBackgroundImageAsync(string? path, string mode, double opacityPercent, double blurAmount)
    {
        if (BackgroundImage is null)
        {
            return;
        }

        BackgroundImage.Stretch = mode?.ToLowerInvariant() switch
        {
            "fit" => Stretch.Uniform,
            "fill" => Stretch.UniformToFill,
            "stretch" => Stretch.Fill,
            "tile" => Stretch.None,
            _ => Stretch.UniformToFill,
        };
        BackgroundImage.Opacity = Math.Clamp(opacityPercent / 100d, 0d, 1d);

        if (string.IsNullOrWhiteSpace(path) || !File.Exists(path))
        {
            BackgroundImage.Source = null;
            _bgCachedBitmap?.Dispose();
            _bgCachedBitmap = null;
            _bgCachedBitmapPath = null;
            return;
        }

        double blur = Math.Max(0, blurAmount);

        if (blur < 0.5)
        {
            BackgroundImage.Source = new Microsoft.UI.Xaml.Media.Imaging.BitmapImage(new Uri(path));
            return;
        }

        try
        {
            _bgCanvasDevice ??= Microsoft.Graphics.Canvas.CanvasDevice.GetSharedDevice();

            if (_bgCachedBitmap is null || !string.Equals(_bgCachedBitmapPath, path, StringComparison.OrdinalIgnoreCase))
            {
                _bgCachedBitmap?.Dispose();
                _bgCachedBitmap = await Microsoft.Graphics.Canvas.CanvasBitmap.LoadAsync(_bgCanvasDevice, path);
                _bgCachedBitmapPath = path;
            }

            var size = _bgCachedBitmap.Size;
            var blurredSource = new Microsoft.Graphics.Canvas.UI.Xaml.CanvasImageSource(
                _bgCanvasDevice,
                (float)size.Width,
                (float)size.Height,
                96f);

            using (var session = blurredSource.CreateDrawingSession(Windows.UI.Color.FromArgb(0, 0, 0, 0)))
            using (var effect = new Microsoft.Graphics.Canvas.Effects.GaussianBlurEffect
            {
                Source = _bgCachedBitmap,
                BlurAmount = (float)blur,
                BorderMode = Microsoft.Graphics.Canvas.Effects.EffectBorderMode.Hard,
            })
            {
                session.DrawImage(effect);
            }

            BackgroundImage.Source = blurredSource;
        }
        catch
        {
            try
            {
                BackgroundImage.Source = new Microsoft.UI.Xaml.Media.Imaging.BitmapImage(new Uri(path));
            }
            catch
            {
                BackgroundImage.Source = null;
            }
        }
    }

    private void LoadBackgroundImageFromConfig()
    {
        try
        {
            string configPath = ResolveLookConfigPath();
            if (!File.Exists(configPath))
            {
                return;
            }

            var values = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
            foreach (string rawLine in File.ReadAllLines(configPath))
            {
                string line = rawLine;
                int commentIdx = line.IndexOf('#');
                if (commentIdx >= 0)
                {
                    line = line[..commentIdx];
                }
                line = line.Trim();
                if (line.Length == 0)
                {
                    continue;
                }
                int eq = line.IndexOf('=');
                if (eq <= 0)
                {
                    continue;
                }
                values[line[..eq].Trim()] = line[(eq + 1)..].Trim();
            }

            string imagePath = values.GetValueOrDefault("ui_background_image", string.Empty);
            if (string.IsNullOrWhiteSpace(imagePath))
            {
                return;
            }

            string mode = values.GetValueOrDefault("ui_background_image_mode", "fill");
            double opacityPercent = 35;
            if (double.TryParse(values.GetValueOrDefault("ui_background_image_opacity"), System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out double opacityFraction))
            {
                opacityPercent = Math.Clamp(opacityFraction * 100d, 0, 100);
            }
            double blurAmount = 8;
            if (double.TryParse(values.GetValueOrDefault("ui_background_image_blur"), System.Globalization.NumberStyles.Float, System.Globalization.CultureInfo.InvariantCulture, out double blurParsed))
            {
                blurAmount = Math.Clamp(blurParsed, 0, 30);
            }

            ApplyBackgroundImage(imagePath, mode, opacityPercent, blurAmount);
        }
        catch
        {
        }
    }

    private static string ResolveLookConfigPath()
    {
        string? custom = Environment.GetEnvironmentVariable("LOOK_CONFIG_PATH");
        if (!string.IsNullOrWhiteSpace(custom))
        {
            return custom;
        }

        string profile = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
        return Path.Combine(profile, ".look.config");
    }

    private void ApplyFrameStyle(bool removeRoundedCorners, bool removeBorder)
    {
        IntPtr hwnd = WindowNative.GetWindowHandle(this);
        if (hwnd == IntPtr.Zero)
        {
            return;
        }

        int cornerPreference = removeRoundedCorners
            ? (int)DwmWindowCornerPreference.DoNotRound
            : (int)DwmWindowCornerPreference.Default;

        _ = DwmSetWindowAttribute(
            hwnd,
            (int)DwmWindowAttribute.WindowCornerPreference,
            ref cornerPreference,
            Marshal.SizeOf<int>());

        _frameBorderHidden = removeBorder;
        ApplyDwmBorderColor();
    }

    private void ApplyDwmBorderColor()
    {
        IntPtr hwnd = WindowNative.GetWindowHandle(this);
        if (hwnd == IntPtr.Zero)
        {
            return;
        }

        int borderColor = unchecked((int)0xFFFFFFFE);

        _ = DwmSetWindowAttribute(
            hwnd,
            (int)DwmWindowAttribute.BorderColor,
            ref borderColor,
            Marshal.SizeOf<int>());
    }

    private void ApplyDwmCaptionColor(Windows.UI.Color panelColor)
    {
        IntPtr hwnd = WindowNative.GetWindowHandle(this);
        if (hwnd == IntPtr.Zero)
        {
            return;
        }

        int captionColor = BuildColorRef(panelColor.R, panelColor.G, panelColor.B);
        _ = DwmSetWindowAttribute(
            hwnd,
            (int)DwmWindowAttribute.CaptionColor,
            ref captionColor,
            Marshal.SizeOf<int>());

        int textColor = BuildColorRef(0xE8, 0xED, 0xF7);
        _ = DwmSetWindowAttribute(
            hwnd,
            (int)DwmWindowAttribute.TextColor,
            ref textColor,
            Marshal.SizeOf<int>());
    }

    private static int BuildColorRef(byte red, byte green, byte blue)
    {
        return red | (green << 8) | (blue << 16);
    }

    private void InitializeFrameBorderState()
    {
        if (Application.Current?.Resources is not ResourceDictionary resources)
        {
            ApplyDwmBorderColor();
            return;
        }

        Windows.UI.Color panelColor = Windows.UI.Color.FromArgb(45, 21, 28, 38);

        if (resources.ContainsKey("LauncherColorBorder") && resources["LauncherColorBorder"] is Windows.UI.Color color)
        {
            _frameBorderColor = color;
        }

        if (resources.ContainsKey("LauncherColorPanel") && resources["LauncherColorPanel"] is Windows.UI.Color panel)
        {
            panelColor = panel;
        }

        if (resources.ContainsKey("LauncherBorderThickness") && resources["LauncherBorderThickness"] is Thickness thickness)
        {
            _frameBorderThickness = thickness.Left;
        }

        UpdateFrameBorder(_frameBorderColor, _frameBorderThickness);
        UpdateTopEdgeMask(panelColor, _frameBorderColor);
        UpdateFrameCaptionColor(panelColor);
    }

    private void ApplyConfiguredSurface()
    {
        LauncherSurface.Background = (Brush)Application.Current.Resources["LauncherPanelBrush"];
    }
}
