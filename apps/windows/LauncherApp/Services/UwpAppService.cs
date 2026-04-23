using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Runtime.InteropServices;
using System.Threading.Tasks;
using LauncherApp.Bridge;

namespace LauncherApp.Services;

public sealed class UwpAppService
{
    private readonly object _cacheLock = new();
    private List<LauncherResult> _cache = [];
    private bool _populated;

    public bool IsReady
    {
        get
        {
            lock (_cacheLock)
            {
                return _populated;
            }
        }
    }

    public void BeginInitialize()
    {
        Task.Run(() =>
        {
            try
            {
                var apps = EnumerateAppsFolder();
                lock (_cacheLock)
                {
                    _cache = apps;
                    _populated = true;
                }
                Debug.WriteLine($"[UwpAppService] loaded {apps.Count} apps");
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"[UwpAppService] init failed: {ex.Message}");
                lock (_cacheLock)
                {
                    _populated = true;
                }
            }
        });
    }

    public IReadOnlyList<LauncherResult> Search(string query, int limit)
    {
        List<LauncherResult> snapshot;
        lock (_cacheLock)
        {
            snapshot = _cache;
        }

        if (snapshot.Count == 0)
        {
            return Array.Empty<LauncherResult>();
        }

        if (string.IsNullOrWhiteSpace(query))
        {
            return snapshot.Take(limit).ToList();
        }

        var filtered = new List<(LauncherResult Result, int Score)>();
        foreach (var app in snapshot)
        {
            int score = MatchScore(app.Title, query);
            if (score <= 0)
            {
                continue;
            }

            filtered.Add((
                new LauncherResult
                {
                    Id = app.Id,
                    Kind = app.Kind,
                    Title = app.Title,
                    Subtitle = app.Subtitle,
                    Path = app.Path,
                    Score = score,
                },
                score));
        }

        return filtered
            .OrderByDescending(x => x.Score)
            .Take(limit)
            .Select(x => x.Result)
            .ToList();
    }

    private static int MatchScore(string title, string query)
    {
        if (string.IsNullOrEmpty(title) || string.IsNullOrEmpty(query))
        {
            return 0;
        }

        if (title.Equals(query, StringComparison.OrdinalIgnoreCase))
        {
            return 1500;
        }

        if (title.StartsWith(query, StringComparison.OrdinalIgnoreCase))
        {
            return 1400;
        }

        int idx = title.IndexOf(query, StringComparison.OrdinalIgnoreCase);
        if (idx < 0)
        {
            return 0;
        }

        int wordBoundaryBonus = idx > 0 && !char.IsLetterOrDigit(title[idx - 1]) ? 100 : 0;
        return 1000 + wordBoundaryBonus;
    }

    private static List<LauncherResult> EnumerateAppsFolder()
    {
        var results = new List<LauncherResult>();

        Type? shellType = Type.GetTypeFromProgID("Shell.Application");
        if (shellType is null)
        {
            Debug.WriteLine("[UwpAppService] Shell.Application ProgID not found");
            return results;
        }

        dynamic? shell = null;
        try
        {
            shell = Activator.CreateInstance(shellType);
            if (shell is null)
            {
                return results;
            }

            dynamic appsFolder = shell.NameSpace("shell:AppsFolder");
            dynamic items = appsFolder.Items();
            int count = (int)items.Count;

            var seenAumids = new HashSet<string>(StringComparer.OrdinalIgnoreCase);

            for (int i = 0; i < count; i++)
            {
                try
                {
                    dynamic item = items.Item(i);
                    string? name = item.Name as string;
                    string? path = item.Path as string;

                    if (string.IsNullOrWhiteSpace(name) || string.IsNullOrWhiteSpace(path))
                    {
                        continue;
                    }

                    // AUMIDs contain "!" separating PackageFamilyName from AppId.
                    // Entries without "!" are Win32 shortcuts already indexed by the Rust backend.
                    if (!path.Contains('!'))
                    {
                        continue;
                    }

                    if (!seenAumids.Add(path))
                    {
                        continue;
                    }

                    results.Add(new LauncherResult
                    {
                        Id = "uwp:" + path,
                        Kind = "app",
                        Title = name,
                        Path = "shell:AppsFolder\\" + path,
                        Score = 800,
                    });
                }
                catch (Exception ex)
                {
                    Debug.WriteLine($"[UwpAppService] item {i} failed: {ex.Message}");
                }
            }
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"[UwpAppService] enumeration failed: {ex.Message}");
        }
        finally
        {
            if (shell is not null)
            {
                try
                {
                    Marshal.FinalReleaseComObject(shell);
                }
                catch
                {
                }
            }
        }

        return results;
    }
}
