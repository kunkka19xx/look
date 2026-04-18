using LauncherApp.Bridge;

namespace LauncherApp.Core;

public sealed class LauncherRowItem
{
    public LauncherResult Result { get; }

    public string Title => Result.Title;

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
            {
                return Result.Subtitle ?? KindLabel;
            }

            if (Result.Kind == "app")
            {
                return Result.Subtitle ?? KindLabel;
            }

            return $"{KindLabel}  •  {PathInfo()}";
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

    public LauncherRowItem(LauncherResult result)
    {
        Result = result;
    }

    private string PathInfo()
    {
        string parent = System.IO.Path.GetDirectoryName(Result.Path) ?? Result.Path;
        string normalized = parent.Replace('/', '\\').TrimEnd('\\');
        string[] parts = normalized.Split('\\', System.StringSplitOptions.RemoveEmptyEntries);

        if (parts.Length == 0)
        {
            return "\\";
        }

        int take = System.Math.Min(3, parts.Length);
        string tail = string.Join("\\", parts[^take..]);
        return parts.Length > 3 ? $"...\\{tail}" : $"\\{tail}";
    }
}
