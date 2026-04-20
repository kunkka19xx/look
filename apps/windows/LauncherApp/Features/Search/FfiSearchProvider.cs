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
            catch (Exception ex)
            {
                Debug.WriteLine($"[FfiSearchProvider] Error: {ex.Message}");
            }
        }
    }

    public IReadOnlyList<LauncherResult> Search(string query, int limit)
    {
        return _engineBridge.Search(query, limit);
    }
}
