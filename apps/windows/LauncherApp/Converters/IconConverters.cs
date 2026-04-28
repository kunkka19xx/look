using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Threading.Tasks;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.Graphics.Imaging;
using Windows.Storage.Streams;

namespace LauncherApp.Converters;

public class IconHandleToImageConverter
{
    private static readonly string TempIconDir = Path.Combine(Path.GetTempPath(), "LookIcons");

    public static async Task<SoftwareBitmapSource?> ConvertAsync(IntPtr hIcon)
    {
        if (hIcon == IntPtr.Zero)
            return null;

        try
        {
            if (!Directory.Exists(TempIconDir))
                Directory.CreateDirectory(TempIconDir);

            using var gdiIcon = System.Drawing.Icon.FromHandle(hIcon);
            using var gdiBitmap = gdiIcon.ToBitmap();

            var width = gdiBitmap.Width;
            var height = gdiBitmap.Height;

            if (width <= 0 || height <= 0)
                return null;

            var stride = width * 4;
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
            softwareBitmap.CopyFromBuffer(pixels.AsBuffer());

            var source = new SoftwareBitmapSource();
            await source.SetBitmapAsync(softwareBitmap);
            return source;
        }
        catch
        {
            return null;
        }
    }

    public static SoftwareBitmapSource? Convert(IntPtr hIcon)
    {
        return ConvertAsync(hIcon).GetAwaiter().GetResult();
    }
}

public static class ByteArrayExtensions
{
    public static IBuffer AsBuffer(this byte[] array)
    {
        var writer = new DataWriter();
        writer.WriteBytes(array);
        return writer.DetachBuffer();
    }
}