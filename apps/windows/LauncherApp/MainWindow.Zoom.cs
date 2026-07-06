using System;
using System.Collections.Generic;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace LauncherApp;

// Mirrors macOS ThemeStore.uiScale (Support/ThemeStore.swift:268-278): a multiplier
// applied to text sizes across the launcher UI, driven by Ctrl+= / Ctrl+- / Ctrl+0.
// Range and step (0.7..1.8 in 0.1 increments) match macOS exactly so cross-platform
// muscle memory carries over.
//
// Why a visual-tree walk instead of XAML resource binding: Look's XAML uses many
// hardcoded FontSize="..." values per role (14 for row title, 11 for meta, 16 for
// command title, etc.). A single resource override would flatten that hierarchy.
// We cache each element's *original* FontSize on first sight, then set
// FontSize = original * _uiScale on subsequent zoom changes - preserving the per-role
// size relationships while scaling the whole tree proportionally.
public sealed partial class MainWindow
{
    private const double ZoomMin = 0.7;
    private const double ZoomMax = 1.8;
    private const double ZoomStep = 0.1;
    private double _uiScale = 1.0;
    private readonly Dictionary<TextBlock, double> _originalTextBlockSizes = new();
    private readonly Dictionary<TextBox, double> _originalTextBoxSizes = new();

    public void ZoomIn()
    {
        _uiScale = Math.Round(Math.Min(ZoomMax, _uiScale + ZoomStep) * 10) / 10;
        ApplyUiScale();
    }

    public void ZoomOut()
    {
        _uiScale = Math.Round(Math.Max(ZoomMin, _uiScale - ZoomStep) * 10) / 10;
        ApplyUiScale();
    }

    public void ResetZoom()
    {
        _uiScale = 1.0;
        ApplyUiScale();
    }

    // Called from SetMode in MainWindow.xaml.cs when newly-visible elements may not
    // have been walked yet (e.g. Settings panel, command panels mounted lazily).
    private void ReapplyUiScaleAfterModeSwitch()
    {
        if (Math.Abs(_uiScale - 1.0) > 0.001)
        {
            ApplyUiScale();
        }
    }

    private void ApplyUiScale()
    {
        if (Content is not UIElement root)
        {
            return;
        }

        ScaleSubtree(root);
    }

    private void ScaleSubtree(DependencyObject node)
    {
        switch (node)
        {
            case TextBlock textBlock:
                if (!_originalTextBlockSizes.TryGetValue(textBlock, out double originalTb))
                {
                    originalTb = textBlock.FontSize;
                    _originalTextBlockSizes[textBlock] = originalTb;
                }
                textBlock.FontSize = Math.Max(8, originalTb * _uiScale);
                break;

            case TextBox textBox:
                if (!_originalTextBoxSizes.TryGetValue(textBox, out double originalTbx))
                {
                    originalTbx = textBox.FontSize;
                    _originalTextBoxSizes[textBox] = originalTbx;
                }
                textBox.FontSize = Math.Max(8, originalTbx * _uiScale);
                break;
        }

        int count = VisualTreeHelper.GetChildrenCount(node);
        for (int i = 0; i < count; i++)
        {
            ScaleSubtree(VisualTreeHelper.GetChild(node, i));
        }
    }
}
