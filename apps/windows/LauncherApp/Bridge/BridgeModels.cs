using System.Collections.Generic;
using System.Text.Json.Serialization;

namespace LauncherApp.Bridge;

public sealed class BridgeError
{
    [JsonPropertyName("code")]
    public string Code { get; set; } = string.Empty;

    [JsonPropertyName("message")]
    public string Message { get; set; } = string.Empty;
}

public sealed class CompactSearchPayload
{
    [JsonPropertyName("count")]
    public int Count { get; set; }

    [JsonPropertyName("results")]
    public List<SearchItem> Results { get; set; } = new();

    [JsonPropertyName("error")]
    public BridgeError? Error { get; set; }
}

public sealed class SearchItem
{
    [JsonPropertyName("id")]
    public string Id { get; set; } = string.Empty;

    [JsonPropertyName("kind")]
    public string Kind { get; set; } = string.Empty;

    [JsonPropertyName("title")]
    public string Title { get; set; } = string.Empty;

    [JsonPropertyName("subtitle")]
    public string? Subtitle { get; set; }

    [JsonPropertyName("path")]
    public string Path { get; set; } = string.Empty;

    [JsonPropertyName("score")]
    public int Score { get; set; }
}

public sealed class LauncherResult
{
    public string Id { get; init; } = string.Empty;
    public string Kind { get; init; } = string.Empty;
    public string Title { get; init; } = string.Empty;
    public string? Subtitle { get; init; }
    public string Path { get; init; } = string.Empty;
    public int Score { get; init; }
}

public sealed class TranslatePayload
{
    [JsonPropertyName("original")]
    public string Original { get; set; } = string.Empty;

    [JsonPropertyName("translated")]
    public string Translated { get; set; } = string.Empty;

    [JsonPropertyName("error")]
    public BridgeError? Error { get; set; }
}
