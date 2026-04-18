using System.Collections.Generic;
using LauncherApp.Bridge;

namespace LauncherApp.Features.Search;

public sealed class LauncherSearchLogic
{
    private readonly ISearchProvider _searchProvider;

    public LauncherSearchLogic(ISearchProvider searchProvider)
    {
        _searchProvider = searchProvider;
    }

    public IReadOnlyList<LauncherResult> Search(string query, int limit = 40)
    {
        return _searchProvider.Search(query, limit);
    }
}
