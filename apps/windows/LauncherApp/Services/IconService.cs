using System;
using System.Collections.Concurrent;
using System.Threading.Tasks;
using Microsoft.UI.Xaml.Media;

namespace LauncherApp.Services;

public enum SearchItemKind
{
    Unknown,
    App,
    Setting,
    File,
    Folder,
    Command,
    Calculator,
}

public interface IIconService
{
    Task<ImageSource?> GetIconAsync(string? path, SearchItemKind kind = SearchItemKind.Unknown);
    void ClearCache();
}

public sealed class IconService : IIconService
{
    private readonly ShellIconProvider _shellIconProvider = new();
    private readonly ConcurrentDictionary<string, Task<ImageSource?>> _cache = new(StringComparer.OrdinalIgnoreCase);

    public Task<ImageSource?> GetIconAsync(string? path, SearchItemKind kind = SearchItemKind.Unknown)
    {
        if (string.IsNullOrWhiteSpace(path))
            return Task.FromResult<ImageSource?>(null);

        var normalizedPath = NormalizePath(path);
        return _cache.GetOrAdd(normalizedPath, _ => LoadCoreAsync(normalizedPath));
    }

    public void ClearCache() => _cache.Clear();

    private async Task<ImageSource?> LoadCoreAsync(string path)
    {
        try
        {
            var shellIcon = await _shellIconProvider.GetIconAsync(path, smallIcon: false);
            if (shellIcon != null)
                return shellIcon;

            return await _shellIconProvider.GetIconAsync(path, smallIcon: true);
        }
        catch
        {
            return null;
        }
    }

    private static string NormalizePath(string path)
    {
        var normalized = path.Trim().Replace('/', '\\');
        if (normalized.EndsWith("\\") && normalized.Length > 3)
            normalized = normalized.TrimEnd('\\');
        return normalized;
    }
}
