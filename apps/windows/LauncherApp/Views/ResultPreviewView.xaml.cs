using System;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using LauncherApp.Core;
using LauncherApp.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Imaging;

namespace LauncherApp.Views;

public sealed partial class ResultPreviewView : UserControl
{
    private LauncherRowItem? _currentClipboardRow;

    public event EventHandler<string>? ClipboardDeleteRequested;

    public ResultPreviewView()
    {
        this.InitializeComponent();
        SetRow(null);
    }

    public void SetRow(LauncherRowItem? row)
    {
        if (row == null)
        {
            SetEmpty();
            return;
        }

        TitleBlock.Text = string.IsNullOrWhiteSpace(row.Title) ? "Preview" : row.Title;
        GlyphBlock.Text = row.IconGlyph;
        HeaderImage.Visibility = Visibility.Collapsed;

        if (row.Result.Kind == "clipboard")
        {
            _currentClipboardRow = row;
            RenderClipboardPreview(row);
            return;
        }

        _currentClipboardRow = null;
        RenderStandardPreview(row);
    }

    private void ClipboardDeleteButton_OnClick(object sender, RoutedEventArgs e)
    {
        string? id = _currentClipboardRow?.Result.Id;
        if (string.IsNullOrEmpty(id))
        {
            return;
        }
        ClipboardDeleteRequested?.Invoke(this, id);
    }

    private void RenderStandardPreview(LauncherRowItem row)
    {
        StandardPanel.Visibility = Visibility.Visible;
        ClipboardPanel.Visibility = Visibility.Collapsed;
        HeaderMetaRow.Visibility = Visibility.Visible;
        SubtitleBlock.Visibility = Visibility.Collapsed;

        HeaderKindBadge.Text = row.KindLabel;
        PathText.Text = row.Result.Path;
        HeaderSizeText.Text = FormatSize(GetSizeBytes(row.Result.Path));
        ModifiedText.Text = GetModifiedLabel(row.Result.Path);

        string? version = GetVersion(row.Result.Path, row.Kind);
        VersionText.Text = string.IsNullOrWhiteSpace(version) ? "-" : version;

        if (TryBuildFileUri(row.Result.Path, out Uri? fileUri) && IsImageFile(row.Result.Path))
        {
            try
            {
                FilePreviewImage.Source = new BitmapImage(fileUri);
                FilePreviewImage.Visibility = Visibility.Visible;
                HeaderImage.Source = new BitmapImage(fileUri);
                HeaderImage.Visibility = Visibility.Visible;
            }
            catch
            {
                FilePreviewImage.Visibility = Visibility.Collapsed;
                HeaderImage.Visibility = Visibility.Collapsed;
            }
        }
        else
        {
            FilePreviewImage.Visibility = Visibility.Collapsed;
        }
    }

    private void RenderClipboardPreview(LauncherRowItem row)
    {
        StandardPanel.Visibility = Visibility.Collapsed;
        ClipboardPanel.Visibility = Visibility.Visible;
        HeaderMetaRow.Visibility = Visibility.Collapsed;
        SubtitleBlock.Visibility = Visibility.Visible;
        SubtitleBlock.Text = BuildHeaderSubtitle(row);

        string content = row.Result.Path ?? string.Empty;
        int chars = content.Length;
        int lines = string.IsNullOrEmpty(content) ? 0 : content.Split('\n').Length;

        ClipboardCapturedText.Text = string.IsNullOrWhiteSpace(row.Result.Subtitle)
            ? "Captured recently"
            : row.Result.Subtitle;
        ClipboardCharsText.Text = $"{chars} chars";
        ClipboardLinesText.Text = $"{lines} lines";
        ClipboardContentText.Text = content;
    }

    private void SetEmpty()
    {
        _currentClipboardRow = null;
        TitleBlock.Text = "Preview";
        SubtitleBlock.Text = "Select a result to view details";
        GlyphBlock.Text = "\uE8A5";
        HeaderImage.Visibility = Visibility.Collapsed;

        StandardPanel.Visibility = Visibility.Visible;
        ClipboardPanel.Visibility = Visibility.Collapsed;

        HeaderKindBadge.Text = "Item";
        PathText.Text = string.Empty;
        HeaderSizeText.Text = "Size";
        ModifiedText.Text = "-";
        VersionText.Text = "-";
        FilePreviewImage.Visibility = Visibility.Collapsed;
        HeaderMetaRow.Visibility = Visibility.Visible;
        SubtitleBlock.Visibility = Visibility.Collapsed;
    }

    private static string? GetVersion(string path, SearchItemKind kind)
    {
        if (kind != SearchItemKind.App)
        {
            return null;
        }

        try
        {
            if (!File.Exists(path))
            {
                return null;
            }

            FileVersionInfo info = FileVersionInfo.GetVersionInfo(path);
            return info.ProductVersion ?? info.FileVersion;
        }
        catch
        {
            return null;
        }
    }

    private static long GetSizeBytes(string path)
    {
        try
        {
            if (File.Exists(path))
            {
                return new FileInfo(path).Length;
            }
        }
        catch
        {
            return 0;
        }

        return 0;
    }

    private static string FormatSize(long bytes)
    {
        if (bytes <= 0)
        {
            return "-";
        }

        string[] units = ["B", "KB", "MB", "GB", "TB"];
        double size = bytes;
        int idx = 0;
        while (size >= 1024 && idx < units.Length - 1)
        {
            size /= 1024;
            idx++;
        }

        return $"{size:0.#} {units[idx]}";
    }

    private static string GetModifiedLabel(string path)
    {
        try
        {
            DateTime dt;
            if (File.Exists(path))
            {
                dt = File.GetLastWriteTime(path);
                return dt.ToString("g", CultureInfo.CurrentCulture);
            }

            if (Directory.Exists(path))
            {
                dt = Directory.GetLastWriteTime(path);
                return dt.ToString("g", CultureInfo.CurrentCulture);
            }
        }
        catch
        {
        }

        return "-";
    }

    private static bool IsImageFile(string path)
    {
        string ext = Path.GetExtension(path).ToLowerInvariant();
        return ext is ".jpg" or ".jpeg" or ".png" or ".gif" or ".bmp" or ".tiff" or ".webp" or ".ico";
    }

    private static bool TryBuildFileUri(string path, out Uri? uri)
    {
        uri = null;
        try
        {
            if (!File.Exists(path))
            {
                return false;
            }

            uri = new Uri(path);
            return true;
        }
        catch
        {
            return false;
        }
    }

    private static string BuildHeaderSubtitle(LauncherRowItem row)
    {
        string subtitle = row.Result.Subtitle?.Trim() ?? string.Empty;
        if (string.IsNullOrWhiteSpace(subtitle))
        {
            return "Select a result to view details";
        }

        if (string.Equals(subtitle, row.KindLabel, StringComparison.OrdinalIgnoreCase))
        {
            string parent = Path.GetDirectoryName(row.Result.Path) ?? string.Empty;
            return string.IsNullOrWhiteSpace(parent) ? subtitle : parent;
        }

        return subtitle;
    }
}
