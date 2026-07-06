using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Net;
using System.Runtime.InteropServices;
using System.Text;
using LauncherApp.Services;

namespace LauncherApp.Commands;

public static class KillCommand
{
    public sealed record RunningApp(int Index, int Pid, string Name, string WindowTitle, string ExecutablePath);

    private static readonly Lazy<Dictionary<string, string>> ShortcutDisplayNameByTargetPath =
        new(BuildShortcutDisplayNameMap, true);
    private static readonly ConcurrentDictionary<string, string> DisplayNameByPath = new(StringComparer.OrdinalIgnoreCase);
    private static readonly ConcurrentDictionary<string, bool> HiddenPathCache = new(StringComparer.OrdinalIgnoreCase);

    public static (bool needsConfirmation, bool ok, string message, RunningApp? target) Resolve(string query)
    {
        string normalized = query.Trim();
        var apps = ListRunningApps();

        if (apps.Count == 0)
            return (false, false, "No apps running", null);

        if (string.IsNullOrWhiteSpace(normalized))
        {
            string listing = string.Join("\n", apps.Take(20).Select(FormatRunningAppLine));
            return (false, false, "Running apps:\n" + listing + "\n\nkill <name, title, or number>", null);
        }

        if (int.TryParse(normalized, out int index))
        {
            var selected = apps.FirstOrDefault(a => a.Index == index);
            if (selected is null)
                return (false, false, "Invalid app number", null);

            return (true, true, BuildConfirmMessage(selected), selected);
        }

        var matches = apps
            .Where(a => a.Name.Contains(normalized, StringComparison.OrdinalIgnoreCase)
                || a.WindowTitle.Contains(normalized, StringComparison.OrdinalIgnoreCase))
            .ToList();

        if (matches.Count == 0)
            return (false, false, "No matching apps. Use kill to list all.", null);

        if (matches.Count > 1)
        {
            string list = string.Join("\n", matches.Take(12).Select(FormatRunningAppLine));
            return (false, false, "Multiple matches:\n" + list + "\n\nBe more specific.", null);
        }

        var app = matches[0];
        return (true, true, BuildConfirmMessage(app), app);
    }

    public static List<RunningApp> ListRunningApps(string? filter = null)
    {
        string normalized = (filter ?? string.Empty).Trim();

        if (IsPortQuery(normalized))
        {
            if (!TryParsePortQuery(normalized, out int port))
            {
                return [];
            }

            return ListRunningAppsByPort(port);
        }

        int currentPid = Process.GetCurrentProcess().Id;
        var visibleWindows = GetVisibleWindowsByProcess();

        var windowedApps = new List<(int pid, string name, string title, string exePath)>();
        var fallbackApps = new List<(int pid, string name, string title, string exePath)>();

        foreach (var process in Process.GetProcesses())
        {
            try
            {
                if (process.Id == currentPid || process.Id <= 4)
                    continue;

                string name = string.IsNullOrWhiteSpace(process.ProcessName)
                    ? "Unknown"
                    : process.ProcessName;
                string executablePath = string.Empty;
                try
                {
                    executablePath = process.MainModule?.FileName ?? string.Empty;
                }
                catch
                {
                }

                if (string.IsNullOrWhiteSpace(executablePath))
                {
                    executablePath = TryGetProcessPath(process.Id);
                }

                bool hasVisibleWindow = visibleWindows.TryGetValue(process.Id, out string? title);
                string currentTitle = hasVisibleWindow ? title! : string.Empty;

                // Apply heavy system-helper filters only when the process is windowless.
                // UWP apps like the new Notepad / Windows Terminal live under \WindowsApps\
                // but ARE user-facing when they have a top-level window - skipping them there
                // hid them from the kill screen.
                if (ShouldHideProcess(name, executablePath, hasVisibleWindow))
                {
                    continue;
                }

                string displayName = ResolveDisplayName(name, executablePath, currentTitle);

                if (hasVisibleWindow)
                {
                    windowedApps.Add((process.Id, displayName, currentTitle, executablePath));
                }
                else if (!IsSystemNoise(name))
                {
                    fallbackApps.Add((process.Id, displayName, string.Empty, executablePath));
                }
            }
            catch
            {
            }
            finally
            {
                process.Dispose();
            }
        }

        IEnumerable<(int pid, string name, string title, string exePath)> apps = (windowedApps.Count > 0 ? windowedApps : fallbackApps)
            .GroupBy(x => x.pid)
            .Select(g => g.First())
            .OrderBy(x => x.name, StringComparer.OrdinalIgnoreCase)
            .ThenBy(x => x.title, StringComparer.OrdinalIgnoreCase);

        if (!string.IsNullOrWhiteSpace(normalized))
        {
            apps = apps.Where(x =>
                x.name.Contains(normalized, StringComparison.OrdinalIgnoreCase)
                || x.title.Contains(normalized, StringComparison.OrdinalIgnoreCase));
        }

        var result = new List<RunningApp>();
        int idx = 1;
        foreach (var app in apps)
        {
            result.Add(new RunningApp(idx++, app.pid, app.name, app.title, app.exePath));
        }

        return result;
    }

