using System;
using System.Data;
using System.Globalization;

namespace LauncherApp.Commands;

public static class CalcCommand
{
    public static bool TryEvaluate(string expression, out string message)
    {
        message = "Invalid expression";
        if (string.IsNullOrWhiteSpace(expression))
            return false;

        string normalized = expression
            .Trim()
            .Replace('x', '*')
            .Replace('X', '*')
            .Replace(':', '/');

        try
        {
            var table = new DataTable();
            object raw = table.Compute(normalized, string.Empty);
            double value = Convert.ToDouble(raw, CultureInfo.InvariantCulture);
            if (double.IsNaN(value) || double.IsInfinity(value))
            {
                message = "Invalid expression";
                return false;
            }

            message = $"Result: {value:0.####}";
            return true;
        }
        catch
        {
            return false;
        }
    }
}
