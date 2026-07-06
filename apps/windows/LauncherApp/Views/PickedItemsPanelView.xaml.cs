using System;
using System.Collections.ObjectModel;
using LauncherApp.Core;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace LauncherApp.Views;

// Side panel that replaces the result preview when the user has picked one or more items
// (Ctrl+P). Mirrors macOS PickedItemsPanel in LauncherSubviews.swift: header with running
// count + "Clear all", scrollable list of icon + title + path with a per-row X button.
public sealed partial class PickedItemsPanelView : UserControl
{
    public event EventHandler<string>? RemoveRequested;
    public event EventHandler? ClearAllRequested;

    public ObservableCollection<LauncherRowItem> Items { get; } = new();

    public PickedItemsPanelView()
    {
        InitializeComponent();
        PickedItemsControl.ItemsSource = Items;
        UpdateHeader();
    }

    public void SetItems(System.Collections.Generic.IEnumerable<LauncherRowItem> items)
    {
        Items.Clear();
        foreach (LauncherRowItem item in items)
        {
            Items.Add(item);
            // Picked panel rows render the same icon as the result list - kick off the
            // shared async load so the panel doesn't stay on the fallback glyph.
            _ = item.LoadIconAsync();
        }
        UpdateHeader();
    }

    private void UpdateHeader()
    {
        HeaderText.Text = $"Picked ({Items.Count})";
    }

    private void ClearAllButton_OnClick(object sender, RoutedEventArgs e)
    {
        ClearAllRequested?.Invoke(this, EventArgs.Empty);
    }

    private void RemoveButton_OnClick(object sender, RoutedEventArgs e)
    {
        if (sender is Button btn && btn.Tag is string key && !string.IsNullOrEmpty(key))
        {
            RemoveRequested?.Invoke(this, key);
        }
    }
}
