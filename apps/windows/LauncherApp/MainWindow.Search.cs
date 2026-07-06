using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using LauncherApp.Bridge;
using LauncherApp.Core;
using LauncherApp.Services;

namespace LauncherApp;

// Search pipeline: query routing (ResolveMode), async Rust backend fetch (QueryInput_OnTextChanged),
// rendering (RefreshResults), and the dedup/noise filters that trim Rust results before display.
// UWP entries live in the Rust candidates table (seeded by UwpAppService at startup) so there's no
// separate UWP search path here - the engine ranks them alongside System32 / Start Menu apps.
public sealed partial class MainWindow
{
    private (LauncherMode mode, string normalizedQuery) ResolveMode(string rawQuery)
    {
        string query = rawQuery.Trim();

        if (query.StartsWith("c\"", StringComparison.OrdinalIgnoreCase))
        {
            return (LauncherMode.Clipboard, query.Substring(2).Trim());
        }

        if (query.StartsWith("t\"", StringComparison.OrdinalIgnoreCase))
        {
            return (LauncherMode.Translate, query.Substring(2).Trim());
        }

        if (_mode == LauncherMode.Command)
        {
            return (LauncherMode.Command, query);
        }

        return (LauncherMode.Search, query);
    }

    private async void QueryInput_OnTextChanged(object sender, Microsoft.UI.Xaml.Controls.TextChangedEventArgs e)
    {
        int currentVersion = ++_searchVersion;
        string rawQuery = QueryInput.Text?.Trim() ?? string.Empty;

        // `:cmdid<space>...` jumps straight into command mode with the rest pre-filled as
        // command input. Bare `:cmdid` (no space) is handled on Enter only - see Keyboard.cs.
        // Skipped while already in command mode so editing a command argument that happens to
        // start with `:` (e.g. `:3000` for kill-by-port) doesn't re-trigger.
        if (_mode != LauncherMode.Command
            && TryExtractInlineCommand(rawQuery, out string inlineCommandId, out string inlineArgs, out bool inlineHasSpace)
            && inlineHasSpace)
        {
            EnterCommandScreen(inlineCommandId, inlineArgs);
            return;
        }

        if (rawQuery.StartsWith("t\"", StringComparison.OrdinalIgnoreCase))
        {
            HandleTranslateInputChanged(rawQuery);
            return;
        }

        // Cancel any in-flight translate when leaving translate mode.
        _translateCts?.Cancel();
        _translateCts = null;

        await Task.Delay(16);
        if (currentVersion != _searchVersion)
        {
            return;
        }

        var (resolvedMode, resolvedQuery) = ResolveMode(rawQuery);
        IReadOnlyList<LauncherResult>? backendResults = null;

        if (resolvedMode == LauncherMode.Search)
        {
            try
            {
                string searchQuery = resolvedQuery;
                backendResults = await Task.Run(() => _searchLogic.Search(searchQuery, 120));
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"[MainWindow] backend search failed: {ex.Message}");
            }

            if (currentVersion != _searchVersion)
            {
                return;
            }
        }

