using System;
using System.Threading.Tasks;
using LauncherApp.Bridge;
using LauncherApp.Services;
using Microsoft.UI.Xaml.Media;

namespace LauncherApp.Core;

public sealed class LauncherRowItem
{
    private static readonly IIconService SharedIconService = new IconService();

    private ImageSource? _icon;
    private bool _iconLoaded;

    public LauncherResult Result { get; }

    public string Title => Result.Title;

    public SearchItemKind Kind => Result.Kind switch
    {
        "app" => SearchItemKind.App,
        "file" => SearchItemKind.File,
        "folder" => SearchItemKind.Folder,
        "clipboard" => SearchItemKind.Unknown,
        _ => SearchItemKind.Unknown,
    };

    public string KindLabel => Result.Kind switch
    {
        "app" => "App",
        "file" => "File",
        "folder" => "Folder",
        "clipboard" => "Clipboard",
        _ => Result.Kind,
    };

    public string MetaLabel
    {
        get
        {
            if (Result.Kind == "clipboard")
                return Result.Subtitle ?? KindLabel;

            if (Result.Kind == "app")
                return Result.Subtitle ?? KindLabel;

            return KindLabel + "  •  " + PathInfo();
        }
    }

    public string IconGlyph => Result.Kind switch
    {
        "app" => "\uE71D",
        "file" => "\uE8A5",
        "folder" => "\uE8B7",
        "clipboard" => "\uE8C8",
        _ => "\uE8A5",
    };

    public ImageSource? Icon
    {
        get => _icon;
        set => _icon = value;
    }

    public bool HasIcon => _icon != null;

    public async Task LoadIconAsync()
    {
        if (_iconLoaded)
            return;

        _iconLoaded = true;

        if (!string.IsNullOrEmpty(Result.Path))
        {
            Icon = await SharedIconService.GetIconAsync(Result.Path, Kind);
        }
    }

    public LauncherRowItem(LauncherResult result)
    {
        Result = result;
    }

    private string PathInfo()
    {
        string parent = System.IO.Path.GetDirectoryName(Result.Path) ?? Result.Path;
        string normalized = parent.Replace('/', '\\').TrimEnd('\\');
        string[] parts = normalized.Split('\\', StringSplitOptions.RemoveEmptyEntries);

        if (parts.Length == 0)
            return "\\";

        int take = System.Math.Min(3, parts.Length);
        string tail = string.Join("\\", parts[^take..]);
        return parts.Length > 3 ? "...\\" + tail : "\\" + tail;
    }
}