    private static List<RunningApp> ListRunningAppsByPort(int port)
    {
        int currentPid = Process.GetCurrentProcess().Id;
        var visibleWindows = GetVisibleWindowsByProcess();
        var pids = GetListeningPidsByPort(port)
            .Where(pid => pid > 4 && pid != currentPid)
            .OrderBy(pid => pid)
            .ToList();

        var result = new List<RunningApp>();
        int index = 1;
        foreach (int pid in pids)
        {
            try
            {
                using var process = Process.GetProcessById(pid);
                string name = string.IsNullOrWhiteSpace(process.ProcessName)
                    ? "Unknown"
                    : process.ProcessName;

                string executablePath = string.Empty;
                try
                {
                    executablePath = process.MainModule?.FileName ?? string.Empty;
                }
                catch
                {
                }

                if (string.IsNullOrWhiteSpace(executablePath))
                {
                    executablePath = TryGetProcessPath(pid);
                }

                bool hasVisibleWindow = visibleWindows.TryGetValue(pid, out string? titleValue);
                string currentTitle = hasVisibleWindow ? titleValue! : string.Empty;

                if (ShouldHideProcess(name, executablePath, hasVisibleWindow))
                {
                    continue;
                }

                string displayName = ResolveDisplayName(name, executablePath, currentTitle);
                result.Add(new RunningApp(index++, pid, displayName, $"Port: {port}", executablePath));
            }
            catch
            {
            }
        }

        return result;
    }

    public static (bool ok, string message) ConfirmKill(RunningApp target)
        => KillByPid(target.Pid, target.Name);

    private static string BuildConfirmMessage(RunningApp app)
    {
        string target = string.IsNullOrWhiteSpace(app.WindowTitle)
            ? app.Name
            : $"{app.Name} - {app.WindowTitle}";
        return $"Kill {target} (PID: {app.Pid})? Press Y to confirm, N to cancel.";
    }

    private static string FormatRunningAppLine(RunningApp app)
    {
        if (string.IsNullOrWhiteSpace(app.WindowTitle))
            return $"{app.Index}. {app.Name} (PID {app.Pid})";

        return $"{app.Index}. {app.Name} - {app.WindowTitle} (PID {app.Pid})";
    }

    private static Dictionary<int, string> GetVisibleWindowsByProcess()
    {
        var output = new Dictionary<int, string>();
        IntPtr shellWindow = GetShellWindow();

        EnumWindows((hWnd, _) =>
        {
            if (hWnd == shellWindow || !IsWindowVisible(hWnd))
                return true;

            int length = GetWindowTextLengthW(hWnd);
            if (length <= 0)
                return true;

            var titleBuffer = new StringBuilder(length + 1);
            _ = GetWindowTextW(hWnd, titleBuffer, titleBuffer.Capacity);
            string title = titleBuffer.ToString().Trim();
            if (string.IsNullOrWhiteSpace(title))
                return true;

            uint windowPid = 0;
            GetWindowThreadProcessId(hWnd, out windowPid);
            if (windowPid == 0)
                return true;

            int processId = unchecked((int)windowPid);
            if (!output.TryGetValue(processId, out string? existingTitle) || title.Length > existingTitle.Length)
            {
                output[processId] = title;
            }

            return true;
        }, IntPtr.Zero);

        return output;
    }

