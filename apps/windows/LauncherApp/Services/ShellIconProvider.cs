using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Globalization;
using System.IO;
using System.Security.Cryptography;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading.Tasks;
using LauncherApp.Converters;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;

namespace LauncherApp.Services;

public sealed class ShellIconProvider
{
    private const StringComparison PathComparison = StringComparison.OrdinalIgnoreCase;
    private const int ShellPathBufferSize = 32768;

    private static readonly string LogPath = Path.Combine(Path.GetTempPath(), "look-icon-debug.log");
    private static readonly string IconCacheDir = Path.Combine(Path.GetTempPath(), "look-icon-cache");
    private static readonly bool DebugLoggingEnabled = Environment.GetEnvironmentVariable("LOOK_ICON_DEBUG") == "1";
    private const string IconCacheVersion = "v2";

    public async Task<ImageSource?> GetIconAsync(string path, bool smallIcon = true)
    {
        if (string.IsNullOrWhiteSpace(path))
            return null;

        if (path.StartsWith("ms-settings:", PathComparison))
        {
            var settingsIconPath = ResolveWindowsSettingsIconPath();
            if (string.IsNullOrEmpty(settingsIconPath))
                return null;

            path = settingsIconPath;
        }

        if (path.StartsWith("ms-", PathComparison))
            return null;

        if (path.StartsWith("command://", PathComparison))
            return null;

        IntPtr hIcon = IntPtr.Zero;

        try
        {
            var normalizedPath = NormalizePath(path);
            var isDirectory = Directory.Exists(normalizedPath);

            if (normalizedPath.EndsWith(".lnk", PathComparison))
            {
                hIcon = GetIconFromShortcut(normalizedPath, smallIcon);
                if (hIcon != IntPtr.Zero)
                {
                    Log("shortcut icon", normalizedPath, hIcon);
                }
            }

            if (hIcon == IntPtr.Zero)
            {
                hIcon = GetIconFromFile(normalizedPath, isDirectory, smallIcon);
                if (hIcon != IntPtr.Zero)
                {
                    Log("file icon", normalizedPath, hIcon);
                }
            }

            if (hIcon == IntPtr.Zero)
            {
                Log("icon missing", normalizedPath, IntPtr.Zero);
                return null;
            }

            var cachedImage = TryCreateCachedBitmapImage(hIcon, normalizedPath, smallIcon);
            if (cachedImage != null)
                return cachedImage;

            return await IconHandleToImageConverter.ConvertAsync(hIcon);
        }
        catch
        {
            return null;
        }
        finally
        {
            if (hIcon != IntPtr.Zero)
                DestroyIcon(hIcon);
        }
    }

    private static IntPtr GetIconFromShortcut(string shortcutPath, bool smallIcon)
    {
        if (!File.Exists(shortcutPath))
            return IntPtr.Zero;

        try
        {
            if (TryResolveShortcut(shortcutPath, out var targetPath, out var iconPath, out var iconIndex))
            {
                if (!string.IsNullOrWhiteSpace(iconPath))
                {
                    var extracted = ExtractSpecificIcon(iconPath, iconIndex, smallIcon);
                    if (extracted != IntPtr.Zero)
                        return extracted;
                }

                if (!string.IsNullOrWhiteSpace(targetPath))
                {
                    var target = NormalizePath(targetPath);
                    var isDir = Directory.Exists(target);
                    var fromTarget = GetIconFromFile(target, isDir, smallIcon);
                    if (fromTarget != IntPtr.Zero)
                        return fromTarget;
                }
            }
        }
        catch
        {
        }

        return GetIconFromFile(shortcutPath, isDir: false, smallIcon);
    }

    private static IntPtr GetIconFromFile(string path, bool isDir, bool smallIcon)
    {
        IntPtr hIcon = IntPtr.Zero;

        if (!isDir && path.EndsWith(".exe", PathComparison) && File.Exists(path))
        {
            try
            {
                IntPtr largeIcon, smallIconOut;
                ExtractIconExW(path, 0, out largeIcon, out smallIconOut, 1);

                if (smallIcon)
                {
                    hIcon = smallIconOut != IntPtr.Zero ? smallIconOut : largeIcon;
                    if (largeIcon != IntPtr.Zero && largeIcon != hIcon)
                        DestroyIcon(largeIcon);
                }
                else
                {
                    hIcon = largeIcon != IntPtr.Zero ? largeIcon : smallIconOut;
                    if (smallIconOut != IntPtr.Zero && smallIconOut != hIcon)
                        DestroyIcon(smallIconOut);
                }
            }
            catch { }
        }

        if (hIcon == IntPtr.Zero && File.Exists(path) && CanExtractIconDirectly(path))
        {
            try
            {
                var direct = ExtractSpecificIcon(path, 0, smallIcon);
                if (direct != IntPtr.Zero)
                    hIcon = direct;
            }
            catch { }
        }

        if (hIcon == IntPtr.Zero)
        {
            var shfi = new SHFILEINFO();
            uint flags = SHGFI_ICON | (smallIcon ? SHGFI_SMALLICON : SHGFI_LARGEICON);
            uint attrs = isDir ? FILE_ATTRIBUTE_DIRECTORY : FILE_ATTRIBUTE_NORMAL;

            if (!File.Exists(path) && !Directory.Exists(path))
                flags |= SHGFI_USEFILEATTRIBUTES;

            SHGetFileInfoW(path, attrs, out shfi, (uint)Marshal.SizeOf<SHFILEINFO>(), flags);
            hIcon = shfi.hIcon;
        }

        return hIcon;
    }

