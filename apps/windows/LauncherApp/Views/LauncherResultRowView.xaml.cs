using System;
using System.Threading.Tasks;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using LauncherApp.Core;

namespace LauncherApp.Views;

public sealed partial class LauncherResultRowView : UserControl
{
    private bool _iconLoaded;

    public LauncherResultRowView()
    {
        InitializeComponent();
        DataContextChanged += OnDataContextChanged;
    }

    private void OnDataContextChanged(FrameworkElement sender, DataContextChangedEventArgs args)
    {
        if (DataContext is not LauncherRowItem item)
            return;

        if (_iconLoaded)
            return;

        _iconLoaded = true;

        if (item.Icon is { } iconImage)
        {
            IconImage.Source = iconImage;
            IconImage.Visibility = Visibility.Visible;
            IconGlyph.Visibility = Visibility.Collapsed;
        }
        else
        {
            IconGlyph.Text = item.IconGlyph;
        }
    }
}