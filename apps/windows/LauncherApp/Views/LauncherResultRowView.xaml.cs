using System.ComponentModel;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using LauncherApp.Core;

namespace LauncherApp.Views;

public sealed partial class LauncherResultRowView : UserControl
{
    private int _iconLoadVersion;
    private LauncherRowItem? _boundItem;

    public LauncherResultRowView()
    {
        InitializeComponent();
        DataContextChanged += OnDataContextChanged;
        Unloaded += OnUnloaded;
    }

    private async void OnDataContextChanged(FrameworkElement sender, DataContextChangedEventArgs args)
    {
        if (_boundItem is not null)
        {
            _boundItem.PropertyChanged -= OnItemPropertyChanged;
        }

        if (DataContext is not LauncherRowItem item)
        {
            _boundItem = null;
            PickedCheck.Visibility = Visibility.Collapsed;
            return;
        }

        _boundItem = item;
        item.PropertyChanged += OnItemPropertyChanged;
        PickedCheck.Visibility = item.IsPicked ? Visibility.Visible : Visibility.Collapsed;

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

    private void OnItemPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName != nameof(LauncherRowItem.IsPicked) || sender is not LauncherRowItem item)
            return;

        // ListView recycles row containers; PropertyChanged may fire from a non-UI thread
        // path (it doesn't currently, but cheap insurance). DispatcherQueue marshals onto UI.
        DispatcherQueue.TryEnqueue(() =>
        {
            if (_boundItem == item)
            {
                PickedCheck.Visibility = item.IsPicked ? Visibility.Visible : Visibility.Collapsed;
            }
        });
    }

    private void OnUnloaded(object sender, RoutedEventArgs e)
    {
        if (_boundItem is not null)
        {
            _boundItem.PropertyChanged -= OnItemPropertyChanged;
            _boundItem = null;
        }
    }
}
