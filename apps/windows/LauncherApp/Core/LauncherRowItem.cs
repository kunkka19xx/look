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
        "setting" => SearchItemKind.Setting,
        "app" => SearchItemKind.App,
        "file" => SearchItemKind.File,
        "folder" => SearchItemKind.Folder,
        "clipboard" => SearchItemKind.Unknown,
        _ => SearchItemKind.Unknown,
    } switch
    {
        SearchItemKind.App when IsSettingsResult() => SearchItemKind.Setting,
        var kind => kind,
    };

    public string KindLabel => Kind switch
    {
        SearchItemKind.App => "App",
        SearchItemKind.Setting => "Setting",
        SearchItemKind.File => "File",
        SearchItemKind.Folder => "Folder",
        _ when Result.Kind == "clipboard" => "Clipboard",
        _ => Result.Kind,
    };

    public string MetaLabel
    {
        get
        {
            if (Result.Kind == "clipboard")
                return Result.Subtitle ?? KindLabel;

            if (Kind == SearchItemKind.App || Kind == SearchItemKind.Setting)
                return Result.Subtitle ?? KindLabel;

            return KindLabel + "  •  " + PathInfo();
        }
    }

    // Kind-only chip for the row meta line (secondary text color).
    public string MetaKind => Result.Kind == "clipboard" ? KindLabel : KindLabel;

    // Path or subtitle (muted text color). Intentionally empty for clipboard + app/setting rows
    // so the row renders only the kind chip without a stray dot.
    public string MetaPath
    {
        get
        {
            if (Result.Kind == "clipboard")
                return Result.Subtitle ?? string.Empty;

            if (Kind == SearchItemKind.App || Kind == SearchItemKind.Setting)
                return Result.Subtitle ?? string.Empty;

            return PathInfo();
        }
    }

    public bool HasMetaPath => !string.IsNullOrEmpty(MetaPath) && MetaPath != MetaKind;

    public string IconGlyph => Kind switch
    {
        SearchItemKind.App => "\uE71D",
        SearchItemKind.Setting => "\uE713",
        SearchItemKind.File => "\uE8A5",
        SearchItemKind.Folder => "\uE8B7",
        _ when Result.Kind == "clipboard" => "\uE8C8",
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

    private bool IsSettingsResult()
    {
        return Result.Id.StartsWith("setting:", StringComparison.OrdinalIgnoreCase)
            || Result.Path.StartsWith("ms-settings:", StringComparison.OrdinalIgnoreCase);
    }
}
