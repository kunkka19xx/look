using System;
using System.Globalization;

namespace LauncherApp.Commands;

public static class CalcCommand
{
    private const double MaxMagnitude = 1_000_000_000_000.0;

    public static bool TryEvaluate(string expression, out string message)
    {
        message = "Invalid expression";
        if (!IsReadyForEvaluation(expression))
        {
            return false;
        }

        string normalized = NormalizeExpression(expression);
        try
        {
            var parser = new Parser(normalized);
            double value = parser.Parse();

            if (Math.Abs(value) > MaxMagnitude)
            {
                message = "Error: result out of range (+/-1,000,000,000,000)";
                return false;
            }

            message = $"Result: {FormatFloat(value)}";
            return true;
        }
        catch (DivideByZeroException)
        {
            message = "Error: division by zero";
            return false;
        }
        catch
        {
            return false;
        }
    }

    public static bool IsReadyForEvaluation(string expression)
    {
        if (string.IsNullOrWhiteSpace(expression))
        {
            return false;
        }

        string trimmed = expression.Trim();
        int balance = 0;
        foreach (char ch in trimmed)
        {
            if (!IsAllowedChar(ch))
            {
                return false;
            }

            if (ch == '(')
            {
                balance++;
            }
            else if (ch == ')')
            {
                balance--;
                if (balance < 0)
                {
                    return false;
                }
            }
        }

        if (balance != 0)
        {
            return false;
        }

        char last = trimmed[^1];
        if ("+-*/%.(".IndexOf(last) >= 0)
        {
            return false;
        }

        return true;
    }

    private static bool IsAllowedChar(char ch)
    {
        return char.IsLetterOrDigit(ch)
            || ch == '_'
            || ch == '+'
            || ch == '-'
            || ch == '*'
            || ch == '/'
            || ch == '%'
            || ch == '('
            || ch == ')'
            || ch == '.'
            || ch == ':'
            || ch == 'x'
            || ch == 'X'
            || ch == 'v'
            || ch == 'V'
            || char.IsWhiteSpace(ch);
    }

    private static string NormalizeExpression(string expression)
    {
        string normalized = expression
            .Trim()
            .Replace('x', '*')
            .Replace('X', '*')
            .Replace(':', '/');

        return ReplacePrefixSqrt(normalized);
    }

    private static string ReplacePrefixSqrt(string expression)
    {
        var output = new System.Text.StringBuilder(expression.Length + 4);
        for (int i = 0; i < expression.Length; i++)
        {
            char current = expression[i];
            if (current == 'v' || current == 'V')
            {
                char prev = i > 0 ? expression[i - 1] : ' ';
                char next = i + 1 < expression.Length ? expression[i + 1] : ' ';
                bool prevIsWord = char.IsLetterOrDigit(prev) || prev == '_';
                bool nextIsStart = char.IsDigit(next) || next == '.' || next == '(' || char.IsWhiteSpace(next);

                if (!prevIsWord && nextIsStart)
                {
                    output.Append("sqrt");
                    continue;
                }
            }

            output.Append(current);
        }

        return output.ToString();
    }

    private static string FormatFloat(double value)
    {
        if (double.IsNaN(value) || double.IsInfinity(value))
        {
            return "nan";
        }

        var format = (NumberFormatInfo)CultureInfo.GetCultureInfo("en-US").NumberFormat.Clone();
        format.NumberGroupSeparator = ",";
        return value.ToString("N4", format);
    }

    private sealed class Parser
    {
        private readonly char[] _chars;
        private int _index;

        public Parser(string input)
        {
            _chars = input.ToCharArray();
            _index = 0;
        }

        public double Parse()
        {
            double value = ParseExpression();
            SkipWhitespace();
            if (_index != _chars.Length)
            {
                throw new InvalidOperationException("Invalid expression");
            }

            return value;
        }

        private double ParseExpression()
        {
            double value = ParseTerm();
            while (true)
            {
                SkipWhitespace();
                if (Consume('+'))
                {
                    value += ParseTerm();
                }
                else if (Consume('-'))
                {
                    value -= ParseTerm();
                }
                else
                {
                    return value;
                }
            }
        }

        private double ParseTerm()
        {
            double value = ParseFactor();
            while (true)
            {
                SkipWhitespace();
                if (Consume('*'))
                {
                    value *= ParseFactor();
                }
                else if (Consume('/'))
                {
                    double divisor = ParseFactor();
                    if (divisor == 0)
                    {
                        throw new DivideByZeroException();
                    }

                    value /= divisor;
                }
                else if (Consume('%'))
                {
                    double divisor = ParseFactor();
                    if (divisor == 0)
                    {
                        throw new DivideByZeroException();
                    }

                    value %= divisor;
                }
                else
                {
                    return value;
                }
            }
        }

        private double ParseFactor()
        {
            SkipWhitespace();

            if (Consume('+'))
            {
                return ParseFactor();
            }

            if (Consume('-'))
            {
                return -ParseFactor();
            }

            if (ConsumeKeyword("sqrt"))
            {
                double inner = ParseFactor();
                if (inner < 0)
                {
                    throw new InvalidOperationException("Invalid expression");
                }

                return Math.Sqrt(inner);
            }

            if (Consume('('))
            {
                double value = ParseExpression();
                SkipWhitespace();
                if (!Consume(')'))
                {
                    throw new InvalidOperationException("Invalid expression");
                }

                return value;
            }

            return ParseNumber();
        }

        private double ParseNumber()
        {
            SkipWhitespace();
            int start = _index;
            bool sawDigit = false;
            bool sawDot = false;

            while (_index < _chars.Length)
            {
                char ch = _chars[_index];
                if (char.IsDigit(ch))
                {
                    sawDigit = true;
                    _index++;
                }
                else if (ch == '.' && !sawDot)
                {
                    sawDot = true;
                    _index++;
                }
                else
                {
                    break;
                }
            }

            if (!sawDigit)
            {
                throw new InvalidOperationException("Invalid expression");
            }

            string token = new(_chars[start.._index]);
            if (!double.TryParse(token, NumberStyles.Float, CultureInfo.InvariantCulture, out double value))
            {
                throw new InvalidOperationException("Invalid expression");
            }

            return value;
        }

        private void SkipWhitespace()
        {
            while (_index < _chars.Length && char.IsWhiteSpace(_chars[_index]))
            {
                _index++;
            }
        }

        private bool Consume(char ch)
        {
            if (_index >= _chars.Length || _chars[_index] != ch)
            {
                return false;
            }

            _index++;
            return true;
        }

        private bool ConsumeKeyword(string keyword)
        {
            if (_index + keyword.Length > _chars.Length)
            {
                return false;
            }

            for (int i = 0; i < keyword.Length; i++)
            {
                if (char.ToLowerInvariant(_chars[_index + i]) != keyword[i])
                {
                    return false;
                }
            }

            _index += keyword.Length;
            return true;
        }
    }
}
