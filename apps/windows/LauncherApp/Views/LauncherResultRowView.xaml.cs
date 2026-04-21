using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using LauncherApp.Core;

namespace LauncherApp.Views;

public sealed partial class LauncherResultRowView : UserControl
{
    private int _iconLoadVersion;

    public LauncherResultRowView()
    {
        InitializeComponent();
        DataContextChanged += OnDataContextChanged;
    }

    private async void OnDataContextChanged(FrameworkElement sender, DataContextChangedEventArgs args)
    {
        if (DataContext is not LauncherRowItem item)
            return;

        int loadVersion = ++_iconLoadVersion;

        IconImage.Source = null;
        IconImage.Visibility = Visibility.Collapsed;
        IconGlyph.Visibility = Visibility.Visible;
        IconGlyph.Text = item.IconGlyph;

        await item.LoadIconAsync();

        if (loadVersion != _iconLoadVersion)
            return;

        if (item.Icon is { } iconImage)
        {
            IconImage.Source = iconImage;
            IconImage.Visibility = Visibility.Visible;
            IconGlyph.Visibility = Visibility.Collapsed;
        }
    }
}
