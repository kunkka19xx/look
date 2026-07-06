using System;
using System.Collections.Generic;
using LauncherApp.Bridge;
using LauncherApp.Core;
using LauncherApp.Services;
using WinRT.Interop;

namespace LauncherApp;

// Clipboard mode wiring: subscribes to the native clipboard listener and turns each captured
// entry into a LauncherResult row. Invoked from MainWindow ctor and consumed by RefreshResults
// when LauncherMode.Clipboard is active.
public sealed partial class MainWindow
{
    private void InitializeClipboardHistory()
    {
        IntPtr hwnd = WindowNative.GetWindowHandle(this);
        _clipboardHistory = new ClipboardHistoryService(hwnd, DispatcherQueue);
        _clipboardHistory.Changed += OnClipboardHistoryChanged;
    }

    private void OnClipboardHistoryChanged(object? sender, EventArgs e)
    {
        if (_mode != LauncherMode.Clipboard)
        {
            return;
        }
        RefreshResults(QueryInput.Text?.Trim() ?? string.Empty);
    }

    private void OnClipboardDeleteRequested(object? sender, string resultId)
    {
        DeleteClipboardEntryByResultId(resultId);
    }

    private bool DeleteClipboardEntryByResultId(string resultId)
    {
        if (_clipboardHistory is null || string.IsNullOrEmpty(resultId))
        {
            return false;
        }

        // LauncherResult.Id for clipboard rows is "clip:<entryId>" - strip the prefix to
        // recover the persistent entry id used by ClipboardHistoryService.
        const string Prefix = "clip:";
        if (!resultId.StartsWith(Prefix, StringComparison.Ordinal))
        {
            return false;
        }
        string entryId = resultId.Substring(Prefix.Length);

        bool removed = _clipboardHistory.RemoveEntry(entryId);
        if (removed)
        {
            ShowBanner("Removed from clipboard history", BannerStyle.Info);
        }
        return removed;
    }

    private List<LauncherResult> BuildClipboardRows()
    {
        if (_clipboardHistory is null)
        {
            return [];
        }

        IReadOnlyList<ClipboardHistoryEntry> entries = _clipboardHistory.Snapshot();
        var rows = new List<LauncherResult>(entries.Count);
        int score = 1000;
        foreach (ClipboardHistoryEntry entry in entries)
        {
            rows.Add(new LauncherResult
            {
                Id = "clip:" + entry.Id,
                Kind = "clipboard",
                Title = BuildClipboardPreview(entry.Content),
                Subtitle = BuildClipboardSubtitle(entry),
                Path = entry.Content,
                Score = score,
            });
            score -= 1;
        }
        return rows;
    }

    private static string BuildClipboardPreview(string content)
    {
        string collapsed = content.Replace('\r', ' ').Replace('\n', ' ').Replace('\t', ' ').Trim();
        if (collapsed.Length <= 120)
        {
            return collapsed;
        }
        return collapsed[..120] + "…";
    }

    private static string BuildClipboardSubtitle(ClipboardHistoryEntry entry)
    {
        string age = HumanAgoLabel(entry.CapturedAt);
        int chars = entry.Content.Length;
        int lines = CountLines(entry.Content);
        string metrics = lines > 1 ? $"{chars} chars · {lines} lines" : $"{chars} chars";
        return $"{age} · {metrics}";
    }

    private static string HumanAgoLabel(DateTimeOffset capturedAt)
    {
        TimeSpan delta = DateTimeOffset.Now - capturedAt;
        if (delta.TotalSeconds < 60) return "Copied just now";
        if (delta.TotalMinutes < 60) return $"Copied {(int)delta.TotalMinutes}m ago";
        if (delta.TotalHours < 24) return $"Copied {(int)delta.TotalHours}h ago";
        return $"Copied {(int)delta.TotalDays}d ago";
    }

    private static int CountLines(string content)
    {
        if (string.IsNullOrEmpty(content))
        {
            return 0;
        }
        int count = 1;
        foreach (char ch in content)
        {
            if (ch == '\n') count++;
        }
        return count;
    }
}