    private static bool IsSystemNoise(string processName)
    {
        return processName.Equals("svchost", StringComparison.OrdinalIgnoreCase)
            || processName.Equals("dwm", StringComparison.OrdinalIgnoreCase)
            || processName.Equals("ctfmon", StringComparison.OrdinalIgnoreCase)
            || processName.Equals("TextInputHost", StringComparison.OrdinalIgnoreCase)
            || processName.Equals("WindowsInternal.ComposableShell.Experiences.TextInput.InputApp", StringComparison.OrdinalIgnoreCase)
            || processName.Equals("SearchHost", StringComparison.OrdinalIgnoreCase)
            || processName.Equals("StartMenuExperienceHost", StringComparison.OrdinalIgnoreCase)
            || processName.Equals("ShellExperienceHost", StringComparison.OrdinalIgnoreCase)
            || processName.Equals("winlogon", StringComparison.OrdinalIgnoreCase)
            || processName.Equals("fontdrvhost", StringComparison.OrdinalIgnoreCase)
            || processName.Equals("csrss", StringComparison.OrdinalIgnoreCase)
            || processName.Equals("smss", StringComparison.OrdinalIgnoreCase)
            || processName.Equals("lsass", StringComparison.OrdinalIgnoreCase)
            || processName.Equals("registry", StringComparison.OrdinalIgnoreCase)
            || processName.Equals("services", StringComparison.OrdinalIgnoreCase)
            || processName.Equals("sihost", StringComparison.OrdinalIgnoreCase)
            || processName.Equals("taskhostw", StringComparison.OrdinalIgnoreCase);
    }

    private static bool IsPortQuery(string query)
    {
        if (string.IsNullOrWhiteSpace(query))
        {
            return false;
        }

        string normalized = query.Trim();
        return normalized.StartsWith(":", StringComparison.Ordinal)
            || normalized.StartsWith("port ", StringComparison.OrdinalIgnoreCase);
    }

    private static bool TryParsePortQuery(string query, out int port)
    {
        port = 0;
        string normalized = query.Trim();
        if (normalized.StartsWith(":", StringComparison.Ordinal))
        {
            normalized = normalized[1..].Trim();
        }
        else if (normalized.StartsWith("port ", StringComparison.OrdinalIgnoreCase))
        {
            normalized = normalized[5..].Trim();
        }
        else
        {
            return false;
        }

        if (!int.TryParse(normalized, out int parsed))
        {
            return false;
        }

        if (parsed < 1 || parsed > 65535)
        {
            return false;
        }

        port = parsed;
        return true;
    }

    private static HashSet<int> GetListeningPidsByPort(int port)
    {
        var pids = new HashSet<int>();
        CollectListeningPidsByPort(port, AddressFamilyInet, pids);
        CollectListeningPidsByPort(port, AddressFamilyInet6, pids);
        return pids;
    }

    private static void CollectListeningPidsByPort(int targetPort, int addressFamily, HashSet<int> output)
    {
        int bufferSize = 0;
        uint first = GetExtendedTcpTable(IntPtr.Zero, ref bufferSize, true, addressFamily, TcpTableClass.OwnerPidListener, 0);
        if (first != ErrorInsufficientBuffer || bufferSize <= 0)
        {
            return;
        }

        IntPtr buffer = IntPtr.Zero;
        try
        {
            buffer = Marshal.AllocHGlobal(bufferSize);
            uint result = GetExtendedTcpTable(buffer, ref bufferSize, true, addressFamily, TcpTableClass.OwnerPidListener, 0);
            if (result != ErrorSuccess)
            {
                return;
            }

            int rowCount = Marshal.ReadInt32(buffer);
            IntPtr rowPtr = IntPtr.Add(buffer, sizeof(uint));

            if (addressFamily == AddressFamilyInet)
            {
                int rowSize = Marshal.SizeOf<MibTcpRowOwnerPid>();
                for (int i = 0; i < rowCount; i++)
                {
                    var row = Marshal.PtrToStructure<MibTcpRowOwnerPid>(rowPtr);
                    if (row.State == TcpStateListen && ParsePort(row.LocalPort) == targetPort)
                    {
                        output.Add(unchecked((int)row.OwningPid));
                    }

                    rowPtr = IntPtr.Add(rowPtr, rowSize);
                }
            }
            else if (addressFamily == AddressFamilyInet6)
            {
                int rowSize = Marshal.SizeOf<MibTcp6RowOwnerPid>();
                for (int i = 0; i < rowCount; i++)
                {
                    var row = Marshal.PtrToStructure<MibTcp6RowOwnerPid>(rowPtr);
                    if (row.State == TcpStateListen && ParsePort(row.LocalPort) == targetPort)
                    {
                        output.Add(unchecked((int)row.OwningPid));
                    }

                    rowPtr = IntPtr.Add(rowPtr, rowSize);
                }
            }
        }
        catch
        {
        }
        finally
        {
            if (buffer != IntPtr.Zero)
            {
                Marshal.FreeHGlobal(buffer);
            }
        }
    }

