using System.Diagnostics;
using System.IO;

namespace LauncherApp.Services;

public sealed class ExplorerRevealService
{
    public bool Reveal(string path)
    {
        if (string.IsNullOrWhiteSpace(path))
        {
            return false;
        }

        try
        {
            if (Directory.Exists(path))
            {
                Process.Start(new ProcessStartInfo
                {
                    FileName = path,
                    UseShellExecute = true,
                });
                return true;
            }

            if (File.Exists(path))
            {
                Process.Start(new ProcessStartInfo
                {
                    FileName = "explorer.exe",
                    Arguments = $"/select,\"{path}\"",
                    UseShellExecute = true,
                });
                return true;
            }

            return false;
        }
        catch
        {
            return false;
        }
    }
}
