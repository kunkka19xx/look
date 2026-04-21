using System;
using System.IO;
using System.Linq;

namespace LauncherApp.Commands;

public static class SystemInfoCommand
{
    public static string BuildSummary()
    {
        string machine = Environment.MachineName;
        string os = Environment.OSVersion.VersionString;
        int cpu = Environment.ProcessorCount;
        double memGb = GC.GetGCMemoryInfo().TotalAvailableMemoryBytes > 0
            ? GC.GetGCMemoryInfo().TotalAvailableMemoryBytes / 1024d / 1024d / 1024d
            : 0;
        string uptime = FormatUptime(Environment.TickCount64);

        string disk = "N/A";
        try
        {
            var systemDrive = DriveInfo.GetDrives()
                .FirstOrDefault(d => d.IsReady && d.Name.StartsWith(Path.GetPathRoot(Environment.SystemDirectory) ?? "C", StringComparison.OrdinalIgnoreCase));
            if (systemDrive != null)
            {
                double free = systemDrive.AvailableFreeSpace / 1024d / 1024d / 1024d;
                double total = systemDrive.TotalSize / 1024d / 1024d / 1024d;
                disk = $"{free:0.#} GB free / {total:0.#} GB";
            }
        }
        catch
        {
        }

        return string.Join("\n", new[]
        {
            "System Info",
            $"Machine: {machine}",
            $"Windows: {os}",
            $"CPU: {cpu} logical cores",
            memGb > 0 ? $"Memory: {memGb:0.#} GB" : "Memory: N/A",
            $"Uptime: {uptime}",
            $"Disk: {disk}",
        });
    }

    private static string FormatUptime(long uptimeMs)
    {
        var ts = TimeSpan.FromMilliseconds(uptimeMs);
        if (ts.TotalDays >= 1)
            return $"{(int)ts.TotalDays}d {ts.Hours}h {ts.Minutes}m";

        return $"{ts.Hours}h {ts.Minutes}m";
    }
}