    private static bool CanExtractIconDirectly(string path)
    {
        var ext = Path.GetExtension(path);
        if (string.IsNullOrWhiteSpace(ext))
            return false;

        return ext.Equals(".exe", PathComparison)
            || ext.Equals(".dll", PathComparison)
            || ext.Equals(".ico", PathComparison)
            || ext.Equals(".icl", PathComparison)
            || ext.Equals(".mun", PathComparison);
    }

    private static IntPtr ExtractSpecificIcon(string path, int iconIndex, bool smallIcon)
    {
        if (string.IsNullOrWhiteSpace(path) || !File.Exists(path))
            return IntPtr.Zero;

        ExtractIconExW(path, iconIndex, out var largeIcon, out var smallIconOut, 1);
        if (smallIcon)
        {
            var selected = smallIconOut != IntPtr.Zero ? smallIconOut : largeIcon;
            if (largeIcon != IntPtr.Zero && largeIcon != selected)
                DestroyIcon(largeIcon);
            return selected;
        }

        var selectedLarge = largeIcon != IntPtr.Zero ? largeIcon : smallIconOut;
        if (smallIconOut != IntPtr.Zero && smallIconOut != selectedLarge)
            DestroyIcon(smallIconOut);
        return selectedLarge;
    }

    private static bool TryResolveShortcut(string shortcutPath, out string targetPath, out string iconPath, out int iconIndex)
    {
        targetPath = string.Empty;
        iconPath = string.Empty;
        iconIndex = 0;

        IShellLinkW? shellLink = null;
        IPersistFile? persistFile = null;
        try
        {
            shellLink = (IShellLinkW)new ShellLink();
            persistFile = (IPersistFile)shellLink;
            persistFile.Load(shortcutPath, 0);

            var pathBuffer = new StringBuilder(ShellPathBufferSize);
            shellLink.GetPath(pathBuffer, pathBuffer.Capacity, IntPtr.Zero, 0);
            targetPath = pathBuffer.ToString().Trim();

            var iconBuffer = new StringBuilder(ShellPathBufferSize);
            shellLink.GetIconLocation(iconBuffer, iconBuffer.Capacity, out iconIndex);
            iconPath = iconBuffer.ToString().Trim();

            return !string.IsNullOrWhiteSpace(targetPath) || !string.IsNullOrWhiteSpace(iconPath);
        }
        catch
        {
            return false;
        }
        finally
        {
            if (persistFile != null)
                Marshal.ReleaseComObject(persistFile);
            if (shellLink != null)
                Marshal.ReleaseComObject(shellLink);
        }
    }

    private static string NormalizePath(string path)
    {
        var normalized = path.Trim().Replace('/', '\\');

        if (normalized.EndsWith("\\") && normalized.Length > 3)
            normalized = normalized.TrimEnd('\\');

        return normalized;
    }

    private static string? ResolveWindowsSettingsIconPath()
    {
        var windowsDir = Environment.GetFolderPath(Environment.SpecialFolder.Windows);
        if (string.IsNullOrWhiteSpace(windowsDir))
            return null;

        var candidates = new[]
        {
            Path.Combine(windowsDir, "ImmersiveControlPanel", "SystemSettings.exe"),
            Path.Combine(windowsDir, "System32", "control.exe"),
        };

        foreach (var candidate in candidates)
        {
            if (File.Exists(candidate))
                return candidate;
        }

        return null;
    }

