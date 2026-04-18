using System.Collections.Generic;
using LauncherApp.Bridge;

namespace LauncherApp.Features.Search;

public interface ISearchProvider
{
    IReadOnlyList<LauncherResult> Search(string query, int limit);
}
