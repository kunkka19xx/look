using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Threading.Tasks;
using LauncherApp.Bridge;
using Windows.ApplicationModel.DataTransfer;
using Windows.Storage;

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
            RecordUsage(result, kind);
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
        {
            RecordUsage(result, kind);
            return true;
        }

        bool fallback = _shellExecute.Open(result.Path);
        Log($"Dispatch fallback open result={fallback}");
        if (fallback)
        {
            RecordUsage(result, kind);
        }
        return fallback;
    }

    public bool RevealResult(LauncherResult result)
    {
        var kind = ResolveResultKind(result);
        if (kind is LauncherActionKind.Setting or LauncherActionKind.Url or LauncherActionKind.Unknown)
            return false;

        return _reveal.Reveal(result.Path);
    }

    // Single-row Ctrl+C. For file/folder kinds we attach a real IStorageItem so Ctrl+V in
    // Explorer pastes the file (parity with macOS pasteboard.writeObjects([NSURL, NSString])
    // in LauncherView+Results.swift:164). Other kinds (settings, urls, UWP shell: targets)
    // fall back to text-only since they have no filesystem path to attach.
    public async Task<bool> CopyResultAsync(LauncherResult result)
    {
        if (string.IsNullOrWhiteSpace(result.Path))
        {
            return false;
        }

        DataPackage package = new();
        package.SetText(result.Path);

        IStorageItem? storageItem = await TryGetStorageItemAsync(result);
        if (storageItem is not null)
        {
            package.SetStorageItems(new[] { storageItem });
        }

        Clipboard.SetContent(package);
        Clipboard.Flush();
        return true;
    }

    // Multi-pick write. Mirrors macOS writePickedToPasteboard (LauncherView+Results.swift:134):
    // attach every resolvable file/folder as IStorageItem so Explorer paste copies them all,
    // and join paths with newlines for the text fallback (paste-into-text-field).
    public async Task<bool> CopyResultsAsync(IReadOnlyList<LauncherResult> results)
    {
        if (results is null || results.Count == 0)
        {
            return false;
        }

        List<IStorageItem> storageItems = new();
        List<string> paths = new();

        foreach (LauncherResult result in results)
        {
            if (string.IsNullOrWhiteSpace(result.Path))
                continue;

            paths.Add(result.Path);

            IStorageItem? item = await TryGetStorageItemAsync(result);
            if (item is not null)
            {
                storageItems.Add(item);
            }
        }

        if (paths.Count == 0)
        {
            return false;
        }

        DataPackage package = new();
        package.SetText(string.Join(Environment.NewLine, paths));

        if (storageItems.Count > 0)
        {
            package.SetStorageItems(storageItems);
        }

        Clipboard.SetContent(package);
        Clipboard.Flush();
        return true;
    }

    private static async Task<IStorageItem?> TryGetStorageItemAsync(LauncherResult result)
    {
        var kind = ResolveResultKind(result);
        if (kind != LauncherActionKind.File && kind != LauncherActionKind.Folder)
        {
            return null;
        }

        try
        {
            if (kind == LauncherActionKind.Folder)
            {
                return await StorageFolder.GetFolderFromPathAsync(result.Path);
            }
            return await StorageFile.GetFileFromPathAsync(result.Path);
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"[ActionDispatcher] storage item resolve failed for '{result.Path}': {ex.Message}");
            return null;
        }
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

    public bool OpenUrl(string url)
    {
        if (string.IsNullOrWhiteSpace(url))
        {
            return false;
        }
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

    // Records a successful launch so the Rust engine's rank_score (core/ranking/src/lib.rs)
    // sees the use_count bump. UWP entries are seeded into sqlite as `app:uwp:<AUMID>` by
    // UwpAppService on startup, so they go through this same FFI path with no special-casing.
    private static void RecordUsage(LauncherResult result, LauncherActionKind kind)
    {
        if (string.IsNullOrWhiteSpace(result.Id))
        {
            Log("Dispatch record_usage skipped: empty id");
            return;
        }

        string? action = kind switch
        {
            LauncherActionKind.App => "open_app",
            LauncherActionKind.File => "open_file",
            LauncherActionKind.Folder => "open_folder",
            LauncherActionKind.Setting => "open",
            _ => null,
        };

        if (action is null)
        {
            return;
        }

        try
        {
            bool ok = LauncherApp.Bridge.FfiBindings.look_record_usage(result.Id, action);
            Log($"Dispatch record_usage: id='{result.Id}' action='{action}' ok={ok}");
        }
        catch (Exception ex)
        {
            Log($"Dispatch record_usage threw: {ex.Message}");
        }
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

        // UWP entries: shell:AppsFolder\<PackageFamilyName>!<AppId>. Process.GetProcessesByName(title)
        // misses these whenever the AppX display name and the executable basename diverge -
        // Windows Terminal ("Terminal" vs "WindowsTerminal.exe"), Photos, Snipping Tool, etc.
        // So we scan all running processes and match the WindowsApps install dir's package
        // name prefix against MainModule.FileName, which always contains the package full name
        // (e.g. C:\Program Files\WindowsApps\Microsoft.WindowsTerminal_<ver>_<arch>__<hash>\WindowsTerminal.exe).
        if (TryActivateUwpByAumid(path))
        {
            return true;
        }

        string resolved = ResolveExecutablePath(path);
        string normalizedPath = NormalizePath(resolved);
        bool hasExePath = normalizedPath.EndsWith(".exe", StringComparison.OrdinalIgnoreCase);

        // For UWP entries (shell:AppsFolder\<AUMID>) and anything else without a resolved .exe,
        // fall back to the result's display title as the process-name probe. Notepad's AUMID
        // doesn't end in .exe, but the running process is "Notepad" so the fallback matches.
        foreach (string processName in EnumerateProcessNameCandidates(hasExePath ? normalizedPath : null, title))
        {
            Process[] candidates;
            try
            {
                candidates = Process.GetProcessesByName(processName);
            }
            catch
            {
                continue;
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

                    if (!hasExePath)
                    {
                        // No exe path to match against - accept the first visible window for this
                        // process name. Safe because UWP apps expose one activation target.
                        ActivateWindow(hwnd);
                        return true;
                    }

                    string? processPath = process.MainModule?.FileName;
                    if (string.IsNullOrWhiteSpace(processPath))
                        continue;

                    if (!NormalizePath(processPath).Equals(normalizedPath, StringComparison.OrdinalIgnoreCase))
                        continue;

                    ActivateWindow(hwnd);
                    return true;
                }
                catch
                {
                }
            }

            if (fallbackWindow != IntPtr.Zero && IsLikelySingleAppAlias(path, normalizedPath, title))
            {
                ActivateWindow(fallbackWindow);
                return true;
            }
        }

        return false;
    }

    private static bool TryActivateUwpByAumid(string path)
    {
        const string prefix = "shell:AppsFolder\\";
        if (!path.StartsWith(prefix, StringComparison.OrdinalIgnoreCase))
        {
            return false;
        }

        string targetAumid = path.Substring(prefix.Length).Trim();
        if (targetAumid.IndexOf('!') <= 0)
        {
            return false;
        }

        Process[] processes;
        try
        {
            processes = Process.GetProcesses();
        }
        catch
        {
            return false;
        }

        // Exact-match by full AUMID (PackageFamilyName!AppId) via GetApplicationUserModelId
        // on each candidate process. Matching only on the package family name was ambiguous
        // when a single package exposes multiple AppIds (e.g. utilities that ship a "Settings"
        // entry alongside the main app) - selecting one would activate the other's window
        // and report success. Pre-filter by `\WindowsApps\` in the exe path so we only call
        // the kernel32 API on processes that could plausibly host a UWP entrypoint.
        foreach (var process in processes)
        {
            try
            {
                if (process.MainWindowHandle == IntPtr.Zero)
                    continue;

                string? processPath;
                try
                {
                    processPath = process.MainModule?.FileName;
                }
                catch
                {
                    // Access-denied on processes from other users / elevated; skip.
                    continue;
                }

                if (string.IsNullOrWhiteSpace(processPath))
                    continue;

                if (processPath.IndexOf("\\WindowsApps\\", StringComparison.OrdinalIgnoreCase) < 0)
                    continue;

                string? processAumid = TryGetProcessAumid(process.Id);
                if (processAumid is null)
                    continue;

                if (!string.Equals(processAumid, targetAumid, StringComparison.OrdinalIgnoreCase))
                    continue;

                ActivateWindow(process.MainWindowHandle);
                return true;
            }
            catch
            {
            }
            finally
            {
                try { process.Dispose(); } catch { }
            }
        }

        return false;
    }

    private static string? TryGetProcessAumid(int processId)
    {
        IntPtr handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, (uint)processId);
        if (handle == IntPtr.Zero)
        {
            return null;
        }

        try
        {
            uint length = 0;
            int sizeProbeRc = GetApplicationUserModelId(handle, ref length, null);
            // ERROR_INSUFFICIENT_BUFFER (122) is the only path that gives us the real length
            // for non-empty AUMIDs. Anything else (APPMODEL_ERROR_NO_APPLICATION = 15700,
            // success with length=0, etc.) means there's no AUMID to match against.
            if (sizeProbeRc != ERROR_INSUFFICIENT_BUFFER || length == 0)
            {
                return null;
            }

            char[] buffer = new char[length];
            int rc = GetApplicationUserModelId(handle, ref length, buffer);
            if (rc != 0 || length == 0)
            {
                return null;
            }

            // Returned length includes the trailing null terminator; trim it.
            int actual = (int)length;
            if (actual > 0 && buffer[actual - 1] == '\0')
            {
                actual -= 1;
            }
            return actual <= 0 ? null : new string(buffer, 0, actual);
        }
        finally
        {
            CloseHandle(handle);
        }
    }

    private static IEnumerable<string> EnumerateProcessNameCandidates(string? normalizedExePath, string? title)
    {
        var seen = new HashSet<string>(StringComparer.OrdinalIgnoreCase);

        if (!string.IsNullOrWhiteSpace(normalizedExePath))
        {
            string fromPath = Path.GetFileNameWithoutExtension(normalizedExePath);
            if (!string.IsNullOrWhiteSpace(fromPath) && seen.Add(fromPath))
                yield return fromPath;
        }

        if (!string.IsNullOrWhiteSpace(title))
        {
            string trimmed = title.Trim();
            if (seen.Add(trimmed))
                yield return trimmed;

            string noSpaces = trimmed.Replace(" ", string.Empty);
            if (noSpaces.Length > 0 && seen.Add(noSpaces))
                yield return noSpaces;
        }
    }

    private static void ActivateWindow(IntPtr hwnd)
    {
        // Only restore minimized windows. Calling SW_RESTORE on a maximized or fullscreen window
        // un-maximizes it (Edge losing F11/fullscreen when launched from search). SetForegroundWindow
        // alone is enough to pull a non-minimized window to the front.
        if (IsIconic(hwnd))
        {
            ShowWindowAsync(hwnd, SW_RESTORE);
        }
        SetForegroundWindow(hwnd);
    }

    private static string ResolveExecutablePath(string path)
    {
        string normalized = NormalizePath(path);
        if (!normalized.EndsWith(".lnk", StringComparison.OrdinalIgnoreCase))
            return normalized;

        if (ShortcutResolver.TryResolveShortcutTarget(normalized, out string targetPath)
            && !string.IsNullOrWhiteSpace(targetPath))
        {
            return NormalizePath(targetPath);
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

    private const uint PROCESS_QUERY_LIMITED_INFORMATION = 0x1000;
    private const int ERROR_INSUFFICIENT_BUFFER = 122;

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool SetForegroundWindow(IntPtr hWnd);

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool ShowWindowAsync(IntPtr hWnd, int nCmdShow);

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool IsIconic(IntPtr hWnd);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern IntPtr OpenProcess(uint dwDesiredAccess, bool bInheritHandle, uint dwProcessId);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool CloseHandle(IntPtr hObject);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern int GetApplicationUserModelId(
        IntPtr hProcess,
        ref uint applicationUserModelIdLength,
        [Out] char[]? applicationUserModelId);
}
