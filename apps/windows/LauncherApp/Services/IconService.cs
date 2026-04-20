using System;
using System.Collections.Concurrent;
using System.IO;
using System.Runtime.InteropServices;
using System.Threading.Tasks;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.Graphics.Imaging;
using Windows.Storage.Streams;

namespace LauncherApp.Services;

public static class IconService
{
    private static readonly ConcurrentDictionary<string, ImageSource> IconCache = new();

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

    private const uint SHGFI_ICON = 0x100;
    private const uint SHGFI_LARGEICON = 0;

    [DllImport("shell32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern IntPtr SHGetFileInfo(
        string pszPath,
        uint dwFileAttributes,
        ref SHFILEINFO psfi,
        uint cbSizeFileInfo,
        uint uFlags);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool DestroyIcon(IntPtr hIcon);

    public static ImageSource? GetIcon(string? path)
    {
        if (string.IsNullOrEmpty(path))
            return null;

        string key = path.ToLowerInvariant();

        if (IconCache.TryGetValue(key, out var cached))
            return cached;

        var icon = ExtractIcon(path);
        if (icon != null)
            IconCache[key] = icon;

        return icon;
    }

    private static ImageSource? ExtractIcon(string path)
    {
        if (string.IsNullOrEmpty(path))
            return null;

        string actualPath = path.Replace('/', '\\');

        if (actualPath.EndsWith(".lnk", StringComparison.OrdinalIgnoreCase))
        {
            if (!File.Exists(actualPath))
                return null;

            string? resolved = ResolveShortcut(actualPath);
            if (!string.IsNullOrEmpty(resolved))
            {
                string normalized = resolved.Replace('/', '\\');
                if (File.Exists(normalized))
                    actualPath = normalized;
                else if (Directory.Exists(normalized))
                    actualPath = normalized;
            }
        }

        bool isFile = File.Exists(actualPath);
        bool isDir = Directory.Exists(actualPath);

        if (!isFile && !isDir)
            return null;

        var info = new SHFILEINFO();
        IntPtr _ = SHGetFileInfo(
            actualPath,
            isDir ? 0x10u : 0,
            ref info,
            (uint)Marshal.SizeOf<SHFILEINFO>(),
            SHGFI_ICON | SHGFI_LARGEICON);

        IntPtr hIcon = info.hIcon;
        if (hIcon == IntPtr.Zero)
            return null;

        try
        {
            return ConvertHIconToImageSource(hIcon);
        }
        finally
        {
            DestroyIcon(hIcon);
        }
    }

    private static ImageSource? ConvertHIconToImageSource(IntPtr hIcon)
    {
        using var gdiIcon = System.Drawing.Icon.FromHandle(hIcon);
        using var gdiBitmap = gdiIcon.ToBitmap();

        int width = gdiBitmap.Width;
        int height = gdiBitmap.Height;

        if (width <= 0 || height <= 0)
            return null;

        int stride = width * 4;
        var pixels = new byte[height * stride];

        var bitmapData = gdiBitmap.LockBits(
            new System.Drawing.Rectangle(0, 0, width, height),
            System.Drawing.Imaging.ImageLockMode.ReadOnly,
            System.Drawing.Imaging.PixelFormat.Format32bppArgb);

        try
        {
            Marshal.Copy(bitmapData.Scan0, pixels, 0, pixels.Length);
        }
        finally
        {
            gdiBitmap.UnlockBits(bitmapData);
        }

        var softwareBitmap = new SoftwareBitmap(BitmapPixelFormat.Bgra8, width, height);
        
        var writer = new DataWriter();
        writer.WriteBytes(pixels);
        var buffer = writer.DetachBuffer();
        softwareBitmap.CopyFromBuffer(buffer);

        var source = new SoftwareBitmapSource();
        
        var task = Task.Run(async () =>
        {
            await source.SetBitmapAsync(softwareBitmap);
        });
        task.Wait();

        return source;
    }

    private static string? ResolveShortcut(string lnkPath)
    {
        try
        {
            if (!File.Exists(lnkPath))
                return null;

            var shellType = Type.GetTypeFromProgID("WScript.Shell");
            if (shellType is null)
                return null;

            dynamic? shell = Activator.CreateInstance(shellType);
            if (shell is null)
                return null;

            dynamic shortcut = shell.CreateShortcut(lnkPath);
            return shortcut.TargetPath;
        }
        catch
        {
            return null;
        }
    }

    public static void ClearCache()
    {
        IconCache.Clear();
    }
}