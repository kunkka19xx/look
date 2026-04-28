using System;
using System.Threading;
using System.Threading.Tasks;
using LauncherApp.Bridge;

namespace LauncherApp.Services;

public sealed class TranslationLanguageResult
{
    public string Language { get; init; } = string.Empty;
    public string Label { get; init; } = string.Empty;
    public string? Translated { get; init; }
    public string? ErrorMessage { get; init; }
}

public sealed class TranslationResultSet
{
    public string Original { get; init; } = string.Empty;
    public TranslationLanguageResult Vi { get; init; } = new();
    public TranslationLanguageResult En { get; init; } = new();
    public TranslationLanguageResult Ja { get; init; } = new();

    public bool HasAnyResult =>
        !string.IsNullOrWhiteSpace(Vi.Translated)
        || !string.IsNullOrWhiteSpace(En.Translated)
        || !string.IsNullOrWhiteSpace(Ja.Translated);

    public string? FirstErrorMessage =>
        Vi.ErrorMessage ?? En.ErrorMessage ?? Ja.ErrorMessage;
}

public sealed class TranslationService
{
    private readonly EngineBridge _bridge;

    public TranslationService(EngineBridge bridge)
    {
        _bridge = bridge;
    }

    public async Task<TranslationResultSet> TranslateAsync(string text, CancellationToken cancellationToken = default)
    {
        string normalized = (text ?? string.Empty).Trim();

        Task<TranslationLanguageResult> viTask = TranslateOneAsync(normalized, "vi", "Tiếng Việt", cancellationToken);
        Task<TranslationLanguageResult> enTask = TranslateOneAsync(normalized, "en", "English", cancellationToken);
        Task<TranslationLanguageResult> jaTask = TranslateOneAsync(normalized, "ja", "日本語", cancellationToken);

        await Task.WhenAll(viTask, enTask, jaTask).ConfigureAwait(false);

        return new TranslationResultSet
        {
            Original = normalized,
            Vi = viTask.Result,
            En = enTask.Result,
            Ja = jaTask.Result,
        };
    }

    private Task<TranslationLanguageResult> TranslateOneAsync(string text, string lang, string label, CancellationToken cancellationToken)
    {
        return Task.Run(() =>
        {
            cancellationToken.ThrowIfCancellationRequested();
            TranslatePayload? payload = _bridge.Translate(text, lang);
            if (payload == null)
            {
                return new TranslationLanguageResult
                {
                    Language = lang,
                    Label = label,
                    Translated = null,
                    ErrorMessage = "Translation failed",
                };
            }

            string? translated = payload.Translated?.Trim();
            bool hasTranslated = !string.IsNullOrEmpty(translated);

            return new TranslationLanguageResult
            {
                Language = lang,
                Label = label,
                Translated = hasTranslated ? translated : null,
                ErrorMessage = payload.Error?.Message,
            };
        }, cancellationToken);
    }
}