        try
        {
            RefreshResults(rawQuery, backendResults);
            UpdateCommandPreview();
        }
        catch (Exception ex)
        {
            _pendingKillTarget = null;
            if (_mode == LauncherMode.Command)
            {
                CommandPanelsPanel.SetExecutionFeedback($"Input update failed: {ex.Message}", isError: true);
            }
        }
    }

    // macOS parity: typing only updates the panel header/hint. Translation fires on Enter only,
    // because each call shells out to curl over the network.
    private void HandleTranslateInputChanged(string rawQuery)
    {
        if (_mode != LauncherMode.Translate)
        {
            SetMode(LauncherMode.Translate);
        }

        // Discard any in-flight translate from the prior query - its results are stale now.
        _translateCts?.Cancel();
        _translateCts = null;

        string text = rawQuery.Substring(2).Trim();
        if (string.IsNullOrEmpty(text))
        {
            TranslatePanel.ShowEmptyPrompt();
        }
        else
        {
            TranslatePanel.ShowReadyPrompt(text);
        }
    }

    private void TranslatePanel_OnOpenInBrowserRequested(object? sender, string text)
    {
        if (string.IsNullOrWhiteSpace(text))
        {
            return;
        }

        string encoded = Uri.EscapeDataString(text);
        string url = $"https://translate.google.com/?sl=auto&tl=en&text={encoded}&op=translate";
        _actionDispatcher.OpenUrl(url);
    }

    private void TranslatePanel_OnCopyTranslatedRequested(object? sender, string text)
    {
        if (string.IsNullOrWhiteSpace(text))
        {
            return;
        }

        try
        {
            Windows.ApplicationModel.DataTransfer.DataPackage package = new();
            package.SetText(text);
            Windows.ApplicationModel.DataTransfer.Clipboard.SetContent(package);
            ShowBanner("Copied translation");
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"[MainWindow] copy translation failed: {ex.Message}");
        }
    }

    public async Task TriggerTranslateFromEnterAsync()
    {
        string rawQuery = QueryInput.Text?.Trim() ?? string.Empty;
        if (!rawQuery.StartsWith("t\"", StringComparison.OrdinalIgnoreCase))
        {
            return;
        }

        string text = rawQuery.Substring(2).Trim();
        if (string.IsNullOrEmpty(text))
        {
            return;
        }

        _translateCts?.Cancel();
        var cts = new CancellationTokenSource();
        _translateCts = cts;
        int currentVersion = _searchVersion;

        TranslatePanel.ShowLoading(text);

        TranslationResultSet results;
        try
        {
            results = await _translationService.TranslateAsync(text, cts.Token);
        }
        catch (OperationCanceledException)
        {
            return;
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"[MainWindow] translate failed: {ex.Message}");
            return;
        }

        if (currentVersion != _searchVersion || cts.Token.IsCancellationRequested || _mode != LauncherMode.Translate)
        {
            return;
        }

        TranslatePanel.ShowResults(results);
    }

    private void RefreshResults(string rawQuery, IReadOnlyList<LauncherResult>? prefetchedBackendResults = null)
    {
        var (resolvedMode, query) = ResolveMode(rawQuery);
        if (resolvedMode != _mode)
        {
            SetMode(resolvedMode);
        }

        if (_mode == LauncherMode.Command)
        {
            query = CommandPanelsPanel.CommandInputText.Trim();
        }

        IReadOnlyList<LauncherResult> source = _mode switch
        {
            LauncherMode.Search => FilterSearchNoise(
                DeduplicatePairedAppEntries(prefetchedBackendResults ?? _searchLogic.Search(query, 120)), 40),
            LauncherMode.Command => FilterRows(_commandSeed, query),
            LauncherMode.Clipboard => FilterRows(BuildClipboardRows(), query),
            LauncherMode.Settings => [],
            LauncherMode.Help => FilterRows(BuildHelpRows(), query),
            _ => [],
        };

        _results.Clear();
        foreach (LauncherResult item in source)
        {
            _results.Add(new LauncherRowItem(item));
        }

        // After repopulating the row list, reconcile the IsPicked flag so newly-rendered
        // rows that are part of the session pick set show the checkmark glyph.
        SyncPickedRowItemsForVisibleResults();

        if (_mode == LauncherMode.Command)
        {
            try
            {
                CommandPanelsPanel.ApplyFilter(string.Empty);
                CommandPanelsPanel.SelectPanel(ResolveCommandId(CommandPanelsPanel.ActiveCommandId));
            }
            catch
            {
                CommandPanelsPanel.SelectPanel("command:calc");
            }
        }

        if (_results.Count > 0)
        {
            if (_mode != LauncherMode.Command)
            {
                ResultsList.SelectedIndex = 0;
            }
            return;
        }

        ResultPreviewPanel.Visibility = Microsoft.UI.Xaml.Visibility.Collapsed;
        PreviewDivider.Visibility = Microsoft.UI.Xaml.Visibility.Collapsed;
    }

    private static IReadOnlyList<LauncherResult> FilterRows(IEnumerable<LauncherResult> source, string query)
    {
        if (string.IsNullOrWhiteSpace(query))
        {
            return source.OrderByDescending(item => item.Score).ToList();
        }

        return source.Where(item =>
                item.Title.Contains(query, StringComparison.OrdinalIgnoreCase)
                || item.Path.Contains(query, StringComparison.OrdinalIgnoreCase)
                || (item.Subtitle?.Contains(query, StringComparison.OrdinalIgnoreCase) ?? false))
            .OrderByDescending(item => item.Score)
            .ToList();
    }

    private static IReadOnlyList<LauncherResult> FilterSearchNoise(IReadOnlyList<LauncherResult> source, int limit)
    {
        return source
            .Where(item => !ShouldHideSearchResult(item))
            .Take(limit)
            .ToList();
    }

    private static IReadOnlyList<LauncherResult> DeduplicatePairedAppEntries(IReadOnlyList<LauncherResult> source)
    {
        var output = new List<LauncherResult>(source.Count);
        var representativeIndexByKey = new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase);

        foreach (var item in source)
        {
            var category = GetAppPathCategory(item);
            if (category == AppPathCategory.Other)
            {
                output.Add(item);
                continue;
            }

            string key = NormalizeAppIdentity(item.Title);
            if (string.IsNullOrEmpty(key))
            {
                output.Add(item);
                continue;
            }

            if (!representativeIndexByKey.TryGetValue(key, out int existingIndex))
            {
                representativeIndexByKey[key] = output.Count;
                output.Add(item);
                continue;
            }

            var existing = output[existingIndex];
            var existingCategory = GetAppPathCategory(existing);

            if (!ArePairedDuplicateCategories(existingCategory, category))
            {
                output.Add(item);
                continue;
            }

            if (IsPreferredAppEntry(item, existing))
                output[existingIndex] = item;
        }

        return output;
    }

    private static bool ArePairedDuplicateCategories(AppPathCategory a, AppPathCategory b)
    {
        if (a == AppPathCategory.Other || b == AppPathCategory.Other)
        {
            return false;
        }
        return a != b;
    }

    private static bool IsPreferredAppEntry(LauncherResult candidate, LauncherResult existing)
    {
        int candidateRank = GetAppEntryRank(candidate);
        int existingRank = GetAppEntryRank(existing);
        if (candidateRank != existingRank)
            return candidateRank > existingRank;

        return candidate.Score > existing.Score;
    }

    private static int GetAppEntryRank(LauncherResult item)
    {
        return GetAppPathCategory(item) switch
        {
            AppPathCategory.UwpAppsFolder => 3,
            AppPathCategory.InstallExecutable => 2,
            AppPathCategory.StartMenuShortcut => 1,
            AppPathCategory.SystemExecutable => 0,
            _ => -1,
        };
    }

    private static AppPathCategory GetAppPathCategory(LauncherResult item)
    {
        if (!item.Kind.Equals("app", StringComparison.OrdinalIgnoreCase)
            || string.IsNullOrWhiteSpace(item.Path))
        {
            return AppPathCategory.Other;
        }

        if (item.Path.StartsWith("shell:AppsFolder\\", StringComparison.OrdinalIgnoreCase))
        {
            return AppPathCategory.UwpAppsFolder;
        }

        string path = item.Path.Replace('/', '\\');
        if (path.EndsWith(".lnk", StringComparison.OrdinalIgnoreCase)
            && path.Contains("\\Start Menu\\Programs\\", StringComparison.OrdinalIgnoreCase))
        {
            return AppPathCategory.StartMenuShortcut;
        }

        if (path.EndsWith(".exe", StringComparison.OrdinalIgnoreCase)
            && (path.Contains("\\Program Files\\", StringComparison.OrdinalIgnoreCase)
                || path.Contains("\\Program Files (x86)\\", StringComparison.OrdinalIgnoreCase)
                || path.Contains("\\AppData\\Local\\Programs\\", StringComparison.OrdinalIgnoreCase)))
        {
            return AppPathCategory.InstallExecutable;
        }

        if (path.EndsWith(".exe", StringComparison.OrdinalIgnoreCase)
            && IsWindowsSystemPath(path))
        {
            return AppPathCategory.SystemExecutable;
        }

        return AppPathCategory.Other;
    }

    private static bool IsWindowsSystemPath(string path)
    {
        // Match any of the well-known OS executable roots so they participate in dedup with
        // Start Menu / UWP entries of the same name (e.g., Windows\System32\notepad.exe vs
        // AppsFolder\Notepad). Case-insensitive; tolerates drive letters via Contains.
        return path.Contains("\\Windows\\System32\\", StringComparison.OrdinalIgnoreCase)
            || path.Contains("\\Windows\\SysWOW64\\", StringComparison.OrdinalIgnoreCase)
            || path.Contains("\\Windows\\SysNative\\", StringComparison.OrdinalIgnoreCase);
    }

    private static string NormalizeAppIdentity(string title)
    {
        if (string.IsNullOrWhiteSpace(title))
            return string.Empty;

        return new string(title
            .ToLowerInvariant()
            .Where(char.IsLetterOrDigit)
            .ToArray());
    }

    private static bool ShouldHideSearchResult(LauncherResult item)
    {
        if (!item.Kind.Equals("app", StringComparison.OrdinalIgnoreCase)
            && !item.Kind.Equals("file", StringComparison.OrdinalIgnoreCase))
        {
            return false;
        }

        if (string.IsNullOrWhiteSpace(item.Path))
            return false;

        string path = item.Path.Replace('/', '\\');

        foreach (string segment in JunkPathSegments)
        {
            if (path.Contains(segment, StringComparison.OrdinalIgnoreCase))
                return true;
        }

        if (!path.EndsWith(".exe", StringComparison.OrdinalIgnoreCase))
            return false;

        string fileName = Path.GetFileNameWithoutExtension(path);
        if (string.IsNullOrWhiteSpace(fileName))
            return false;

        string normalizedName = fileName.ToLowerInvariant();
        return NoisyExecutableNameTokens.Any(token => normalizedName.Contains(token, StringComparison.Ordinal));
    }

    private enum AppPathCategory
    {
        Other,
        SystemExecutable,
        StartMenuShortcut,
        InstallExecutable,
        UwpAppsFolder,
    }

    private static IReadOnlyList<LauncherResult> BuildHelpRows()
    {
        return
        [
            new LauncherResult { Id = "help:search", Kind = "app", Title = "Search mode", Subtitle = "Blend apps, files, folders", Path = "help://search", Score = 1000 },
            new LauncherResult { Id = "help:command", Kind = "app", Title = "Command mode", Subtitle = "calc, shell, kill, sys", Path = "help://command", Score = 990 },
            new LauncherResult { Id = "help:clipboard", Kind = "clipboard", Title = "Clipboard mode", Subtitle = "session-local history", Path = "help://clipboard", Score = 980 },
        ];
    }
}