    private static int ParsePort(uint portField)
    {
        byte[] bytes = BitConverter.GetBytes(portField);
        return (bytes[0] << 8) + bytes[1];
    }

    private static bool ShouldHideProcess(string processName, string executablePath, bool hasVisibleWindow)
    {
        if (IsSystemNoise(processName))
        {
            return true;
        }

        // Processes with a visible top-level window are always user-facing apps (Notepad UWP,
        // Windows Terminal, etc.). Skip the SystemApps / WindowsApps / Windows-Operating-System
        // filters so the user can still kill them.
        if (hasVisibleWindow)
        {
            return false;
        }

        string normalizedPath = NormalizePath(executablePath);
        if (string.IsNullOrWhiteSpace(normalizedPath))
            return false;

        return HiddenPathCache.GetOrAdd(normalizedPath, path =>
        {
            if (path.Contains("\\Windows\\SystemApps\\", StringComparison.OrdinalIgnoreCase)
                || path.Contains("\\WindowsApps\\", StringComparison.OrdinalIgnoreCase)
                || path.Contains("\\Windows\\ImmersiveControlPanel\\", StringComparison.OrdinalIgnoreCase))
            {
                return true;
            }

            try
            {
                var info = FileVersionInfo.GetVersionInfo(path);
                string product = (info.ProductName ?? string.Empty).Trim();
                string description = (info.FileDescription ?? string.Empty).Trim();
                return product.Equals("Microsoft Windows Operating System", StringComparison.OrdinalIgnoreCase)
                    || description.Equals("Microsoft Windows Operating System", StringComparison.OrdinalIgnoreCase);
            }
            catch
            {
                return false;
            }
        });
    }

    private static string ResolveDisplayName(string processName, string executablePath, string windowTitle)
    {
        string normalized = processName.Trim();

        string? fromWindowTitle = TryDeriveNameFromWindowTitle(windowTitle);
        if (!string.IsNullOrWhiteSpace(fromWindowTitle))
        {
            return fromWindowTitle;
        }

        if (!string.IsNullOrWhiteSpace(executablePath))
        {
            string normalizedPath = NormalizePath(executablePath);
            string cached = DisplayNameByPath.GetOrAdd(normalizedPath, path => ResolvePathDisplayName(path, normalized));
            if (!string.IsNullOrWhiteSpace(cached))
            {
                return cached;
            }
        }

        if (string.IsNullOrWhiteSpace(normalized))
            return "Unknown";

        return normalized;
    }

    private static string ResolvePathDisplayName(string normalizedPath, string processName)
    {
        if (ShortcutDisplayNameByTargetPath.Value.TryGetValue(normalizedPath, out string? shortcutName)
            && !string.IsNullOrWhiteSpace(shortcutName))
        {
            return shortcutName.Trim();
        }

        if (processName.Equals("mintty", StringComparison.OrdinalIgnoreCase)
            && TryGetGitBashNameFromShortcut(normalizedPath, out string gitBashName))
        {
            return gitBashName;
        }

        try
        {
            var info = FileVersionInfo.GetVersionInfo(normalizedPath);
            string description = FirstNonEmpty(info.FileDescription, info.ProductName);
            if (!string.IsNullOrWhiteSpace(description)
                && !description.Equals("Application", StringComparison.OrdinalIgnoreCase)
                && !description.Equals("Program", StringComparison.OrdinalIgnoreCase)
                && !description.Equals("Windows Software Development Kit", StringComparison.OrdinalIgnoreCase)
                && !description.Equals(Path.GetFileNameWithoutExtension(normalizedPath), StringComparison.OrdinalIgnoreCase))
            {
                return description.Trim();
            }
        }
        catch
        {
        }

        return string.Empty;
    }

