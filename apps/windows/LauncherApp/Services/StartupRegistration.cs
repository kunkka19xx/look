using System;
using System.Diagnostics;
using Microsoft.Win32;

namespace LauncherApp.Services;

public static class StartupRegistration
{
    private const string RunKeyPath = @"Software\Microsoft\Windows\CurrentVersion\Run";
    private const string ValueName = "LookLauncher";

    public static void Sync(bool enabled)
    {
        try
        {
            using RegistryKey? key = Registry.CurrentUser.OpenSubKey(RunKeyPath, writable: true)
                ?? Registry.CurrentUser.CreateSubKey(RunKeyPath, writable: true);
            if (key is null)
            {
                return;
            }

            if (enabled)
            {
                string exePath = ResolveExecutablePath();
                if (string.IsNullOrWhiteSpace(exePath))
                {
                    return;
                }
                key.SetValue(ValueName, $"\"{exePath}\"", RegistryValueKind.String);
            }
            else
            {
                key.DeleteValue(ValueName, throwOnMissingValue: false);
            }
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"[StartupRegistration] Sync({enabled}) failed: {ex.Message}");
        }
    }

    public static bool IsRegistered()
    {
        try
        {
            using RegistryKey? key = Registry.CurrentUser.OpenSubKey(RunKeyPath);
            return key?.GetValue(ValueName) is string value && !string.IsNullOrWhiteSpace(value);
        }
        catch
        {
            return false;
        }
    }

    private static string ResolveExecutablePath()
    {
        string? envPath = Environment.ProcessPath;
        if (!string.IsNullOrWhiteSpace(envPath))
        {
            return envPath;
        }

        try
        {
            return Process.GetCurrentProcess().MainModule?.FileName ?? string.Empty;
        }
        catch
        {
            return string.Empty;
        }
    }
}
