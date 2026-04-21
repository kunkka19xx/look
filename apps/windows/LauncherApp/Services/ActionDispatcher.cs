using System;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
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

    public bool OpenResult(LauncherResult result, bool forceNewWindow = false)
    {
        var kind = ResolveResultKind(result);
        Log($"Dispatch open: kind={kind} forceNewWindow={forceNewWindow} id='{result.Id}' title='{result.Title}' path='{result.Path}'");

        if (!forceNewWindow && kind == LauncherActionKind.App && TryActivateExistingAppWindow(result.Path, result.Title))
        {
            Log("Dispatch activate-existing succeeded");
            return true;
        }

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

    private static bool TryActivateExistingAppWindow(string path, string? title)
    {
        if (string.IsNullOrWhiteSpace(path))
            return false;

        string resolved = ResolveExecutablePath(path);
        string normalizedPath = NormalizePath(resolved);
        if (!normalizedPath.EndsWith(".exe", StringComparison.OrdinalIgnoreCase))
            return false;

        string processName = Path.GetFileNameWithoutExtension(normalizedPath);
        if (string.IsNullOrWhiteSpace(processName))
            return false;

        Process[] candidates;
        try
        {
            candidates = Process.GetProcessesByName(processName);
        }
        catch
        {
            return false;
        }

        IntPtr fallbackWindow = IntPtr.Zero;

        foreach (var process in candidates)
        {
            try
            {
                IntPtr hwnd = process.MainWindowHandle;
                if (hwnd == IntPtr.Zero)
                    continue;

                if (fallbackWindow == IntPtr.Zero)
                    fallbackWindow = hwnd;

                string? processPath = process.MainModule?.FileName;
                if (string.IsNullOrWhiteSpace(processPath))
                    continue;

                if (!NormalizePath(processPath).Equals(normalizedPath, StringComparison.OrdinalIgnoreCase))
                    continue;

                ShowWindowAsync(hwnd, SW_RESTORE);
                return SetForegroundWindow(hwnd);
            }
            catch
            {
            }
        }

        if (fallbackWindow != IntPtr.Zero && IsLikelySingleAppAlias(path, normalizedPath, title))
        {
            ShowWindowAsync(fallbackWindow, SW_RESTORE);
            return SetForegroundWindow(fallbackWindow);
        }

        return false;
    }

    private static string ResolveExecutablePath(string path)
    {
        string normalized = NormalizePath(path);
        if (!normalized.EndsWith(".lnk", StringComparison.OrdinalIgnoreCase))
            return normalized;

        try
        {
            var shellType = Type.GetTypeFromProgID("WScript.Shell");
            if (shellType == null)
                return normalized;

            dynamic? shell = Activator.CreateInstance(shellType);
            if (shell == null)
                return normalized;

            dynamic shortcut = shell.CreateShortcut(normalized);
            string targetPath = shortcut.TargetPath;
            if (!string.IsNullOrWhiteSpace(targetPath))
                return NormalizePath(targetPath);
        }
        catch
        {
        }

        return normalized;
    }

    private static bool IsLikelySingleAppAlias(string originalPath, string resolvedExePath, string? title)
    {
        string resolvedFile = Path.GetFileName(resolvedExePath);
        if (string.IsNullOrWhiteSpace(resolvedFile))
            return false;

        if (originalPath.Contains("WindowsApps", StringComparison.OrdinalIgnoreCase)
            || resolvedExePath.Contains("WindowsApps", StringComparison.OrdinalIgnoreCase)
            || resolvedExePath.Contains("\\Windows\\System32\\", StringComparison.OrdinalIgnoreCase))
        {
            return true;
        }

        return !string.IsNullOrWhiteSpace(title)
            && title.Equals(Path.GetFileNameWithoutExtension(resolvedExePath), StringComparison.OrdinalIgnoreCase);
    }

    private static string NormalizePath(string path)
    {
        var normalized = path.Replace('/', '\\').Trim();
        if (normalized.EndsWith("\\") && normalized.Length > 3)
            normalized = normalized.TrimEnd('\\');
        return normalized;
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

    private const int SW_RESTORE = 9;

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool SetForegroundWindow(IntPtr hWnd);

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool ShowWindowAsync(IntPtr hWnd, int nCmdShow);
}
