using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace LauncherApp.Views;

public sealed partial class KeyChordRowView : UserControl
{
    public KeyChordRowView()
    {
        InitializeComponent();
    }

    public static readonly DependencyProperty KeyTextProperty = DependencyProperty.Register(
        nameof(KeyText),
        typeof(string),
        typeof(KeyChordRowView),
        new PropertyMetadata(string.Empty, OnKeyTextChanged));

    public string KeyText
    {
        get => (string)GetValue(KeyTextProperty);
        set => SetValue(KeyTextProperty, value);
    }

    public static readonly DependencyProperty DescriptionProperty = DependencyProperty.Register(
        nameof(Description),
        typeof(string),
        typeof(KeyChordRowView),
        new PropertyMetadata(string.Empty, OnDescriptionChanged));

    public string Description
    {
        get => (string)GetValue(DescriptionProperty);
        set => SetValue(DescriptionProperty, value);
    }

    public static readonly DependencyProperty KeyColumnWidthProperty = DependencyProperty.Register(
        nameof(KeyColumnWidth),
        typeof(double),
        typeof(KeyChordRowView),
        new PropertyMetadata(110.0, OnKeyColumnWidthChanged));

    public double KeyColumnWidth
    {
        get => (double)GetValue(KeyColumnWidthProperty);
        set => SetValue(KeyColumnWidthProperty, value);
    }

    public static readonly DependencyProperty IsPillProperty = DependencyProperty.Register(
        nameof(IsPill),
        typeof(bool),
        typeof(KeyChordRowView),
        new PropertyMetadata(true, OnIsPillChanged));

    public bool IsPill
    {
        get => (bool)GetValue(IsPillProperty);
        set => SetValue(IsPillProperty, value);
    }

    private static void OnKeyTextChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
    {
        if (d is KeyChordRowView view)
        {
            string text = (string)e.NewValue ?? string.Empty;
            view.KeyTextPill.Text = text;
            view.KeyTextPlain.Text = text;
        }
    }

    private static void OnDescriptionChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
    {
        if (d is KeyChordRowView view)
        {
            view.DescriptionText.Text = (string)e.NewValue ?? string.Empty;
        }
    }

    private static void OnKeyColumnWidthChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
    {
        if (d is KeyChordRowView view && e.NewValue is double width)
        {
            view.KeyColumn.Width = new GridLength(width);
        }
    }

    private static void OnIsPillChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
    {
        if (d is KeyChordRowView view && e.NewValue is bool isPill)
        {
            view.KeyPill.Visibility = isPill ? Visibility.Visible : Visibility.Collapsed;
            view.KeyTextPlain.Visibility = isPill ? Visibility.Collapsed : Visibility.Visible;
        }
    }
}
