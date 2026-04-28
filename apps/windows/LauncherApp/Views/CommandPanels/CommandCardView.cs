using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;

namespace LauncherApp.Views.CommandPanels;

public sealed class CommandCardView : ToggleButton
{
    private readonly TextBlock _titleText;
    private readonly TextBlock _subtitleText;

    public CommandCardView()
    {
        MinHeight = 44;
        HorizontalAlignment = HorizontalAlignment.Stretch;
        HorizontalContentAlignment = HorizontalAlignment.Left;

        _titleText = new TextBlock
        {
            FontWeight = Microsoft.UI.Text.FontWeights.SemiBold,
        };
        // Without an explicit Foreground the title falls through to ToggleButton's
        // default brush (a generic theme-resource), which on dark themes reads almost
        // identical to the muted subtitle below it. Bind to LauncherTextBrush so the
        // command-card title sits in the same primary tier as app-list row titles.
        if (Application.Current.Resources.TryGetValue("LauncherTextBrush", out object titleBrushObj)
            && titleBrushObj is Microsoft.UI.Xaml.Media.Brush titleBrush)
        {
            _titleText.Foreground = titleBrush;
        }

        _subtitleText = new TextBlock
        {
            FontSize = 12,
        };
        if (Application.Current.Resources.TryGetValue("LauncherMutedTextBrush", out object mutedBrush)
            && mutedBrush is Microsoft.UI.Xaml.Media.Brush brush)
        {
            _subtitleText.Foreground = brush;
        }

        Content = new StackPanel
        {
            Spacing = 1,
            Children =
            {
                _titleText,
                _subtitleText,
            },
        };
    }

    public static readonly DependencyProperty TitleProperty = DependencyProperty.Register(
        nameof(Title),
        typeof(string),
        typeof(CommandCardView),
        new PropertyMetadata(string.Empty, OnTitleChanged));

    public string Title
    {
        get => (string)GetValue(TitleProperty);
        set => SetValue(TitleProperty, value);
    }

    private static void OnTitleChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
    {
        if (d is CommandCardView card)
        {
            card._titleText.Text = (string)e.NewValue ?? string.Empty;
        }
    }

    public static readonly DependencyProperty SubtitleProperty = DependencyProperty.Register(
        nameof(Subtitle),
        typeof(string),
        typeof(CommandCardView),
        new PropertyMetadata(string.Empty, OnSubtitleChanged));

    public string Subtitle
    {
        get => (string)GetValue(SubtitleProperty);
        set => SetValue(SubtitleProperty, value);
    }

    private static void OnSubtitleChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
    {
        if (d is CommandCardView card)
        {
            card._subtitleText.Text = (string)e.NewValue ?? string.Empty;
        }
    }
}
