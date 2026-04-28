using System;
using System.Collections.Generic;
using System.Linq;
using LauncherApp.Bridge;

namespace LauncherApp.Features.Search;

public sealed class MockSearchProvider : ISearchProvider
{
    private static readonly IReadOnlyList<LauncherResult> Seed =
    [
        new LauncherResult { Id = "app:vscode", Kind = "app", Title = "Visual Studio Code", Subtitle = "App", Path = @"C:\Users\demo\AppData\Local\Programs\Microsoft VS Code\Code.exe", Score = 980 },
        new LauncherResult { Id = "app:terminal", Kind = "app", Title = "Windows Terminal", Subtitle = "App", Path = @"C:\Program Files\WindowsApps\Microsoft.WindowsTerminal_8wekyb3d8bbwe\WindowsTerminal.exe", Score = 960 },
        new LauncherResult { Id = "folder:downloads", Kind = "folder", Title = "Downloads", Subtitle = "Folder", Path = @"C:\Users\demo\Downloads", Score = 920 },
        new LauncherResult { Id = "file:readme", Kind = "file", Title = "README.md", Subtitle = "File", Path = @"C:\Users\demo\Documents\look\README.md", Score = 900 },
        new LauncherResult { Id = "setting:display", Kind = "app", Title = "Display settings", Subtitle = "Windows Settings", Path = "ms-settings:display", Score = 890 },
        new LauncherResult { Id = "app:explorer", Kind = "app", Title = "File Explorer", Subtitle = "App", Path = @"C:\Windows\explorer.exe", Score = 860 },
    ];

    public IReadOnlyList<LauncherResult> Search(string query, int limit)
    {
        string normalized = query.Trim();
        IEnumerable<LauncherResult> source = Seed;

        if (!string.IsNullOrWhiteSpace(normalized))
        {
            string lower = normalized.ToLowerInvariant();
            source = source.Where(item =>
                item.Title.Contains(lower, StringComparison.OrdinalIgnoreCase)
                || item.Path.Contains(lower, StringComparison.OrdinalIgnoreCase)
                || (item.Subtitle?.Contains(lower, StringComparison.OrdinalIgnoreCase) ?? false));
        }

        return source
            .OrderByDescending(item => item.Score)
            .Take(Math.Max(1, limit))
            .ToList();
    }
}
