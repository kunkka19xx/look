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
            // Both streams must be drained concurrently. Awaiting stdout to EOF before
            // even starting the stderr read deadlocks any command that writes >~4KB
            // to stderr (Windows pipe buffer size): the child blocks on stderr.Write,
            // we block on stdout EOF that never arrives. Task.WhenAll lets both pipes
            // empty in parallel so the child never stalls on a full buffer.
            Task<string> stdoutTask = process.StandardOutput.ReadToEndAsync();
            Task<string> stderrTask = process.StandardError.ReadToEndAsync();
            await Task.WhenAll(stdoutTask, stderrTask, process.WaitForExitAsync());
            string stdout = await stdoutTask;
            string stderr = await stderrTask;

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
