using System;
using LauncherApp.Services;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace LauncherApp.Views;

public sealed partial class TranslatePanelView : UserControl
{
    private string _currentOriginal = string.Empty;

    public event EventHandler<string>? OpenInBrowserRequested;
    public event EventHandler<string>? CopyTranslatedRequested;

    public TranslatePanelView()
    {
        InitializeComponent();
        ViSection.CopyRequested += ForwardCopy;
        EnSection.CopyRequested += ForwardCopy;
        JaSection.CopyRequested += ForwardCopy;
    }

    public void ShowEmptyPrompt()
    {
        _currentOriginal = string.Empty;
        OriginalText.Text = "Type text after t\"";
        StatusText.Text = string.Empty;
        ResetSections();
        OpenInBrowserButton.Visibility = Visibility.Collapsed;
    }

    public void ShowReadyPrompt(string original)
    {
        _currentOriginal = original;
        OriginalText.Text = original;
        StatusText.Text = "Press Enter to translate";
        ResetSections();
        OpenInBrowserButton.Visibility = Visibility.Visible;
    }

    public void ShowLoading(string original)
    {
        _currentOriginal = original;
        OriginalText.Text = original;
        StatusText.Text = "Translating…";
        ApplyToAllSections(section =>
        {
            section.SetBody("…");
            section.ClearTranslated();
        });
        OpenInBrowserButton.Visibility = Visibility.Visible;
    }

    public void ShowResults(TranslationResultSet results)
    {
        _currentOriginal = results.Original;
        OriginalText.Text = results.Original;

        StatusText.Text = !results.HasAnyResult && !string.IsNullOrEmpty(results.FirstErrorMessage)
            ? results.FirstErrorMessage!
            : string.Empty;

        ApplySection(ViSection, results.Vi);
        ApplySection(EnSection, results.En);
        ApplySection(JaSection, results.Ja);

        OpenInBrowserButton.Visibility = Visibility.Visible;
    }

    private void OpenInBrowserButton_Click(object sender, RoutedEventArgs e)
    {
        if (string.IsNullOrWhiteSpace(_currentOriginal))
        {
            return;
        }
        OpenInBrowserRequested?.Invoke(this, _currentOriginal);
    }

    private void ForwardCopy(object? sender, string text)
    {
        if (string.IsNullOrWhiteSpace(text))
        {
            return;
        }
        CopyTranslatedRequested?.Invoke(this, text);
    }

    private void ResetSections()
    {
        ApplyToAllSections(section =>
        {
            section.SetBody("-");
            section.ClearTranslated();
        });
    }

    private void ApplyToAllSections(Action<TranslateLanguageSectionView> action)
    {
        action(ViSection);
        action(EnSection);
        action(JaSection);
    }

    private static void ApplySection(TranslateLanguageSectionView section, TranslationLanguageResult result)
    {
        if (!string.IsNullOrWhiteSpace(result.Translated))
        {
            section.SetBody(result.Translated!);
            section.SetTranslated(result.Translated);
            return;
        }

        if (!string.IsNullOrWhiteSpace(result.ErrorMessage))
        {
            section.SetBody(result.ErrorMessage!);
        }
        else
        {
            section.SetBody("-");
        }
        section.ClearTranslated();
    }
}
