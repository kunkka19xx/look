using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;

namespace LauncherApp.Views;

public sealed partial class LabeledSliderView : UserControl
{
    public LabeledSliderView()
    {
        InitializeComponent();
        InnerSlider.ValueChanged += (s, e) => ValueChanged?.Invoke(this, e);
    }

    public event RangeBaseValueChangedEventHandler? ValueChanged;

    public static readonly DependencyProperty LabelProperty = DependencyProperty.Register(
        nameof(Label),
        typeof(string),
        typeof(LabeledSliderView),
        new PropertyMetadata(string.Empty, OnLabelChanged));

    public string Label
    {
        get => (string)GetValue(LabelProperty);
        set => SetValue(LabelProperty, value);
    }

    private static void OnLabelChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
    {
        if (d is LabeledSliderView view)
        {
            view.LabelText.Text = (string)e.NewValue ?? string.Empty;
        }
    }

    public static readonly DependencyProperty LabelColumnWidthProperty = DependencyProperty.Register(
        nameof(LabelColumnWidth),
        typeof(double),
        typeof(LabeledSliderView),
        new PropertyMetadata(170.0, OnLabelColumnWidthChanged));

    public double LabelColumnWidth
    {
        get => (double)GetValue(LabelColumnWidthProperty);
        set => SetValue(LabelColumnWidthProperty, value);
    }

    private static void OnLabelColumnWidthChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
    {
        if (d is LabeledSliderView view && e.NewValue is double width)
        {
            view.LabelColumn.Width = new GridLength(width);
        }
    }

    public double Minimum
    {
        get => InnerSlider.Minimum;
        set => InnerSlider.Minimum = value;
    }

    public double Maximum
    {
        get => InnerSlider.Maximum;
        set => InnerSlider.Maximum = value;
    }

    public double StepFrequency
    {
        get => InnerSlider.StepFrequency;
        set => InnerSlider.StepFrequency = value;
    }

    public double Value
    {
        get => InnerSlider.Value;
        set => InnerSlider.Value = value;
    }
}
