using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;

namespace LauncherApp.Services;

public static class LookConfig
{
    public static string ResolvePath()
    {
        string? custom = Environment.GetEnvironmentVariable("LOOK_CONFIG_PATH");
        if (!string.IsNullOrWhiteSpace(custom))
        {
            return custom;
        }

        string profile = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
        return Path.Combine(profile, ".look.config");
    }

    public static Dictionary<string, string> Read()
    {
        var values = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        string path = ResolvePath();
        if (!File.Exists(path))
        {
            return values;
        }

        foreach (string rawLine in File.ReadAllLines(path))
        {
            string line = StripComment(rawLine).Trim();
            if (line.Length == 0)
            {
                continue;
            }

            int split = line.IndexOf('=');
            if (split <= 0)
            {
                continue;
            }

            values[line[..split].Trim()] = line[(split + 1)..].Trim();
        }

        return values;
    }

    public static string? Get(string key)
    {
        return Read().GetValueOrDefault(key);
    }

    public static bool GetBool(string key, bool fallback)
    {
        return TryParseBool(Get(key), out bool parsed) ? parsed : fallback;
    }

    public static void SetBool(string key, bool value)
    {
        Upsert(key, value ? "true" : "false");
    }

    public static void Upsert(string key, string value)
    {
        string path = ResolvePath();
        string? dir = Path.GetDirectoryName(path);
        if (!string.IsNullOrWhiteSpace(dir))
        {
            Directory.CreateDirectory(dir);
        }

        List<string> lines = File.Exists(path)
            ? File.ReadAllLines(path).ToList()
            : [];

        string wanted = key + "=";
        bool replaced = false;
        for (int i = 0; i < lines.Count; i++)
        {
            string trimmed = StripComment(lines[i]).Trim();
            if (trimmed.StartsWith(wanted, StringComparison.OrdinalIgnoreCase))
            {
                lines[i] = key + "=" + value;
                replaced = true;
                break;
            }
        }

        if (!replaced)
        {
            lines.Add(key + "=" + value);
        }

        File.WriteAllText(path, string.Join("\n", lines) + "\n");
    }

    private static string StripComment(string line)
    {
        int idx = line.IndexOf('#');
        return idx >= 0 ? line[..idx] : line;
    }

    private static bool TryParseBool(string? raw, out bool value)
    {
        switch (raw?.Trim().ToLowerInvariant())
        {
            case "1":
            case "true":
            case "yes":
            case "on":
                value = true;
                return true;
            case "0":
            case "false":
            case "no":
            case "off":
                value = false;
                return true;
            default:
                value = false;
                return false;
        }
    }
}
