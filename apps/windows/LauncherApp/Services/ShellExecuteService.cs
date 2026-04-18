using System.Diagnostics;

namespace LauncherApp.Services;

public sealed class ShellExecuteService
{
    public bool Open(string target)
    {
        if (string.IsNullOrWhiteSpace(target))
        {
            return false;
        }

        try
        {
            Process.Start(new ProcessStartInfo
            {
                FileName = target,
                UseShellExecute = true,
            });
            return true;
        }
        catch
        {
            return false;
        }
    }
}
