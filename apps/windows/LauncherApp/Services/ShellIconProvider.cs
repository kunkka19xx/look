using System;
using System.Diagnostics;
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

    private static readonly string LookCacheRoot = ResolveLookCacheRoot();
    private static readonly string LogPath = Path.Combine(LookCacheRoot, "icon-debug.log");
    private static readonly string IconCacheDir = Path.Combine(LookCacheRoot, "icon-cache");
    private static readonly bool DebugLoggingEnabled = Environment.GetEnvironmentVariable("LOOK_ICON_DEBUG") == "1";
    private const string IconCacheVersion = "v2";

    private static string ResolveLookCacheRoot()
    {
        try
        {
            string localAppData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
            if (!string.IsNullOrWhiteSpace(localAppData))
            {
                return Path.Combine(localAppData, "look");
            }
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"[ShellIconProvider] LocalAppData lookup failed: {ex.Message}");
        }

        return Path.Combine(Path.GetTempPath(), "look");
    }

    public async Task<ImageSource?> GetIconAsync(string path, bool smallIcon = true)
    {
        if (string.IsNullOrWhiteSpace(path))
            return null;

        // ms-settings: URIs intentionally return null so the row falls back to the per-page
        // Fluent glyph in SettingsIconCatalog. Resolving them to SystemSettings.exe gave every
        // setting row the same generic gear PNG, which always outranks the glyph in the row
        // template (LauncherResultRowView swaps to IconImage as soon as Icon is non-null).
        if (path.StartsWith("ms-settings:", PathComparison))
            return null;

        if (path.StartsWith("ms-", PathComparison))
            return null;

        if (path.StartsWith("command://", PathComparison))
            return null;

        IntPtr hIcon = IntPtr.Zero;

        try
        {
            var normalizedPath = NormalizePath(path);
            var isDirectory = Directory.Exists(normalizedPath);

            int shellSizePx = smallIcon ? 32 : 64;

            // Factory path only for inputs the HICON pipeline handles poorly - UWP shell items
            // and .lnk stubs that point at packaged apps. Plain .exe / .url / files keep using
            // the proven ExtractIconExW / SHGetFileInfoW path to avoid native regressions.
            bool useShellFactoryFirst = normalizedPath.StartsWith("shell:", PathComparison)
                                         || normalizedPath.EndsWith(".lnk", PathComparison);
            if (useShellFactoryFirst)
            {
                var shellImage = TryCreateImageViaShellItemImageFactory(normalizedPath, shellSizePx, smallIcon);
                if (shellImage != null)
                {
                    Log("shell item image", normalizedPath, IntPtr.Zero);
                    return shellImage;
                }
            }

            if (normalizedPath.EndsWith(".lnk", PathComparison))
            {
                hIcon = GetIconFromShortcut(normalizedPath, smallIcon);
                if (hIcon != IntPtr.Zero)
                {
                    Log("shortcut icon", normalizedPath, hIcon);
                }
            }
            else if (normalizedPath.EndsWith(".url", PathComparison))
            {
                hIcon = GetIconFromUrlFile(normalizedPath, smallIcon);
                if (hIcon != IntPtr.Zero)
                {
                    Log("url icon", normalizedPath, hIcon);
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
        catch (Exception ex)
        {
            Debug.WriteLine($"[ShellIconProvider] GetIconAsync failed for '{path}': {ex.Message}");
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
        catch (Exception ex)
        {
            Debug.WriteLine($"[ShellIconProvider] shortcut resolve failed for '{shortcutPath}': {ex.Message}");
        }

        return GetIconFromFile(shortcutPath, isDir: false, smallIcon);
    }

    private static IntPtr GetIconFromUrlFile(string urlPath, bool smallIcon)
    {
        if (!File.Exists(urlPath))
            return IntPtr.Zero;

        try
        {
            string? iconFile = null;
            int iconIndex = 0;

            foreach (string rawLine in File.ReadAllLines(urlPath))
            {
                string line = rawLine.Trim();
                if (line.Length == 0 || line.StartsWith("[", PathComparison))
                    continue;

                int eq = line.IndexOf('=');
                if (eq <= 0)
                    continue;

                string key = line[..eq].Trim();
                string value = line[(eq + 1)..].Trim();

                if (key.Equals("IconFile", PathComparison))
                {
                    iconFile = value;
                }
                else if (key.Equals("IconIndex", PathComparison))
                {
                    int.TryParse(value, NumberStyles.Integer, CultureInfo.InvariantCulture, out iconIndex);
                }
            }

            if (!string.IsNullOrWhiteSpace(iconFile))
            {
                string resolved = Environment.ExpandEnvironmentVariables(iconFile);
                var extracted = ExtractSpecificIcon(resolved, iconIndex, smallIcon);
                if (extracted != IntPtr.Zero)
                    return extracted;
            }
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"[ShellIconProvider] .url parse failed for '{urlPath}': {ex.Message}");
        }

        return IntPtr.Zero;
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
            catch (Exception ex) { Debug.WriteLine($"[ShellIconProvider] icon extract failed for '{path}': {ex.Message}"); }
        }

        if (hIcon == IntPtr.Zero && File.Exists(path) && CanExtractIconDirectly(path))
        {
            try
            {
                var direct = ExtractSpecificIcon(path, 0, smallIcon);
                if (direct != IntPtr.Zero)
                    hIcon = direct;
            }
            catch (Exception ex) { Debug.WriteLine($"[ShellIconProvider] icon extract failed for '{path}': {ex.Message}"); }
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
        catch (Exception ex)
        {
            Debug.WriteLine($"[ShellIconProvider] IShellLinkW load failed for '{shortcutPath}': {ex.Message}");
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
        catch (Exception ex)
        {
            Debug.WriteLine($"[ShellIconProvider] cache write failed for '{sourcePath}': {ex.Message}");
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

    private static ImageSource? TryCreateImageViaShellItemImageFactory(string path, int sizePx, bool smallIcon)
    {
        if (string.IsNullOrWhiteSpace(path))
            return null;

        if (!File.Exists(path) && !Directory.Exists(path) && !path.StartsWith("shell:", PathComparison))
            return null;

        IntPtr hBitmap = IntPtr.Zero;
        object? ppv = null;
        IShellItemImageFactory? factory = null;
        try
        {
            Guid iidImageFactory = new Guid("BCC18B79-BA16-442F-80C4-8A59C30C463B");
            SHCreateItemFromParsingName(path, IntPtr.Zero, ref iidImageFactory, out ppv);
            factory = ppv as IShellItemImageFactory;
            if (factory is null)
            {
                return null;
            }

            int hr = factory.GetImage(new SIZE(sizePx, sizePx), SIIGBF.ResizeToFit | SIIGBF.IconOnly, out hBitmap);
            if (hr != 0 || hBitmap == IntPtr.Zero)
            {
                return null;
            }

            Directory.CreateDirectory(IconCacheDir);
            string cacheKey = ComputeHash(IconCacheVersion + "|shell|" + path + "|" + sizePx + "|" + (smallIcon ? "s" : "l"));
            string cachePath = Path.Combine(IconCacheDir, cacheKey + ".png");

            if (!File.Exists(cachePath) || new FileInfo(cachePath).Length == 0)
            {
                using var bitmap = HbitmapToBitmapPreservingAlpha(hBitmap);
                if (bitmap is null)
                {
                    return null;
                }
                bitmap.Save(cachePath, ImageFormat.Png);
            }

            return new BitmapImage(new Uri(cachePath, UriKind.Absolute));
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"[ShellIconProvider] IShellItemImageFactory failed for '{path}': {ex.Message}");
            return null;
        }
        finally
        {
            if (hBitmap != IntPtr.Zero)
                DeleteObject(hBitmap);
            if (factory is not null)
            {
                try { Marshal.ReleaseComObject(factory); } catch { }
            }
            else if (ppv is not null)
            {
                try { Marshal.ReleaseComObject(ppv); } catch { }
            }
        }
    }

    private static unsafe Bitmap? HbitmapToBitmapPreservingAlpha(IntPtr hBitmap)
    {
        if (hBitmap == IntPtr.Zero)
            return null;

        BITMAP bm = default;
        if (GetObject(hBitmap, Marshal.SizeOf<BITMAP>(), ref bm) == 0 || bm.bmBits == IntPtr.Zero)
            return null;

        if (bm.bmBitsPixel != 32)
            return null;

        int width = bm.bmWidth;
        int height = Math.Abs(bm.bmHeight);
        int stride = bm.bmWidthBytes;
        bool topDown = bm.bmHeight < 0;

        // Reject degenerate or absurd sizes before allocating or doing pointer math.
        if (width <= 0 || height <= 0 || width > 4096 || height > 4096)
        {
            Debug.WriteLine($"[ShellIconProvider] HBITMAP rejected: w={width} h={height}");
            return null;
        }

        // Stride must be at least width*4 for 32-bpp; a smaller stride means reading garbage bytes.
        if (stride < width * 4)
        {
            Debug.WriteLine($"[ShellIconProvider] HBITMAP stride too small: stride={stride} width*4={width * 4}");
            return null;
        }

        var bitmap = new Bitmap(width, height, PixelFormat.Format32bppArgb);
        var rect = new Rectangle(0, 0, width, height);
        var data = bitmap.LockBits(rect, ImageLockMode.WriteOnly, PixelFormat.Format32bppArgb);

        try
        {
            byte* srcBase = (byte*)bm.bmBits;
            byte* dstBase = (byte*)data.Scan0;
            int copyBytes = Math.Min(stride, data.Stride);

            for (int y = 0; y < height; y++)
            {
                int srcRow = topDown ? y : (height - 1 - y);
                byte* src = srcBase + srcRow * stride;
                byte* dst = dstBase + y * data.Stride;

                for (int x = 0; x < width; x++)
                {
                    byte b = src[0];
                    byte g = src[1];
                    byte r = src[2];
                    byte a = src[3];

                    if (a != 0 && a != 255)
                    {
                        b = (byte)System.Math.Min(255, b * 255 / a);
                        g = (byte)System.Math.Min(255, g * 255 / a);
                        r = (byte)System.Math.Min(255, r * 255 / a);
                    }

                    dst[0] = b;
                    dst[1] = g;
                    dst[2] = r;
                    dst[3] = a;

                    src += 4;
                    dst += 4;
                }
            }
        }
        finally
        {
            bitmap.UnlockBits(data);
        }

        return bitmap;
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

    [DllImport("shell32.dll", CharSet = CharSet.Unicode, ExactSpelling = true, PreserveSig = false)]
    private static extern void SHCreateItemFromParsingName(
        [MarshalAs(UnmanagedType.LPWStr)] string pszPath,
        IntPtr pbc,
        [In] ref Guid riid,
        [MarshalAs(UnmanagedType.Interface)] out object? ppv);

    [DllImport("gdi32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool DeleteObject(IntPtr hObject);

    [DllImport("gdi32.dll", SetLastError = true)]
    private static extern int GetObject(IntPtr hObject, int nCount, ref BITMAP lpObject);

    [StructLayout(LayoutKind.Sequential)]
    private struct BITMAP
    {
        public int bmType;
        public int bmWidth;
        public int bmHeight;
        public int bmWidthBytes;
        public ushort bmPlanes;
        public ushort bmBitsPixel;
        public IntPtr bmBits;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct SIZE
    {
        public int cx;
        public int cy;
        public SIZE(int x, int y) { cx = x; cy = y; }
    }

    [Flags]
    private enum SIIGBF : uint
    {
        ResizeToFit = 0x00,
        BiggerSizeOk = 0x01,
        MemoryOnly = 0x02,
        IconOnly = 0x04,
        ThumbnailOnly = 0x08,
        InCacheOnly = 0x10,
        ScaleUp = 0x100,
    }

    [ComImport]
    [Guid("BCC18B79-BA16-442F-80C4-8A59C30C463B")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private interface IShellItemImageFactory
    {
        [PreserveSig]
        int GetImage(SIZE size, SIIGBF flags, out IntPtr phbm);
    }

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
