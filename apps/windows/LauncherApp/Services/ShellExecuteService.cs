using System.Diagnostics;
using System;
using System.IO;

namespace LauncherApp.Services;

public sealed class ShellExecuteService
{
    private static readonly string LogPath = Path.Combine(Path.GetTempPath(), "look-open.log");

    public bool Open(string target, string? arguments = null)
    {
        if (string.IsNullOrWhiteSpace(target))
        {
            Log("Open skipped: empty target");
            return false;
        }

        try
        {
            string normalizedTarget = NormalizePathLikeTarget(target);
            string resolvedTarget = normalizedTarget;
            string resolvedArguments = arguments ?? string.Empty;

            if (resolvedTarget.StartsWith("ms-settings:", StringComparison.OrdinalIgnoreCase)
                || resolvedTarget.StartsWith("shell:", StringComparison.OrdinalIgnoreCase))
            {
                resolvedArguments = resolvedTarget;
                resolvedTarget = "explorer.exe";
            }

            var info = new ProcessStartInfo
            {
                FileName = resolvedTarget,
                Arguments = resolvedArguments,
                UseShellExecute = true,
            };

            Process.Start(info);
            Log($"Open ok: file='{resolvedTarget}' args='{resolvedArguments}' src='{target}'");
            return true;
        }
        catch (Exception ex)
        {
            Log($"Open fail: target='{target}' args='{arguments ?? string.Empty}' ex={ex.GetType().Name} msg={ex.Message}");
            return false;
        }
    }

    private static string NormalizePathLikeTarget(string target)
    {
        string trimmed = target.Trim();
        if (trimmed.StartsWith("http://", StringComparison.OrdinalIgnoreCase)
            || trimmed.StartsWith("https://", StringComparison.OrdinalIgnoreCase)
            || trimmed.StartsWith("ms-settings:", StringComparison.OrdinalIgnoreCase)
            || trimmed.StartsWith("shell:", StringComparison.OrdinalIgnoreCase)
            || trimmed.StartsWith("command://", StringComparison.OrdinalIgnoreCase)
            || trimmed.StartsWith("help://", StringComparison.OrdinalIgnoreCase))
        {
            return trimmed;
        }

        string normalized = trimmed.Replace('/', '\\');
        if (normalized.EndsWith("\\") && normalized.Length > 3)
            normalized = normalized.TrimEnd('\\');

        return normalized;
    }

    private static void Log(string message)
    {
        try
        {
            File.AppendAllText(LogPath, $"[{DateTime.Now:HH:mm:ss.fff}] {message}{Environment.NewLine}");
        }
        catch
        {
        }
    }
}
