using System.Collections.Generic;
using LauncherApp.Bridge;

namespace LauncherApp.Features.Search;

public sealed class FfiSearchProvider : ISearchProvider
{
    private readonly EngineBridge _engineBridge;

    public FfiSearchProvider(EngineBridge engineBridge)
    {
        _engineBridge = engineBridge;
    }

    public IReadOnlyList<LauncherResult> Search(string query, int limit)
    {
        return _engineBridge.Search(query, limit);
    }
}
