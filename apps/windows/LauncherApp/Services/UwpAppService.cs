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
// so we walk it once via direct shell COM at app start and forward each entry to
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

    // Walks shell:AppsFolder via direct IShellItem / IEnumShellItems COM. The previous
    // implementation used `dynamic` over the Shell.Application IDispatch object; the
    // .NET 10 trimmer strips the Microsoft.CSharp.RuntimeBinder.ComInterop ITypeInfo
    // marshaling stubs even with BuiltInComInteropSupport=true and TrimmerRootAssembly
    // on Microsoft.CSharp, which surfaces as a 0xC0000005 access violation on the
    // first dynamic property access. Early-bound [ComImport] interfaces sidestep the
    // runtime binder entirely and are trim-safe.
    private static List<UwpAppEntry> EnumerateAppsFolder()
    {
        var results = new List<UwpAppEntry>();

        IShellItem? appsFolder = null;
        IEnumShellItems? enumerator = null;
        try
        {
            Guid iidShellItem = IID_IShellItem;
            int hr = SHCreateItemFromParsingName("shell:AppsFolder", IntPtr.Zero, ref iidShellItem, out object? folderObj);
            if (hr != 0 || folderObj is not IShellItem folder)
            {
                Debug.WriteLine($"[UwpAppService] SHCreateItemFromParsingName(shell:AppsFolder) hr=0x{hr:X}");
                return results;
            }
            appsFolder = folder;

            Guid bhidEnumItems = BHID_EnumItems;
            Guid iidEnumShellItems = IID_IEnumShellItems;
            hr = appsFolder.BindToHandler(IntPtr.Zero, ref bhidEnumItems, ref iidEnumShellItems, out object? enumObj);
            if (hr != 0 || enumObj is not IEnumShellItems iter)
            {
                Debug.WriteLine($"[UwpAppService] BindToHandler(BHID_EnumItems) hr=0x{hr:X}");
                return results;
            }
            enumerator = iter;

            var seenAumids = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
            IShellItem[] buffer = new IShellItem[1];

            while (true)
            {
                hr = enumerator.Next(1, buffer, out uint fetched);
                if (hr != 0 || fetched == 0)
                {
                    break;
                }

                IShellItem item = buffer[0];
                buffer[0] = null!;
                try
                {
                    string? name = TryGetDisplayName(item, SIGDN_NORMALDISPLAY);
                    string? aumid = TryGetDisplayName(item, SIGDN_PARENTRELATIVEPARSING);

                    if (string.IsNullOrWhiteSpace(name) || string.IsNullOrWhiteSpace(aumid))
                    {
                        continue;
                    }

                    // Strip the "shell:AppsFolder\" prefix if some shell provider hands us
                    // back a desktop-absolute path instead of the parent-relative one.
                    const string prefix = "shell:AppsFolder\\";
                    if (aumid.StartsWith(prefix, StringComparison.OrdinalIgnoreCase))
                    {
                        aumid = aumid.Substring(prefix.Length);
                    }

                    // AUMIDs contain "!" separating PackageFamilyName from AppId.
                    // Entries without "!" are Win32 shortcuts already indexed by the
                    // Rust backend's Start Menu scan and shouldn't be duplicated here.
                    if (!aumid.Contains('!'))
                    {
                        continue;
                    }

                    if (!seenAumids.Add(aumid))
                    {
                        continue;
                    }

                    results.Add(new UwpAppEntry { aumid = aumid, title = name });
                }
                catch (Exception ex)
                {
                    Debug.WriteLine($"[UwpAppService] item read failed: {ex.Message}");
                }
                finally
                {
                    Marshal.ReleaseComObject(item);
                }
            }
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"[UwpAppService] enumeration failed: {ex.Message}");
        }
        finally
        {
            if (enumerator is not null)
            {
                try { Marshal.ReleaseComObject(enumerator); } catch { }
            }
            if (appsFolder is not null)
            {
                try { Marshal.ReleaseComObject(appsFolder); } catch { }
            }
        }

        return results;
    }

    private static string? TryGetDisplayName(IShellItem item, uint sigdnForm)
    {
        IntPtr pszName = IntPtr.Zero;
        try
        {
            int hr = item.GetDisplayName(sigdnForm, out pszName);
            if (hr != 0 || pszName == IntPtr.Zero)
            {
                return null;
            }
            return Marshal.PtrToStringUni(pszName);
        }
        catch
        {
            return null;
        }
        finally
        {
            if (pszName != IntPtr.Zero)
            {
                Marshal.FreeCoTaskMem(pszName);
            }
        }
    }

    // Public so System.Text.Json's reflection serializer always sees it regardless of
    // assembly access modifiers, and lowercase property names so the emitted JSON keys
    // exactly match seed_api::UwpAppPayload's serde fields with no naming-policy assumptions.
    public sealed class UwpAppEntry
    {
        public string aumid { get; set; } = string.Empty;
        public string title { get; set; } = string.Empty;
    }

    private static readonly Guid IID_IShellItem = new("43826D1E-E718-42EE-BC55-A1E261C37BFE");
    private static readonly Guid IID_IEnumShellItems = new("70629033-E363-4A28-A567-0DB78006E6D7");
    private static readonly Guid BHID_EnumItems = new("94f60519-2850-4924-aa5a-d15e84868039");

    private const uint SIGDN_NORMALDISPLAY = 0x00000000;
    private const uint SIGDN_PARENTRELATIVEPARSING = 0x80018001;

    [DllImport("shell32.dll", CharSet = CharSet.Unicode, ExactSpelling = true, PreserveSig = true)]
    private static extern int SHCreateItemFromParsingName(
        [MarshalAs(UnmanagedType.LPWStr)] string pszPath,
        IntPtr pbc,
        [In] ref Guid riid,
        [MarshalAs(UnmanagedType.Interface)] out object? ppv);

    [ComImport]
    [Guid("43826D1E-E718-42EE-BC55-A1E261C37BFE")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private interface IShellItem
    {
        [PreserveSig]
        int BindToHandler(IntPtr pbc, [In] ref Guid bhid, [In] ref Guid riid, [MarshalAs(UnmanagedType.Interface)] out object? ppv);
        [PreserveSig]
        int GetParent(out IShellItem ppsi);
        [PreserveSig]
        int GetDisplayName(uint sigdnName, out IntPtr ppszName);
        [PreserveSig]
        int GetAttributes(uint sfgaoMask, out uint psfgaoAttribs);
        [PreserveSig]
        int Compare(IShellItem psi, uint hint, out int piOrder);
    }

    [ComImport]
    [Guid("70629033-E363-4A28-A567-0DB78006E6D7")]
    [InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private interface IEnumShellItems
    {
        [PreserveSig]
        int Next(uint celt, [Out, MarshalAs(UnmanagedType.LPArray, SizeParamIndex = 0)] IShellItem[] rgelt, out uint pceltFetched);
        [PreserveSig]
        int Skip(uint celt);
        [PreserveSig]
        int Reset();
        [PreserveSig]
        int Clone(out IEnumShellItems ppenum);
    }
}
