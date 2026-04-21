using System;
using System.Diagnostics;
using System.Threading.Tasks;

namespace LauncherApp.Commands;

public static class ShellCommand
{
    public static async Task<(bool ok, string message)> RunAsync(string command)
    {
        if (string.IsNullOrWhiteSpace(command))
            return (false, "Usage: /shell <command>");

        try
        {
            using var process = new Process();
            process.StartInfo = new ProcessStartInfo
            {
                FileName = "cmd.exe",
                Arguments = "/C " + command,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false,
                CreateNoWindow = true,
            };

            process.Start();
            string stdout = await process.StandardOutput.ReadToEndAsync();
            string stderr = await process.StandardError.ReadToEndAsync();
            await process.WaitForExitAsync();

            string merged = (stdout + Environment.NewLine + stderr).Trim();
            if (string.IsNullOrWhiteSpace(merged))
                return process.ExitCode == 0 ? (true, "Done") : (false, "Error: command failed");

            string clipped = merged.Length > 800 ? merged[..800] + "..." : merged;
            return process.ExitCode == 0 ? (true, clipped) : (false, "Error: " + clipped);
        }
        catch (Exception ex)
        {
            return (false, "Error: " + ex.Message);
        }
    }
}
