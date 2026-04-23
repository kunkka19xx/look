using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Net.NetworkInformation;
using System.Net.Sockets;
using System.Runtime.InteropServices;
using Microsoft.Win32;

namespace LauncherApp.Commands;

public static class SystemInfoCommand
{
    private static readonly object CpuSampleGate = new();
    private static ulong _lastIdleTime;
    private static ulong _lastKernelTime;
    private static ulong _lastUserTime;
    private static bool _hasCpuSample;

    public static string BuildSummary()
    {
        string machine = Environment.MachineName;
        string os = Environment.OSVersion.VersionString;
        int logicalCpu = Environment.ProcessorCount;
        string cpuUsage = TryGetCpuUsagePercent(out double cpuPercent)
            ? $"{cpuPercent:0.#}%"
            : "N/A";

        string memoryUsage = "N/A";
        if (TryGetMemoryUsage(out double usedGb, out double totalGb, out double usedPercent))
        {
            memoryUsage = $"{usedGb:0.#} / {totalGb:0.#} GB ({usedPercent:0.#}%)";
        }

        string uptime = FormatUptime(Environment.TickCount64);

        string network = BuildNetworkSummary();
        string battery = BuildBatterySummary();
        string gpu = BuildGpuSummary();
        string[] topMemory = BuildTopMemorySummary();

        string disk = "N/A";
        try
        {
            var systemDrive = DriveInfo.GetDrives()
                .FirstOrDefault(d => d.IsReady && d.Name.StartsWith(Path.GetPathRoot(Environment.SystemDirectory) ?? "C", StringComparison.OrdinalIgnoreCase));
            if (systemDrive != null)
            {
                double free = systemDrive.AvailableFreeSpace / 1024d / 1024d / 1024d;
                double total = systemDrive.TotalSize / 1024d / 1024d / 1024d;
                double diskUsedPercent = total > 0 ? ((total - free) / total) * 100d : 0;
                disk = $"{free:0.#} GB free / {total:0.#} GB ({diskUsedPercent:0.#}% used)";
            }
        }
        catch
        {
        }

        var lines = new List<string>
        {
            "System Info",
            string.Empty,
            "[Overview]",
            $"Machine: {machine}",
            $"Windows: {os}",
            $"Uptime: {uptime}",
            string.Empty,
            "[Performance]",
            $"CPU: {logicalCpu} logical cores",
            $"CPU usage: {cpuUsage}",
            $"Memory usage: {memoryUsage}",
            "Top memory:",
        };

        lines.AddRange(topMemory.Select(entry => $"- {entry}"));
        lines.AddRange(
        [
            string.Empty,
            "[Hardware]",
            $"GPU: {gpu}",
            $"Battery: {battery}",
            $"Disk: {disk}",
            string.Empty,
            "[Network]",
            $"Status: {network}",
        ]);

        return string.Join("\n", lines);
    }

    private static string BuildNetworkSummary()
    {
        try
        {
            var active = NetworkInterface.GetAllNetworkInterfaces()
                .Where(n => n.OperationalStatus == OperationalStatus.Up
                    && n.NetworkInterfaceType != NetworkInterfaceType.Loopback
                    && n.NetworkInterfaceType != NetworkInterfaceType.Tunnel)
                .ToList();

            if (active.Count == 0)
            {
                return "Offline";
            }

            foreach (var nic in active)
            {
                var ip = nic.GetIPProperties().UnicastAddresses
                    .Select(x => x.Address)
                    .FirstOrDefault(a => a.AddressFamily == AddressFamily.InterNetwork && !a.ToString().StartsWith("169.254.", StringComparison.Ordinal));

                if (ip != null)
                {
                    return $"Online ({ip})";
                }
            }

            return "Online";
        }
        catch
        {
            return "N/A";
        }
    }

    private static string BuildBatterySummary()
    {
        try
        {
            if (!GetSystemPowerStatus(out var status))
            {
                return "N/A";
            }

            if (status.BatteryFlag == 128)
            {
                return "No battery";
            }

            string percent = status.BatteryLifePercent is <= 100 and >= 0
                ? $"{status.BatteryLifePercent}%"
                : "N/A";

            bool charging = (status.BatteryFlag & 8) == 8;
            string state = charging ? "Charging" : "On battery";
            if (status.ACLineStatus == 1)
            {
                state = charging ? "Charging" : "Plugged in";
            }

            if (status.BatteryLifeTime > 0 && status.BatteryLifeTime < int.MaxValue)
            {
                var remaining = TimeSpan.FromSeconds(status.BatteryLifeTime);
                return $"{percent} ({state}, {remaining.Hours}h {remaining.Minutes}m left)";
            }

            return $"{percent} ({state})";
        }
        catch
        {
            return "N/A";
        }
    }

