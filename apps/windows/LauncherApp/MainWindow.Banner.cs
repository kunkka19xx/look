using System;
using System.Numerics;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.UI.Composition;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Hosting;
using Microsoft.UI.Xaml.Media;

namespace LauncherApp;

// Inline banner overlay used for copy / reveal / save feedback. Ported from the macOS
// showBanner() style (capsule pill, 150ms ease in/out, ~1.2s hold, cancel-on-retrigger).
public sealed partial class MainWindow
{
    private CancellationTokenSource? _bannerCts;
    private bool _bannerTranslationPrepared;

    public enum BannerStyle
    {
        Success,
        Info,
        Warning,
        Error,
    }

    public async void ShowBanner(string message, BannerStyle style = BannerStyle.Success, double durationSeconds = 1.2)
    {
        _bannerCts?.Cancel();
        CancellationTokenSource cts = new();
        _bannerCts = cts;
        CancellationToken token = cts.Token;

        BannerText.Text = message;
        BannerHost.Background = new SolidColorBrush(ResolveBannerColor(style));
        BannerHost.Visibility = Visibility.Visible;

        AnimateBannerIn();

        try
        {
            await Task.Delay(TimeSpan.FromSeconds(Math.Max(0.6, durationSeconds)), token);
        }
        catch (TaskCanceledException)
        {
            return;
        }

        if (token.IsCancellationRequested)
        {
            return;
        }

        AnimateBannerOut();

        try
        {
            await Task.Delay(TimeSpan.FromMilliseconds(170), token);
        }
        catch (TaskCanceledException)
        {
            return;
        }

        if (token.IsCancellationRequested)
        {
            return;
        }

        BannerHost.Visibility = Visibility.Collapsed;
    }

    private void AnimateBannerIn()
    {
        Visual visual = ElementCompositionPreview.GetElementVisual(BannerHost);
        Compositor compositor = visual.Compositor;

        if (!_bannerTranslationPrepared)
        {
            ElementCompositionPreview.SetIsTranslationEnabled(BannerHost, true);
            _bannerTranslationPrepared = true;
        }

        ScalarKeyFrameAnimation fade = compositor.CreateScalarKeyFrameAnimation();
        fade.Duration = TimeSpan.FromMilliseconds(150);
        fade.InsertKeyFrame(0f, 0f);
        fade.InsertKeyFrame(1f, 1f);
        visual.StartAnimation("Opacity", fade);

        Vector3KeyFrameAnimation slide = compositor.CreateVector3KeyFrameAnimation();
        slide.Duration = TimeSpan.FromMilliseconds(180);
        slide.InsertKeyFrame(0f, new Vector3(0, 14, 0));
        slide.InsertKeyFrame(1f, Vector3.Zero);
        visual.Properties.StartAnimation("Translation", slide);
    }

    private void AnimateBannerOut()
    {
        Visual visual = ElementCompositionPreview.GetElementVisual(BannerHost);
        Compositor compositor = visual.Compositor;

        ScalarKeyFrameAnimation fade = compositor.CreateScalarKeyFrameAnimation();
        fade.Duration = TimeSpan.FromMilliseconds(150);
        fade.InsertKeyFrame(0f, 1f);
        fade.InsertKeyFrame(1f, 0f);
        visual.StartAnimation("Opacity", fade);
    }

    private static Windows.UI.Color ResolveBannerColor(BannerStyle style)
    {
        // Alpha ~0.42 to mirror the macOS .opacity(0.42) banner treatment.
        return style switch
        {
            BannerStyle.Success => Windows.UI.Color.FromArgb(0x6B, 0x2E, 0xCC, 0x71),
            BannerStyle.Info => Windows.UI.Color.FromArgb(0x66, 0x33, 0xA1, 0xE8),
            BannerStyle.Warning => Windows.UI.Color.FromArgb(0x73, 0xF3, 0x9C, 0x12),
            BannerStyle.Error => Windows.UI.Color.FromArgb(0x73, 0xE7, 0x4C, 0x3C),
            _ => Windows.UI.Color.FromArgb(0x6B, 0x2E, 0xCC, 0x71),
        };
    }
}
