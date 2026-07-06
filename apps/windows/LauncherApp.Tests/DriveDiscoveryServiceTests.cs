using System.Collections.Generic;
using System.IO;
using System.Linq;
using LauncherApp.Services;
using Xunit;

namespace LauncherApp.Tests;

public class DriveDiscoveryServiceTests
{
    [Fact]
    public void Filter_ExcludesSystemDrive()
    {
        var drives = new[]
        {
            Snapshot("C:\\", DriveType.Fixed, ready: true, label: "OS", total: 500_000_000_000),
            Snapshot("D:\\", DriveType.Fixed, ready: true, label: "Data", total: 1_000_000_000_000),
        };

        List<CandidateDrive> result = DriveDiscoveryService.Filter(
            drives,
            existingScanRoots: [],
            systemDriveLetter: "C:");

        Assert.Single(result);
        Assert.Equal("D", result[0].DriveLetter);
    }

    [Fact]
    public void Filter_PartiallyCoveredDrive_IsShownUnchecked()
    {
        // If the user has a sub-path on D: (e.g. D:\Projects) but not the bare root,
        // we still surface D: as a normal candidate. The user is free to add the whole
        // drive on top of the sub-path; managing that overlap is their call, not ours.
        var drives = new[]
        {
            Snapshot("D:\\", DriveType.Fixed, ready: true),
            Snapshot("E:\\", DriveType.Fixed, ready: true),
        };

        List<CandidateDrive> result = DriveDiscoveryService.Filter(
            drives,
            existingScanRoots: ["D:\\Projects"],
            systemDriveLetter: "C:");

        Assert.Equal(2, result.Count);
        Assert.False(result.Single(c => c.DriveLetter == "D").IsSelected);
        Assert.False(result.Single(c => c.DriveLetter == "E").IsSelected);
    }

    [Fact]
    public void Filter_ExactlyCoveredDrive_IsShownChecked()
    {
        // The bare drive root in scan roots means the user already opted the whole drive
        // in. We still surface it (checked) so they can uncheck to opt back out.
        var drives = new[]
        {
            Snapshot("D:\\", DriveType.Fixed, ready: true),
            Snapshot("E:\\", DriveType.Fixed, ready: true),
        };

        List<CandidateDrive> result = DriveDiscoveryService.Filter(
            drives,
            existingScanRoots: ["D:\\"],
            systemDriveLetter: "C:");

        Assert.Equal(2, result.Count);
        CandidateDrive d = result.Single(c => c.DriveLetter == "D");
        CandidateDrive e = result.Single(c => c.DriveLetter == "E");
        Assert.True(d.IsSelected);
        Assert.False(e.IsSelected);
    }

    [Theory]
    [InlineData("D:\\", true)]
    [InlineData("D:", true)]
    [InlineData("d:\\", true)]
    [InlineData("  E:\\  ", true)]
    [InlineData("F:/", true)]
    [InlineData("D:\\Projects", false)]
    [InlineData("Desktop", false)]
    [InlineData("", false)]
    [InlineData("   ", false)]
    public void IsBareDriveRoot_RecognizesDriveRoots(string path, bool expected)
    {
        Assert.Equal(expected, DriveDiscoveryService.IsBareDriveRoot(path));
    }

    [Fact]
    public void Filter_ExcludesNonFixedAndNotReadyDrives()
    {
        var drives = new[]
        {
            Snapshot("D:\\", DriveType.Removable, ready: true),  // USB
            Snapshot("E:\\", DriveType.Network, ready: true),    // mapped network
            Snapshot("F:\\", DriveType.CDRom, ready: true),
            Snapshot("G:\\", DriveType.Ram, ready: true),
            Snapshot("H:\\", DriveType.Fixed, ready: false),     // disconnected/locked
            Snapshot("I:\\", DriveType.Fixed, ready: true),      // the one survivor
        };

        List<CandidateDrive> result = DriveDiscoveryService.Filter(
            drives,
            existingScanRoots: [],
            systemDriveLetter: "C:");

        Assert.Single(result);
        Assert.Equal("I", result[0].DriveLetter);
    }

    [Fact]
    public void Filter_SystemDriveLetterIsCaseAndColonNormalized()
    {
        var drives = new[]
        {
            Snapshot("D:\\", DriveType.Fixed, ready: true),
            Snapshot("E:\\", DriveType.Fixed, ready: true),
        };

        // System drive given as lowercase with backslash - D: should still be filtered out.
        List<CandidateDrive> result = DriveDiscoveryService.Filter(
            drives,
            existingScanRoots: [],
            systemDriveLetter: "d:\\");

        Assert.Single(result);
        Assert.Equal("E", result[0].DriveLetter);
    }

    private static DriveSnapshot Snapshot(
        string root,
        DriveType type,
        bool ready,
        string? label = null,
        long? total = null,
        long? free = null)
    {
        return new DriveSnapshot(root, type, ready, label, free, total);
    }
}