    private static string BuildGpuSummary()
    {
        string[] candidates =
        [
            @"HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows NT\CurrentVersion\WinSAT|PrimaryAdapterString",
            @"HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows NT\CurrentVersion\WinSAT|PrimaryAdapterDescription",
        ];

        foreach (string candidate in candidates)
        {
            string[] split = candidate.Split('|');
            object? value = Registry.GetValue(split[0], split[1], null);
            if (value is string text && !string.IsNullOrWhiteSpace(text))
            {
                return text.Trim();
            }
        }

        return "N/A";
    }

    private static string[] BuildTopMemorySummary()
    {
        try
        {
            int currentPid = Environment.ProcessId;
            var top = new List<(string name, long workingSet)>();

            foreach (var process in Process.GetProcesses())
            {
                try
                {
                    if (process.Id <= 4 || process.Id == currentPid)
                    {
                        continue;
                    }

                    long ws = process.WorkingSet64;
                    if (ws <= 0)
                    {
                        continue;
                    }

                    string name = string.IsNullOrWhiteSpace(process.ProcessName) ? "unknown" : process.ProcessName;
                    top.Add((name, ws));
                }
                catch
                {
                }
                finally
                {
                    process.Dispose();
                }
            }

            if (top.Count == 0)
            {
                return ["N/A"];
            }

            return top.OrderByDescending(x => x.workingSet)
                .Take(3)
                .Select(x => $"{x.name} ({x.workingSet / 1024d / 1024d:0} MB)")
                .ToArray();
        }
        catch
        {
            return ["N/A"];
        }
    }

    private static bool TryGetMemoryUsage(out double usedGb, out double totalGb, out double usedPercent)
    {
        usedGb = 0;
        totalGb = 0;
        usedPercent = 0;

        var status = new MemoryStatusEx();
        status.Length = (uint)Marshal.SizeOf<MemoryStatusEx>();
        if (!GlobalMemoryStatusEx(ref status) || status.TotalPhys == 0)
        {
            return false;
        }

        ulong used = status.TotalPhys - status.AvailPhys;
        totalGb = status.TotalPhys / 1024d / 1024d / 1024d;
        usedGb = used / 1024d / 1024d / 1024d;
        usedPercent = used * 100d / status.TotalPhys;
        return true;
    }

    private static bool TryGetCpuUsagePercent(out double percent)
    {
        percent = 0;

        if (!GetSystemTimes(out var idle, out var kernel, out var user))
        {
            return false;
        }

        ulong idleTime = ToUInt64(idle);
        ulong kernelTime = ToUInt64(kernel);
        ulong userTime = ToUInt64(user);

        lock (CpuSampleGate)
        {
            if (!_hasCpuSample)
            {
                _lastIdleTime = idleTime;
                _lastKernelTime = kernelTime;
                _lastUserTime = userTime;
                _hasCpuSample = true;
                return false;
            }

            ulong idleDelta = idleTime - _lastIdleTime;
            ulong kernelDelta = kernelTime - _lastKernelTime;
            ulong userDelta = userTime - _lastUserTime;

            _lastIdleTime = idleTime;
            _lastKernelTime = kernelTime;
            _lastUserTime = userTime;

            ulong total = kernelDelta + userDelta;
            if (total == 0)
            {
                return false;
            }

            percent = Math.Clamp((total - idleDelta) * 100d / total, 0d, 100d);
            return true;
        }
    }

    private static ulong ToUInt64(FileTime fileTime)
    {
        return ((ulong)fileTime.HighDateTime << 32) | fileTime.LowDateTime;
    }

    private static string FormatUptime(long uptimeMs)
    {
        var ts = TimeSpan.FromMilliseconds(uptimeMs);
        if (ts.TotalDays >= 1)
            return $"{(int)ts.TotalDays}d {ts.Hours}h {ts.Minutes}m";

        return $"{ts.Hours}h {ts.Minutes}m";
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct FileTime
    {
        public uint LowDateTime;
        public uint HighDateTime;
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Auto)]
    private struct MemoryStatusEx
    {
        public uint Length;
        public uint MemoryLoad;
        public ulong TotalPhys;
        public ulong AvailPhys;
        public ulong TotalPageFile;
        public ulong AvailPageFile;
        public ulong TotalVirtual;
        public ulong AvailVirtual;
        public ulong AvailExtendedVirtual;
    }

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool GetSystemTimes(out FileTime idleTime, out FileTime kernelTime, out FileTime userTime);

    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Auto)]
    private static extern bool GlobalMemoryStatusEx(ref MemoryStatusEx lpBuffer);

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool GetSystemPowerStatus(out SystemPowerStatus systemPowerStatus);

    [StructLayout(LayoutKind.Sequential)]
    private struct SystemPowerStatus
    {
        public byte ACLineStatus;
        public byte BatteryFlag;
        public byte BatteryLifePercent;
        public byte Reserved1;
        public int BatteryLifeTime;
        public int BatteryFullLifeTime;
    }
}