    private static string? TryDeriveNameFromWindowTitle(string windowTitle)
    {
        if (string.IsNullOrWhiteSpace(windowTitle))
        {
            return null;
        }

        string trimmed = windowTitle.Trim();
        if (trimmed.Length == 0)
        {
            return null;
        }

        string[] parts = trimmed.Split(" - ", StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);
        if (parts.Length > 1)
        {
            string tail = parts[^1];
            if (IsGoodDisplaySegment(tail))
            {
                return tail;
            }
        }

        return null;
    }

    private static bool IsGoodDisplaySegment(string value)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            return false;
        }

        string normalized = value.Trim();
        if (normalized.Length < 3 || normalized.Length > 64)
        {
            return false;
        }

        if (normalized.Contains("\\") || normalized.Contains("/") || normalized.Contains(":\\"))
        {
            return false;
        }

        if (normalized.Contains('|'))
        {
            return false;
        }

        if (normalized.Equals("Administrator", StringComparison.OrdinalIgnoreCase)
            || normalized.Equals("Running applications", StringComparison.OrdinalIgnoreCase))
        {
            return false;
        }

        return true;
    }

    private static bool TryGetGitBashNameFromShortcut(string executablePath, out string displayName)
    {
        displayName = string.Empty;
        string marker = "\\git\\";
        int markerIndex = executablePath.IndexOf(marker, StringComparison.OrdinalIgnoreCase);
        if (markerIndex <= 0)
        {
            return false;
        }

        string gitRoot = executablePath[..(markerIndex + marker.Length)];
        foreach (var entry in ShortcutDisplayNameByTargetPath.Value)
        {
            if (!entry.Key.StartsWith(gitRoot, StringComparison.OrdinalIgnoreCase))
            {
                continue;
            }

            if (!entry.Value.Contains("bash", StringComparison.OrdinalIgnoreCase))
            {
                continue;
            }

            displayName = entry.Value.Trim();
            return displayName.Length > 0;
        }

        return false;
    }

    private static string TryGetProcessPath(int processId)
    {
        IntPtr handle = IntPtr.Zero;
        try
        {
            handle = OpenProcess(ProcessAccessFlags.QueryLimitedInformation, false, processId);
            if (handle == IntPtr.Zero)
            {
                return string.Empty;
            }

            int capacity = 1024;
            var sb = new StringBuilder(capacity);
            if (QueryFullProcessImageNameW(handle, 0, sb, ref capacity))
            {
                return sb.ToString();
            }
        }
        catch
        {
        }
        finally
        {
            if (handle != IntPtr.Zero)
            {
                _ = CloseHandle(handle);
            }
        }

        return string.Empty;
    }

    private static Dictionary<string, string> BuildShortcutDisplayNameMap()
    {
        var map = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        var startMenuRoots = new[]
        {
            Environment.GetFolderPath(Environment.SpecialFolder.StartMenu),
            Environment.GetFolderPath(Environment.SpecialFolder.CommonStartMenu),
        }
            .Where(path => !string.IsNullOrWhiteSpace(path) && Directory.Exists(path))
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .ToList();

        // EnumerationOptions.IgnoreInaccessible makes the enumerator skip
        // unreadable subtrees instead of throwing UnauthorizedAccessException
        // mid-iteration. Without it, a single permission error on one Start
        // Menu subfolder faults the surrounding Lazy<> initializer; every
        // future ShortcutDisplayNameByTargetPath.Value access then re-throws,
        // permanently breaking process-name resolution in the kill listing
        // for the rest of the session.
        EnumerationOptions enumerationOptions = new()
        {
            RecurseSubdirectories = true,
            IgnoreInaccessible = true,
        };

        foreach (string root in startMenuRoots)
        {
            IEnumerable<string> shortcuts;
            try
            {
                shortcuts = Directory.EnumerateFiles(root, "*.lnk", enumerationOptions);
            }
            catch
            {
                continue;
            }

            foreach (string shortcutPath in shortcuts)
            {
                try
                {
                    if (!ShortcutResolver.TryResolveShortcutTarget(shortcutPath, out string targetPath))
                    {
                        continue;
                    }

                    if (string.IsNullOrWhiteSpace(targetPath)
                        || !targetPath.EndsWith(".exe", StringComparison.OrdinalIgnoreCase)
                        || !File.Exists(targetPath))
                    {
                        continue;
                    }

                    string normalizedTarget = NormalizePath(targetPath);
                    if (map.ContainsKey(normalizedTarget))
                    {
                        continue;
                    }

                    string displayName = Path.GetFileNameWithoutExtension(shortcutPath).Trim();
                    if (!string.IsNullOrWhiteSpace(displayName))
                    {
                        map[normalizedTarget] = displayName;
                    }
                }
                catch
                {
                }
            }
        }

        return map;
    }

    private static string NormalizePath(string path)
    {
        string normalized = path.Trim().Replace('/', '\\');
        if (normalized.EndsWith("\\") && normalized.Length > 3)
        {
            normalized = normalized.TrimEnd('\\');
        }

        return normalized;
    }

    private static string FirstNonEmpty(params string?[] values)
    {
        foreach (string? value in values)
        {
            if (!string.IsNullOrWhiteSpace(value))
                return value;
        }

        return string.Empty;
    }

    private static (bool ok, string message) KillByPid(int pid, string name)
    {
        try
        {
            using var process = Process.GetProcessById(pid);
            process.Kill();
            return (true, $"Killed: {name} (PID: {pid})");
        }
        catch (Exception ex)
        {
            return (false, $"Failed to kill {name}: {ex.Message}");
        }
    }

    private delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

    private const uint ErrorSuccess = 0;
    private const uint ErrorInsufficientBuffer = 122;
    private const int AddressFamilyInet = 2;
    private const int AddressFamilyInet6 = 23;
    private const uint TcpStateListen = 2;

    [Flags]
    private enum ProcessAccessFlags : uint
    {
        QueryLimitedInformation = 0x1000,
    }

    [DllImport("user32.dll")]
    private static extern bool EnumWindows(EnumWindowsProc callback, IntPtr lParam);

    [DllImport("user32.dll")]
    private static extern bool IsWindowVisible(IntPtr hWnd);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetWindowTextW(IntPtr hWnd, StringBuilder lpString, int nMaxCount);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetWindowTextLengthW(IntPtr hWnd);

    [DllImport("user32.dll")]
    private static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);

    [DllImport("user32.dll")]
    private static extern IntPtr GetShellWindow();

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern IntPtr OpenProcess(ProcessAccessFlags processAccess, bool inheritHandle, int processId);

    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    private static extern bool QueryFullProcessImageNameW(IntPtr hProcess, int flags, StringBuilder exeName, ref int size);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool CloseHandle(IntPtr handle);

    [DllImport("iphlpapi.dll", SetLastError = true)]
    private static extern uint GetExtendedTcpTable(
        IntPtr pTcpTable,
        ref int pdwSize,
        bool bOrder,
        int ulAf,
        TcpTableClass tableClass,
        uint reserved);

    private enum TcpTableClass
    {
        BasicListener = 0,
        BasicConnections = 1,
        BasicAll = 2,
        OwnerPidListener = 3,
        OwnerPidConnections = 4,
        OwnerPidAll = 5,
        OwnerModuleListener = 6,
        OwnerModuleConnections = 7,
        OwnerModuleAll = 8,
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct MibTcpRowOwnerPid
    {
        public uint State;
        public uint LocalAddr;
        public uint LocalPort;
        public uint RemoteAddr;
        public uint RemotePort;
        public uint OwningPid;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct MibTcp6RowOwnerPid
    {
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 16)]
        public byte[] LocalAddr;
        public uint LocalScopeId;
        public uint LocalPort;
        [MarshalAs(UnmanagedType.ByValArray, SizeConst = 16)]
        public byte[] RemoteAddr;
        public uint RemoteScopeId;
        public uint RemotePort;
        public uint State;
        public uint OwningPid;
    }
}
