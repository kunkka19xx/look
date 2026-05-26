namespace LauncherApp.Core;

public sealed class AppCommandItem
{
    public string Id { get; init; } = string.Empty;
    public string Name { get; init; } = string.Empty;
    public string Summary { get; init; } = string.Empty;
    public string Hint { get; init; } = string.Empty;
    public string DangerLevel { get; init; } = "normal";
}
