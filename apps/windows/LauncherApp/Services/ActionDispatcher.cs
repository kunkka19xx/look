using System;
using System.IO;
using LauncherApp.Bridge;
using Windows.ApplicationModel.DataTransfer;

namespace LauncherApp.Services;

public sealed class ActionDispatcher
{
    private static readonly string LogPath = Path.Combine(Path.GetTempPath(), "look-open.log");

    private readonly ShellExecuteService _shellExecute;
    private readonly ExplorerRevealService _reveal;

    public ActionDispatcher(ShellExecuteService shellExecute, ExplorerRevealService reveal)
    {
        _shellExecute = shellExecute;
        _reveal = reveal;
    }

    public bool OpenResult(LauncherResult result)
    {
        var kind = ResolveResultKind(result);
        Log($"Dispatch open: kind={kind} id='{result.Id}' title='{result.Title}' path='{result.Path}'");
        bool opened = kind switch
        {
            LauncherActionKind.Setting => OpenSetting(result.Path),
            LauncherActionKind.App => _shellExecute.Open(result.Path),
            LauncherActionKind.File => _shellExecute.Open(result.Path),
            LauncherActionKind.Folder => _shellExecute.Open(result.Path),
            LauncherActionKind.Url => _shellExecute.Open(result.Path),
            _ => false,
        };

        if (opened)
            return true;

        bool fallback = _shellExecute.Open(result.Path);
        Log($"Dispatch fallback open result={fallback}");
        return fallback;
    }

    public bool RevealResult(LauncherResult result)
    {
        var kind = ResolveResultKind(result);
        if (kind is LauncherActionKind.Setting or LauncherActionKind.Url or LauncherActionKind.Unknown)
            return false;

        return _reveal.Reveal(result.Path);
    }

    public bool CopyResultPath(LauncherResult result)
    {
        if (string.IsNullOrWhiteSpace(result.Path))
        {
            return false;
        }

        DataPackage package = new();
        package.SetText(result.Path);
        Clipboard.SetContent(package);
        return true;
    }

    public bool WebHandoff(string query)
    {
        if (string.IsNullOrWhiteSpace(query))
        {
            return false;
        }

        string url = "https://www.google.com/search?q=" + Uri.EscapeDataString(query);
        return _shellExecute.Open(url);
    }

    private static LauncherActionKind ResolveResultKind(LauncherResult result)
    {
        if (result.Path.StartsWith("ms-settings:", StringComparison.OrdinalIgnoreCase)
            || result.Id.StartsWith("setting:", StringComparison.OrdinalIgnoreCase)
            || result.Kind.Equals("setting", StringComparison.OrdinalIgnoreCase))
        {
            return LauncherActionKind.Setting;
        }

        if (result.Kind.Equals("folder", StringComparison.OrdinalIgnoreCase))
            return LauncherActionKind.Folder;

        if (result.Kind.Equals("file", StringComparison.OrdinalIgnoreCase))
            return LauncherActionKind.File;

        if (result.Kind.Equals("app", StringComparison.OrdinalIgnoreCase))
            return LauncherActionKind.App;

        if (Uri.TryCreate(result.Path, UriKind.Absolute, out var uri)
            && (uri.Scheme.Equals(Uri.UriSchemeHttp, StringComparison.OrdinalIgnoreCase)
                || uri.Scheme.Equals(Uri.UriSchemeHttps, StringComparison.OrdinalIgnoreCase)))
        {
            return LauncherActionKind.Url;
        }

        if (Directory.Exists(result.Path))
            return LauncherActionKind.Folder;

        if (File.Exists(result.Path))
        {
            string ext = Path.GetExtension(result.Path);
            if (ext.Equals(".exe", StringComparison.OrdinalIgnoreCase)
                || ext.Equals(".lnk", StringComparison.OrdinalIgnoreCase)
                || ext.Equals(".url", StringComparison.OrdinalIgnoreCase))
            {
                return LauncherActionKind.App;
            }

            return LauncherActionKind.File;
        }

        return LauncherActionKind.Unknown;
    }

    private bool OpenSetting(string path)
    {
        if (string.IsNullOrWhiteSpace(path))
            return false;

        if (_shellExecute.Open(path))
            return true;

        return _shellExecute.Open("explorer.exe", path);
    }

    private static void Log(string message)
    {
        try
        {
            File.AppendAllText(LogPath, $"[{DateTime.Now:HH:mm:ss.fff}] {message}{Environment.NewLine}");
        }
        catch
        {
        }
    }

    private enum LauncherActionKind
    {
        Unknown,
        App,
        File,
        Folder,
        Setting,
        Url,
    }
}
