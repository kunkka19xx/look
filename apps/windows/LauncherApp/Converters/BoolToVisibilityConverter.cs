using System;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Data;

namespace LauncherApp.Converters;

public sealed class BoolToVisibilityConverter : IValueConverter
{
    public bool Invert { get; set; }

    public object Convert(object value, Type targetType, object parameter, string language)
    {
        bool flag = value is bool b && b;
        if (Invert)
            flag = !flag;
        return flag ? Visibility.Visible : Visibility.Collapsed;
    }

    public object ConvertBack(object value, Type targetType, object parameter, string language)
    {
        bool visible = value is Visibility v && v == Visibility.Visible;
        if (Invert)
            visible = !visible;
        return visible;
    }
}
