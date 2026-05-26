using System;
using System.Collections.Generic;
using System.IO;
using System.Runtime.InteropServices;

namespace LauncherApp.Features.Search;

// Mirrors AppConstants.Launcher.QuickFolder on macOS (apps/macos/.../AppConstants.swift).
// The Rust file walker (core/engine/src/index/files.rs) deliberately skips the file_scan
// roots themselves — it emits children but never the root directory — so typing
// "Documents", "Downloads", or "Desktop" in Look never returns those user folders from
// the engine. macOS compensates by injecting synthetic LauncherResult rows in the Swift
// view (LauncherView.swift quickFolderPinnedResults); this file is the Windows analog.
//
// Paths come from Environment.SpecialFolder (or SHGetKnownFolderPath for Downloads, which
// has no SpecialFolder enum entry) so OneDrive-redirected user folders resolve to the real
// `C:\Users\<user>\OneDrive\Desktop` path instead of an empty `C:\Users\<user>\Desktop`.
public static class QuickFolderCatalog
{
    public const string IdPrefix = "quickfolder:";
    public const string PinnedSubtitle = "Pinned home folder";
    public const int MinPrefixMatchLength = 2;
    public const int PinnedScore = 999_999;

    public sealed record QuickFolderEntry(string Title, string Path);

    private static readonly Lazy<IReadOnlyList<QuickFolderEntry>> CachedEntries = new(ResolveEntries);

    public static IReadOnlyList<QuickFolderEntry> Entries => CachedEntries.Value;

    private static IReadOnlyList<QuickFolderEntry> ResolveEntries()
    {
        var list = new List<QuickFolderEntry>(6);
        TryAdd(list, "Desktop", Environment.GetFolderPath(Environment.SpecialFolder.Desktop));
        TryAdd(list, "Documents", Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments));
        TryAdd(list, "Downloads", ResolveDownloadsPath());
        TryAdd(list, "Pictures", Environment.GetFolderPath(Environment.SpecialFolder.MyPictures));
        TryAdd(list, "Videos", Environment.GetFolderPath(Environment.SpecialFolder.MyVideos));
        TryAdd(list, "Music", Environment.GetFolderPath(Environment.SpecialFolder.MyMusic));
        return list;
    }

    private static void TryAdd(List<QuickFolderEntry> list, string title, string? path)
    {
        if (string.IsNullOrWhiteSpace(path))
        {
            return;
        }
        if (!Directory.Exists(path))
        {
            return;
        }
        list.Add(new QuickFolderEntry(title, path));
    }

    private static string? ResolveDownloadsPath()
    {
        // Environment.SpecialFolder has no Downloads value (the Vista-era KNOWNFOLDERID was
        // never backported to the enum). SHGetKnownFolderPath honors OneDrive redirection
        // the same way Explorer's Quick Access entry does — `C:\Users\<user>\Downloads`
        // when local, `C:\Users\<user>\OneDrive\Downloads` when the user enabled the
        // "Back up your folders" toggle in OneDrive.
        IntPtr ptr = IntPtr.Zero;
        try
        {
            int hr = SHGetKnownFolderPath(FolderIdDownloads, 0, IntPtr.Zero, out ptr);
            if (hr != 0 || ptr == IntPtr.Zero)
            {
                return null;
            }
            return Marshal.PtrToStringUni(ptr);
        }
        catch
        {
            return null;
        }
        finally
        {
            if (ptr != IntPtr.Zero)
            {
                Marshal.FreeCoTaskMem(ptr);
            }
        }
    }

    private static readonly Guid FolderIdDownloads = new("374DE290-123F-4565-9164-39C4925E467B");

    [DllImport("shell32.dll", CharSet = CharSet.Unicode, ExactSpelling = true, PreserveSig = true)]
    private static extern int SHGetKnownFolderPath(
        [MarshalAs(UnmanagedType.LPStruct)] Guid rfid,
        uint dwFlags,
        IntPtr hToken,
        out IntPtr ppszPath);
}
