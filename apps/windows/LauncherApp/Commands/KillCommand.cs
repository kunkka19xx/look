using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;

namespace LauncherApp.Commands;

public static class KillCommand
{
    public sealed record RunningApp(int Index, int Pid, string Name, string WindowTitle);

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
        int currentPid = Process.GetCurrentProcess().Id;
        var visibleWindows = GetVisibleWindowsByProcess();

        var windowedApps = new List<(int pid, string name, string title)>();
        var fallbackApps = new List<(int pid, string name, string title)>();

        foreach (var process in Process.GetProcesses())
        {
            try
            {
                if (process.Id == currentPid || process.Id <= 4)
                    continue;

                string name = string.IsNullOrWhiteSpace(process.ProcessName)
                    ? "Unknown"
                    : process.ProcessName;

                if (visibleWindows.TryGetValue(process.Id, out string? title))
                {
                    windowedApps.Add((process.Id, name, title));
                }
                else if (!IsSystemNoise(name))
                {
                    fallbackApps.Add((process.Id, name, string.Empty));
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

        IEnumerable<(int pid, string name, string title)> apps = (windowedApps.Count > 0 ? windowedApps : fallbackApps)
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
            result.Add(new RunningApp(idx++, app.pid, app.name, app.title));
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

    private static (bool ok, string message) KillByPid(int pid, string name)
    {
        try
        {
            using var process = Process.GetProcessById(pid);
            process.Kill(true);
            return (true, $"Killed: {name} (PID: {pid})");
        }
        catch (Exception ex)
        {
            return (false, $"Failed to kill {name}: {ex.Message}");
        }
    }

    private delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

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
}
