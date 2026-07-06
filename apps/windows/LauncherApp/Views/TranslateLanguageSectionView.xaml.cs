using System;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace LauncherApp.Views;

public sealed partial class TranslateLanguageSectionView : UserControl
{
    private string? _translated;

    public event EventHandler<string>? CopyRequested;

    public TranslateLanguageSectionView()
    {
        InitializeComponent();
    }

    public static readonly DependencyProperty LabelProperty = DependencyProperty.Register(
        nameof(Label),
        typeof(string),
        typeof(TranslateLanguageSectionView),
        new PropertyMetadata(string.Empty, OnLabelChanged));

    public string Label
    {
        get => (string)GetValue(LabelProperty);
        set => SetValue(LabelProperty, value);
    }

    private static void OnLabelChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
    {
        if (d is TranslateLanguageSectionView view)
        {
            view.LabelText.Text = (string)e.NewValue ?? string.Empty;
        }
    }

    public void SetBody(string text)
    {
        BodyText.Text = string.IsNullOrEmpty(text) ? "-" : text;
    }

    public void SetTranslated(string? translated)
    {
        _translated = string.IsNullOrWhiteSpace(translated) ? null : translated!.Trim();
        CopyButton.Visibility = _translated == null ? Visibility.Collapsed : Visibility.Visible;
    }

    public void ClearTranslated()
    {
        _translated = null;
        CopyButton.Visibility = Visibility.Collapsed;
    }

    private void CopyButton_Click(object sender, RoutedEventArgs e)
    {
        if (string.IsNullOrWhiteSpace(_translated))
        {
            return;
        }
        CopyRequested?.Invoke(this, _translated!);
    }
}
