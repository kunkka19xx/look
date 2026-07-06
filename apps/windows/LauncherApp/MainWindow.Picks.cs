using System;
using System.Collections.Generic;
using System.Threading.Tasks;
using LauncherApp.Bridge;
using LauncherApp.Core;
using LauncherApp.Services;
using Microsoft.UI.Xaml;
using Windows.ApplicationModel.DataTransfer;

namespace LauncherApp;

// Multi-pick (Ctrl+P toggle, Ctrl+Shift+P clear). Mirrors macOS togglePickForSelectedResult /
// writePickedToPasteboard / clearAllPicked in LauncherView+Results.swift. Picks are session-
// scoped and survive query changes. Each toggle rewrites the clipboard with the full picked
// set as IStorageItem entries (so Ctrl+V in Explorer pastes the files) plus a newline-joined
// text fallback. Only file/folder kinds are pickable; settings/url/UWP-shell rows show an
// info banner instead.
public sealed partial class MainWindow
{
    private static string PickedKey(LauncherResult result) => $"{result.Kind}|{result.Path}";

    private async Task TogglePickForSelectedRowAsync()
    {
        if (_mode != LauncherMode.Search)
        {
            return;
        }

        if (ResultsList.SelectedItem is not LauncherRowItem row)
        {
            return;
        }

        if (row.Kind is not (SearchItemKind.File or SearchItemKind.Folder))
        {
            ShowBanner("Only files or folders can be picked", BannerStyle.Info);
            return;
        }

        string key = PickedKey(row.Result);
        int idx = _pickedKeys.IndexOf(key);
        if (idx >= 0)
        {
            _pickedKeys.RemoveAt(idx);
            _pickedResultsByKey.Remove(key);
            row.IsPicked = false;
        }
        else
        {
            _pickedKeys.Add(key);
            _pickedResultsByKey[key] = row.Result;
            row.IsPicked = true;
        }

        SyncPickedRowItemsForVisibleResults();
        RefreshPickedSidePanel();
        await WritePickedToClipboardAsync();
    }

    private void ClearPicks()
    {
        if (_pickedKeys.Count == 0)
        {
            return;
        }

        _pickedKeys.Clear();
        _pickedResultsByKey.Clear();
        // Clear the system clipboard too - picks are the only thing we put there via this
        // flow, so leaving stale picks on the clipboard after the user explicitly cleared
        // would be confusing. Matches macOS clearAllPicked which also clears NSPasteboard.
        try
        {
            Clipboard.Clear();
        }
        catch
        {
        }
        SyncPickedRowItemsForVisibleResults();
        RefreshPickedSidePanel();
        ShowBanner("Cleared picked items", BannerStyle.Info);
    }

    // Removal from the side panel X button. Same semantics as toggling off via Ctrl+P.
    private async void OnPickedPanelRemoveRequested(object? sender, string key)
    {
        if (string.IsNullOrEmpty(key))
        {
            return;
        }

        int idx = _pickedKeys.IndexOf(key);
        if (idx < 0)
        {
            return;
        }

        _pickedKeys.RemoveAt(idx);
        _pickedResultsByKey.Remove(key);

        SyncPickedRowItemsForVisibleResults();
        RefreshPickedSidePanel();
        await WritePickedToClipboardAsync();
    }

    private void OnPickedPanelClearAllRequested(object? sender, EventArgs e)
    {
        ClearPicks();
    }

    // Iterate currently-rendered result rows and reconcile their IsPicked flag with the
    // session pick set. Called after every pick mutation (toggle / remove / clear) and from
    // RefreshResults when the search list is rebuilt for a new query.
    internal void SyncPickedRowItemsForVisibleResults()
    {
        foreach (LauncherRowItem row in _results)
        {
            row.IsPicked = _pickedKeys.Contains(PickedKey(row.Result));
        }
    }

    // Rebuild the side-panel list and toggle which side panel is visible. When picks exist
    // the picked panel takes the right column slot and the standard preview is hidden;
    // mirrors the macOS HStack branch in LauncherView.swift:447.
    internal void RefreshPickedSidePanel()
    {
        if (_pickedKeys.Count == 0)
        {
            PickedItemsPanel.Visibility = Visibility.Collapsed;
            PickedItemsPanel.SetItems(Array.Empty<LauncherRowItem>());

            // Restore the preview panel for the current selection (only in modes that show it).
            // Help mode covers the entire ResultsHost grid (Grid.ColumnSpan="3" in MainWindow.xaml)
            // so showing the preview here paints it on top of the help screen - exclude it.
            if ((_mode == LauncherMode.Search || _mode == LauncherMode.Clipboard)
                && ResultsList.SelectedItem is LauncherRowItem selectedRow)
            {
                ResultPreviewPanel.SetRow(selectedRow);
                ResultPreviewPanel.Visibility = Visibility.Visible;
                PreviewDivider.Visibility = Visibility.Visible;
            }
            else
            {
                PreviewDivider.Visibility = Visibility.Collapsed;
            }
            return;
        }

        List<LauncherRowItem> ordered = new(_pickedKeys.Count);
        foreach (string key in _pickedKeys)
        {
            if (_pickedResultsByKey.TryGetValue(key, out LauncherResult? r) && r is not null)
            {
                LauncherRowItem item = new(r) { IsPicked = true };
                ordered.Add(item);
            }
        }

        PickedItemsPanel.SetItems(ordered);
        ResultPreviewPanel.Visibility = Visibility.Collapsed;
        PickedItemsPanel.Visibility = Visibility.Visible;
        PreviewDivider.Visibility = Visibility.Visible;
    }

    private async Task WritePickedToClipboardAsync()
    {
        if (_pickedKeys.Count == 0)
        {
            // Last pick was just toggled off - clear clipboard so we don't leave the previous
            // (now-stale) pick set behind.
            try
            {
                Clipboard.Clear();
            }
            catch
            {
            }
            ShowBanner("Cleared picked items", BannerStyle.Info);
            return;
        }

        List<LauncherResult> ordered = new(_pickedKeys.Count);
        foreach (string key in _pickedKeys)
        {
            if (_pickedResultsByKey.TryGetValue(key, out LauncherResult? r) && r is not null)
            {
                ordered.Add(r);
            }
        }

        bool ok = await _actionDispatcher.CopyResultsAsync(ordered);
        if (ok)
        {
            ShowBanner($"Picked {_pickedKeys.Count} item(s)", BannerStyle.Success);
        }
        else
        {
            ShowBanner("Pick failed", BannerStyle.Error);
        }
    }
}
