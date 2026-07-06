using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;

namespace LauncherApp.Services;

public sealed record CandidateDrive(
    string DriveLetter,
    string RootPath,
    string DisplayLabel,
    string? VolumeLabel,
    long? FreeBytes,
    long? TotalBytes,
    bool IsSelected);

internal sealed record DriveSnapshot(
    string RootPath,
    DriveType Type,
    bool IsReady,
    string? VolumeLabel,
    long? FreeBytes,
    long? TotalBytes);

public static class DriveDiscoveryService
{
    public static List<CandidateDrive> Discover(IReadOnlyList<string> existingScanRoots)
    {
        return Filter(EnumerateFixedDrives(), existingScanRoots, GetSystemDriveLetter());
    }

    internal static List<CandidateDrive> Filter(
        IEnumerable<DriveSnapshot> drives,
        IReadOnlyList<string> existingScanRoots,
        string? systemDriveLetter)
    {
        // Track drives the user has opted into via their bare root ("D:\") so the
        // corresponding checkbox shows pre-checked. A sub-path on the same drive
        // (e.g. "D:\Projects") doesn't pre-check the box - the bare root and the
        // sub-path are independent scan entries, and we let the user manage any overlap.
        var exactlyCovered = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (string root in existingScanRoots)
        {
            if (IsBareDriveRoot(root))
            {
                string letter = ExtractDriveLetter(root);
                if (letter.Length > 0)
                {
                    exactlyCovered.Add(letter);
                }
            }
        }

        string sysLetter = NormalizeLetter(systemDriveLetter ?? string.Empty);

        var results = new List<CandidateDrive>();
        foreach (DriveSnapshot drive in drives)
        {
            if (drive.Type != DriveType.Fixed || !drive.IsReady)
            {
                continue;
            }

            string letter = ExtractDriveLetter(drive.RootPath);
            if (letter.Length == 0)
            {
                continue;
            }

            if (sysLetter.Length > 0 && letter.Equals(sysLetter, StringComparison.OrdinalIgnoreCase))
            {
                continue;
            }

            results.Add(new CandidateDrive(
                DriveLetter: letter,
                RootPath: letter + ":\\",
                DisplayLabel: BuildLabel(drive, letter),
                VolumeLabel: drive.VolumeLabel,
                FreeBytes: drive.FreeBytes,
                TotalBytes: drive.TotalBytes,
                IsSelected: exactlyCovered.Contains(letter)));
        }

        return results;
    }

    public static bool IsBareDriveRoot(string path)
    {
        if (string.IsNullOrWhiteSpace(path))
        {
            return false;
        }

        string trimmed = path.Trim().TrimEnd('\\', '/');
        return trimmed.Length == 2
            && trimmed[1] == ':'
            && char.IsLetter(trimmed[0]);
    }

    private static string BuildLabel(DriveSnapshot drive, string letter)
    {
        string head = letter + ":\\";
        if (!string.IsNullOrWhiteSpace(drive.VolumeLabel))
        {
            head += $" ({drive.VolumeLabel})";
        }

        if (drive.FreeBytes is long free && drive.TotalBytes is long total && total > 0)
        {
            head += $"  -  {FormatBytes(free)} free of {FormatBytes(total)}";
        }

        return head;
    }

    private static string FormatBytes(long bytes)
    {
        if (bytes <= 0)
        {
            return "0 B";
        }

        string[] units = ["B", "KB", "MB", "GB", "TB", "PB"];
        double value = bytes;
        int unit = 0;
        while (value >= 1024 && unit < units.Length - 1)
        {
            value /= 1024;
            unit++;
        }

        string fmt = value >= 100 ? "0" : value >= 10 ? "0.0" : "0.00";
        return value.ToString(fmt, CultureInfo.InvariantCulture) + " " + units[unit];
    }

    private static string ExtractDriveLetter(string path)
    {
        if (string.IsNullOrWhiteSpace(path))
        {
            return string.Empty;
        }

        string trimmed = path.Trim();
        // Home-relative entries (e.g. "Desktop") count toward whichever drive the user
        // profile lives on, so resolve them before reading the leading letter.
        if (!Path.IsPathFullyQualified(trimmed))
        {
            string profile = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
            if (string.IsNullOrWhiteSpace(profile))
            {
                return string.Empty;
            }

            trimmed = Path.Combine(profile, trimmed);
        }

        if (trimmed.Length >= 2 && trimmed[1] == ':' && char.IsLetter(trimmed[0]))
        {
            return char.ToUpperInvariant(trimmed[0]).ToString();
        }

        return string.Empty;
    }

    private static string NormalizeLetter(string raw)
    {
        if (string.IsNullOrWhiteSpace(raw))
        {
            return string.Empty;
        }

        string s = raw.Trim();
        if (s.Length >= 2 && s[1] == ':')
        {
            s = s[..1];
        }

        if (s.Length == 1 && char.IsLetter(s[0]))
        {
            return char.ToUpperInvariant(s[0]).ToString();
        }

        return string.Empty;
    }

    private static string GetSystemDriveLetter()
    {
        try
        {
            string? sys = Environment.GetEnvironmentVariable("SystemDrive");
            return string.IsNullOrWhiteSpace(sys) ? string.Empty : NormalizeLetter(sys);
        }
        catch
        {
            return string.Empty;
        }
    }

    private static IEnumerable<DriveSnapshot> EnumerateFixedDrives()
    {
        DriveInfo[] drives;
        try
        {
            drives = DriveInfo.GetDrives();
        }
        catch
        {
            drives = Array.Empty<DriveInfo>();
        }

        foreach (DriveInfo drive in drives)
        {
            DriveSnapshot? snapshot = TrySnapshot(drive);
            if (snapshot is not null)
            {
                yield return snapshot;
            }
        }
    }

    private static DriveSnapshot? TrySnapshot(DriveInfo drive)
    {
        try
        {
            string root = drive.RootDirectory.FullName;
            DriveType type = drive.DriveType;
            bool ready = drive.IsReady;

            string? label = null;
            long? free = null;
            long? total = null;

            if (ready)
            {
                try { label = drive.VolumeLabel; } catch { }
                try { free = drive.AvailableFreeSpace; } catch { }
                try { total = drive.TotalSize; } catch { }
            }

            return new DriveSnapshot(root, type, ready, label, free, total);
        }
        catch
        {
            return null;
        }
    }
}
