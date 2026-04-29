using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Text.Json;
using System.Threading.Tasks;
using LauncherApp.Bridge;

namespace LauncherApp.Services;

// Rust doesn't enumerate shell:AppsFolder (no equivalent of macOS Spotlight on Windows),
// so we walk it once via Shell.Application COM at app start and forward each entry to
// the Rust candidates table via look_seed_uwp_apps_json. After that, the Rust engine
// owns ranking, scoring, use_count, and recency for UWP entries — there's no parallel
// C# scoring path. The seed is idempotent (upsert preserves use_count via ON CONFLICT)
// so re-running on every launch is safe and refreshes the install list cheaply.
public sealed class UwpAppService
{
    private static readonly string LogPath = Path.Combine(Path.GetTempPath(), "look-open.log");

    public void BeginInitialize()
    {
        Task.Run(() =>
        {
            try
            {
                var entries = EnumerateAppsFolder();
                Log($"UwpAppService.BeginInitialize: enumerated {entries.Count} apps");
                if (entries.Count == 0)
                {
                    return;
                }

                string json = JsonSerializer.Serialize(entries);
                Log($"UwpAppService.BeginInitialize: json length={json.Length} preview={json.Substring(0, Math.Min(json.Length, 240))}");

                bool ok = false;
                try
                {
                    ok = FfiBindings.look_seed_uwp_apps_json(json);
                }
                catch (Exception ex)
                {
                    Log($"UwpAppService.BeginInitialize: seed FFI threw: {ex.GetType().Name} {ex.Message}");
                }

                Log($"UwpAppService.BeginInitialize: seeded {entries.Count} apps ok={ok}");
            }
            catch (Exception ex)
            {
                Log($"UwpAppService.BeginInitialize: failed {ex.GetType().Name} {ex.Message}");
            }
        });
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

    private static List<UwpAppEntry> EnumerateAppsFolder()
    {
        var results = new List<UwpAppEntry>();

        Type? shellType = Type.GetTypeFromProgID("Shell.Application");
        if (shellType is null)
        {
            Debug.WriteLine("[UwpAppService] Shell.Application ProgID not found");
            return results;
        }

        dynamic? shell = null;
        try
        {
            shell = Activator.CreateInstance(shellType);
            if (shell is null)
            {
                return results;
            }

            dynamic appsFolder = shell.NameSpace("shell:AppsFolder");
            dynamic items = appsFolder.Items();
            int count = (int)items.Count;

            var seenAumids = new HashSet<string>(StringComparer.OrdinalIgnoreCase);

            for (int i = 0; i < count; i++)
            {
                try
                {
                    dynamic item = items.Item(i);
                    string? name = item.Name as string;
                    string? path = item.Path as string;

                    if (string.IsNullOrWhiteSpace(name) || string.IsNullOrWhiteSpace(path))
                    {
                        continue;
                    }

                    // AUMIDs contain "!" separating PackageFamilyName from AppId.
                    // Entries without "!" are Win32 shortcuts already indexed by the
                    // Rust backend's Start Menu scan and shouldn't be duplicated here.
                    if (!path.Contains('!'))
                    {
                        continue;
                    }

                    if (!seenAumids.Add(path))
                    {
                        continue;
                    }

                    // Anonymous type with lowercase fields → JSON exactly matches the
                    // serde keys in bridge/ffi/src/seed_api.rs (`aumid`, `title`). Avoids
                    // any chance of a default-options PascalCase mismatch (`Aumid`/`Title`)
                    // that would silently make the Rust deserializer drop every entry.
                    results.Add(new UwpAppEntry { aumid = path, title = name });
                }
                catch (Exception ex)
                {
                    Debug.WriteLine($"[UwpAppService] item {i} failed: {ex.Message}");
                }
            }
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"[UwpAppService] enumeration failed: {ex.Message}");
        }
        finally
        {
            if (shell is not null)
            {
                try
                {
                    Marshal.FinalReleaseComObject(shell);
                }
                catch
                {
                }
            }
        }

        return results;
    }

    // Public so System.Text.Json's reflection serializer always sees it regardless of
    // assembly access modifiers, and lowercase property names so the emitted JSON keys
    // exactly match seed_api::UwpAppPayload's serde fields with no naming-policy assumptions.
    public sealed class UwpAppEntry
    {
        public string aumid { get; set; } = string.Empty;
        public string title { get; set; } = string.Empty;
    }
}
