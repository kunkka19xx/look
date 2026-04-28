using System;
using Microsoft.UI.Xaml;

namespace LauncherApp;

// Mirrors macOS LauncherView.cachedShouldShowTestHint: shows a red "TEST APP"
// pill in the top-right when LOOK_DEV_HINT is truthy. `make app-run-dev` sets
// the env var; production launches leave it unset so the badge stays hidden.
public sealed partial class MainWindow
{
    private static readonly string[] DevHintTruthyValues = ["1", "true", "yes", "on"];

    private void InitializeDevHintBadge()
    {
        string? raw = Environment.GetEnvironmentVariable("LOOK_DEV_HINT");
        if (string.IsNullOrWhiteSpace(raw))
        {
            return;
        }

        string normalized = raw.Trim().ToLowerInvariant();
        if (Array.IndexOf(DevHintTruthyValues, normalized) >= 0)
        {
            DevHintBadge.Visibility = Visibility.Visible;
        }
    }
}
