using System;
using System.Collections.Generic;
using LauncherApp.Bridge;

namespace LauncherApp.Features.Search;

public sealed class LauncherSearchLogic
{
    private readonly ISearchProvider _searchProvider;

    public LauncherSearchLogic(ISearchProvider searchProvider)
    {
        _searchProvider = searchProvider;
    }

    public IReadOnlyList<LauncherResult> Search(string query, int limit = 40)
    {
        IReadOnlyList<LauncherResult> backend = _searchProvider.Search(query, limit);

        IReadOnlyList<LauncherResult> quickFolders = BuildQuickFolderResults(query);
        if (quickFolders.Count == 0)
        {
            return backend;
        }

        var combined = new List<LauncherResult>(quickFolders.Count + backend.Count);
        var seenPaths = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (LauncherResult entry in quickFolders)
        {
            if (seenPaths.Add(entry.Path))
            {
                combined.Add(entry);
            }
        }

        // Engine indexing typically skips the file_scan_roots themselves (see
        // core/engine/src/index/files.rs walk_files), so duplicates are unlikely — but
        // belt-and-braces drop any backend folder row that already matches a pinned path
        // so we don't render two rows for the same `~/Documents`.
        foreach (LauncherResult entry in backend)
        {
            if (entry.Kind == "folder" && seenPaths.Contains(entry.Path))
            {
                continue;
            }
            combined.Add(entry);
        }

        return combined;
    }

    // Mirrors apps/macos/.../LauncherView.swift:quickFolderPinnedResults. The macOS view
    // gates this on a `pinnedLookupScope` derived from query prefixes (`d"`, `f"`, `a"`)
    // — Windows doesn't expose those prefixes, so a bare substring/prefix match against
    // the entry titles is enough.
    public static IReadOnlyList<LauncherResult> BuildQuickFolderResults(string rawQuery)
    {
        string normalized = (rawQuery ?? string.Empty).Trim().ToLowerInvariant();
        if (normalized.Length == 0)
        {
            return Array.Empty<LauncherResult>();
        }

        var matches = new List<LauncherResult>();
        foreach (QuickFolderCatalog.QuickFolderEntry entry in QuickFolderCatalog.Entries)
        {
            string titleLower = entry.Title.ToLowerInvariant();
            bool isMatch = titleLower.Contains(normalized)
                || (titleLower.StartsWith(normalized)
                    && normalized.Length >= QuickFolderCatalog.MinPrefixMatchLength);
            if (!isMatch)
            {
                continue;
            }

            matches.Add(new LauncherResult
            {
                Id = QuickFolderCatalog.IdPrefix + titleLower,
                Kind = "folder",
                Title = entry.Title,
                Subtitle = QuickFolderCatalog.PinnedSubtitle,
                Path = entry.Path,
                Score = QuickFolderCatalog.PinnedScore,
            });
        }

        return matches;
    }
}