    private static ImageSource? TryCreateCachedBitmapImage(IntPtr hIcon, string sourcePath, bool smallIcon)
    {
        try
        {
            Directory.CreateDirectory(IconCacheDir);

            var cacheKey = ComputeHash(IconCacheVersion + "|" + sourcePath + "|" + (smallIcon ? "s" : "l"));
            var cachePath = Path.Combine(IconCacheDir, cacheKey + ".png");

            if (!File.Exists(cachePath) || new FileInfo(cachePath).Length == 0)
            {
                IntPtr safeIcon = CopyIcon(hIcon);
                var ownsSafeIcon = safeIcon != IntPtr.Zero;
                if (!ownsSafeIcon)
                    safeIcon = hIcon;

                try
                {
                    using var icon = Icon.FromHandle(safeIcon);
                    using var bitmap = icon.ToBitmap();
                    bitmap.Save(cachePath, ImageFormat.Png);
                }
                finally
                {
                    if (ownsSafeIcon)
                        DestroyIcon(safeIcon);
                }
            }

            var bitmapImage = new BitmapImage(new Uri(cachePath, UriKind.Absolute));
            Log("bitmap uri", cachePath, hIcon);
            return bitmapImage;
        }
        catch
        {
            return null;
        }
    }

    private static string ComputeHash(string text)
    {
        var bytes = SHA256.HashData(Encoding.UTF8.GetBytes(text));
        var sb = new StringBuilder(bytes.Length * 2);
        foreach (var b in bytes)
            sb.Append(b.ToString("x2", CultureInfo.InvariantCulture));
        return sb.ToString();
    }

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static void Log(string stage, string path, IntPtr hIcon)
    {
        if (!DebugLoggingEnabled)
            return;

        try
        {
            File.AppendAllText(LogPath, $"[{DateTime.Now:HH:mm:ss.fff}] {stage} | hIcon=0x{hIcon.ToInt64():X} | {path}{Environment.NewLine}");
        }
        catch
        {
        }
    }

    private const uint SHGFI_ICON = 0x100;
    private const uint SHGFI_LARGEICON = 0;
    private const uint SHGFI_SMALLICON = 0x1;
    private const uint SHGFI_USEFILEATTRIBUTES = 0x10;
    private const uint FILE_ATTRIBUTE_DIRECTORY = 0x10;
    private const uint FILE_ATTRIBUTE_NORMAL = 0x80;

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct SHFILEINFO
    {
        public IntPtr hIcon;
        public int iIcon;
        public uint dwAttributes;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 260)]
        public string szDisplayName;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 80)]
        public string szTypeName;
    }

    [DllImport("shell32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern IntPtr SHGetFileInfoW(string pszPath, uint dwFileAttributes, out SHFILEINFO psfi, uint cbFileInfo, uint uFlags);

    [DllImport("shell32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern IntPtr ExtractIconExW(string lpszFileExeFileName, int nIconIndex, out IntPtr phIconLarge, out IntPtr phIconSmall, uint nIcons);

    [DllImport("user32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool DestroyIcon(IntPtr hIcon);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern IntPtr CopyIcon(IntPtr hIcon);

    [ComImport]
    [Guid("00021401-0000-0000-C000-000000000046")]
    private class ShellLink
    {
    }

    [ComImport]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    [Guid("000214F9-0000-0000-C000-000000000046")]
    private interface IShellLinkW
    {
        void GetPath([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder pszFile, int cchMaxPath, IntPtr pfd, uint fFlags);
        void GetIDList(out IntPtr ppidl);
        void SetIDList(IntPtr pidl);
        void GetDescription([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder pszName, int cchMaxName);
        void SetDescription([MarshalAs(UnmanagedType.LPWStr)] string pszName);
        void GetWorkingDirectory([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder pszDir, int cchMaxPath);
        void SetWorkingDirectory([MarshalAs(UnmanagedType.LPWStr)] string pszDir);
        void GetArguments([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder pszArgs, int cchMaxPath);
        void SetArguments([MarshalAs(UnmanagedType.LPWStr)] string pszArgs);
        void GetHotkey(out short pwHotkey);
        void SetHotkey(short wHotkey);
        void GetShowCmd(out int piShowCmd);
        void SetShowCmd(int iShowCmd);
        void GetIconLocation([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder pszIconPath, int cchIconPath, out int piIcon);
        void SetIconLocation([MarshalAs(UnmanagedType.LPWStr)] string pszIconPath, int iIcon);
        void SetRelativePath([MarshalAs(UnmanagedType.LPWStr)] string pszPathRel, uint dwReserved);
        void Resolve(IntPtr hwnd, uint fFlags);
        void SetPath([MarshalAs(UnmanagedType.LPWStr)] string pszFile);
    }

    [ComImport]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    [Guid("0000010B-0000-0000-C000-000000000046")]
    private interface IPersistFile
    {
        void GetClassID(out Guid pClassID);
        void IsDirty();
        void Load([MarshalAs(UnmanagedType.LPWStr)] string pszFileName, int dwMode);
        void Save([MarshalAs(UnmanagedType.LPWStr)] string pszFileName, bool fRemember);
        void SaveCompleted([MarshalAs(UnmanagedType.LPWStr)] string pszFileName);
        void GetCurFile([MarshalAs(UnmanagedType.LPWStr)] out string ppszFileName);
    }
}
