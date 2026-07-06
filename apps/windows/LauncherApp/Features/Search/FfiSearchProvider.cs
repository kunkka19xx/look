using System;
using System.Collections.Generic;
using LauncherApp.Bridge;
using System.Diagnostics;

namespace LauncherApp.Features.Search;

public sealed class FfiSearchProvider : ISearchProvider
{
    private readonly EngineBridge _engineBridge;
    private static bool _initialized;

    public FfiSearchProvider(EngineBridge engineBridge)
    {
        _engineBridge = engineBridge;
        if (!_initialized)
        {
            _initialized = true;
            try
            {
                Debug.WriteLine("[FfiSearchProvider] Initializing FFI...");
                bool reloadResult = FfiBindings.look_reload_config();
                Debug.WriteLine($"[FfiSearchProvider] look_reload_config: {reloadResult}");

                bool refreshResult = FfiBindings.look_request_index_refresh();
                Debug.WriteLine($"[FfiSearchProvider] look_request_index_refresh: {refreshResult}");
            }
            catch (DllNotFoundException ex)
            {
                // Most common cause on a fresh Windows install: look_ffi.dll loaded but
                // failed to resolve VCRUNTIME140.dll / MSVCP140.dll because the Visual
                // C++ Redistributable isn't installed. Without this log, every search
                // silently returns [] and look.db never gets created - see
                // EngineBridge.Search where the same exception type is caught per-call.
                App.WriteCrashLog(
                    "FfiSearchProvider.Init",
                    new Exception(
                        "look_ffi.dll failed to load. Likely missing Microsoft Visual C++ Redistributable (vcruntime140.dll). " +
                        "Original: " + ex.Message,
                        ex));
                Debug.WriteLine($"[FfiSearchProvider] DllNotFound: {ex.Message}");
            }
            catch (BadImageFormatException ex)
            {
                // Architecture mismatch (e.g. ARM64 host loading an x64 dll) lands here
                // instead of DllNotFoundException - also worth surfacing.
                App.WriteCrashLog("FfiSearchProvider.Init", ex);
                Debug.WriteLine($"[FfiSearchProvider] BadImageFormat: {ex.Message}");
            }
            catch (Exception ex)
            {
                App.WriteCrashLog("FfiSearchProvider.Init", ex);
                Debug.WriteLine($"[FfiSearchProvider] Error: {ex.Message}");
            }
        }
    }

    public IReadOnlyList<LauncherResult> Search(string query, int limit)
    {
        return _engineBridge.Search(query, limit);
    }
}
