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
        if ("+-*/%^.(".IndexOf(last) >= 0)
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
            || ch == '^'
            || ch == '!'
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
        private readonly struct ValueNode
        {
            public double Value { get; }
            public bool IsStandalonePercent { get; }
            public double PercentFraction { get; }

            public ValueNode(double value, bool isStandalonePercent = false, double percentFraction = 0)
            {
                Value = value;
                IsStandalonePercent = isStandalonePercent;
                PercentFraction = percentFraction;
            }
        }

        private readonly char[] _chars;
        private int _index;

        public Parser(string input)
        {
            _chars = input.ToCharArray();
        }

        public double Parse()
        {
            ValueNode value = ParseExpression();
            SkipWhitespace();
            if (_index != _chars.Length)
            {
                throw new InvalidOperationException("Invalid expression");
            }

            return value.Value;
        }

        private ValueNode ParseExpression()
        {
            ValueNode value = ParseTerm();
            while (true)
            {
                SkipWhitespace();
                if (Consume('+'))
                {
                    ValueNode rhs = ParseTerm();
                    double combined = rhs.IsStandalonePercent
                        ? value.Value + (value.Value * rhs.PercentFraction)
                        : value.Value + rhs.Value;
                    value = new ValueNode(combined);
                }
                else if (Consume('-'))
                {
                    ValueNode rhs = ParseTerm();
                    double combined = rhs.IsStandalonePercent
                        ? value.Value - (value.Value * rhs.PercentFraction)
                        : value.Value - rhs.Value;
                    value = new ValueNode(combined);
                }
                else
                {
                    return value;
                }
            }
        }

        private ValueNode ParseTerm()
        {
            ValueNode value = ParseUnary();
            while (true)
            {
                SkipWhitespace();
                if (Consume('*'))
                {
                    ValueNode rhs = ParseUnary();
                    value = new ValueNode(value.Value * rhs.Value);
                }
                else if (Consume('/'))
                {
                    ValueNode rhs = ParseUnary();
                    double divisor = rhs.Value;
                    if (divisor == 0)
                    {
                        throw new DivideByZeroException();
                    }

                    value = new ValueNode(value.Value / divisor);
                }
                else if (Consume('%'))
                {
                    ValueNode rhs = ParseUnary();
                    double divisor = rhs.Value;
                    if (divisor == 0)
                    {
                        throw new DivideByZeroException();
                    }

                    value = new ValueNode(value.Value % divisor);
                }
                else
                {
                    return value;
                }
            }
        }

        private ValueNode ParsePower()
        {
            ValueNode value = ParsePrimary();
            SkipWhitespace();
            if (Consume('^'))
            {
                ValueNode exponent = ParseUnary();
                double powered = Math.Pow(value.Value, exponent.Value);
                if (double.IsNaN(powered) || double.IsInfinity(powered))
                {
                    throw new InvalidOperationException("Invalid expression");
                }

                value = new ValueNode(powered);
            }

            return value;
        }

        private ValueNode ParseUnary()
        {
            SkipWhitespace();

            if (Consume('+'))
            {
                return ParseUnary();
            }

            if (Consume('-'))
            {
                ValueNode negated = ParseUnary();
                return new ValueNode(-negated.Value);
            }

            if (MatchKeyword("sqrt"))
            {
                ConsumeKeyword("sqrt");
                return new ValueNode(ApplyFunction("sqrt", ParseFunctionArgument()));
            }

            if (MatchKeyword("abs"))
            {
                ConsumeKeyword("abs");
                return new ValueNode(ApplyFunction("abs", ParseFunctionArgument()));
            }

            if (MatchKeyword("round"))
            {
                ConsumeKeyword("round");
                return new ValueNode(ApplyFunction("round", ParseFunctionArgument()));
            }

            if (MatchKeyword("floor"))
            {
                ConsumeKeyword("floor");
                return new ValueNode(ApplyFunction("floor", ParseFunctionArgument()));
            }

            if (MatchKeyword("ceil"))
            {
                ConsumeKeyword("ceil");
                return new ValueNode(ApplyFunction("ceil", ParseFunctionArgument()));
            }

            return ParsePower();
        }

        private ValueNode ParsePrimary()
        {
            SkipWhitespace();

            if (Consume('('))
            {
                ValueNode value = ParseExpression();
                SkipWhitespace();
                if (!Consume(')'))
                {
                    throw new InvalidOperationException("Invalid expression");
                }

                return ApplyPostfixOperators(value.Value);
            }

            if (_index < _chars.Length && char.IsLetter(_chars[_index]))
            {
                string ident = ParseIdentifier();
                double constant = ident.ToLowerInvariant() switch
                {
                    "pi" => Math.PI,
                    "e" => Math.E,
                    _ => throw new InvalidOperationException("Invalid expression"),
                };

                return ApplyPostfixOperators(constant);
            }

            double number = ParseNumber();
            return ApplyPostfixOperators(number);
        }

        private ValueNode ApplyPostfixOperators(double seed)
        {
            var node = new ValueNode(seed);
            while (true)
            {
                SkipWhitespace();
                if (Consume('!'))
                {
                    node = new ValueNode(Factorial(node.Value));
                    continue;
                }

                if (ShouldConsumePostfixPercent())
                {
                    Consume('%');
                    double fraction = node.Value / 100d;
                    node = new ValueNode(fraction, isStandalonePercent: true, percentFraction: fraction);
                    continue;
                }

                return node;
            }
        }

        private bool ShouldConsumePostfixPercent()
        {
            if (_index >= _chars.Length || _chars[_index] != '%')
            {
                return false;
            }

            int lookahead = _index + 1;
            while (lookahead < _chars.Length && char.IsWhiteSpace(_chars[lookahead]))
            {
                lookahead++;
            }

            if (lookahead >= _chars.Length)
            {
                return true;
            }

            char next = _chars[lookahead];
            if (char.IsDigit(next) || next == '.' || next == '(' || char.IsLetter(next))
            {
                return false;
            }

            return true;
        }

        private double ParseFunctionArgument()
        {
            SkipWhitespace();
            if (Consume('('))
            {
                ValueNode value = ParseExpression();
                SkipWhitespace();
                if (!Consume(')'))
                {
                    throw new InvalidOperationException("Invalid expression");
                }

                return value.Value;
            }

            return ParseUnary().Value;
        }

        private static double ApplyFunction(string name, double value)
        {
            return name switch
            {
                "sqrt" when value < 0 => throw new InvalidOperationException("Invalid expression"),
                "sqrt" => Math.Sqrt(value),
                "abs" => Math.Abs(value),
                "round" => Math.Round(value),
                "floor" => Math.Floor(value),
                "ceil" => Math.Ceiling(value),
                _ => throw new InvalidOperationException("Invalid expression"),
            };
        }

        private static double Factorial(double value)
        {
            if (value < 0 || Math.Round(value) != value)
            {
                throw new InvalidOperationException("Invalid expression");
            }

            int n = (int)value;
            if (n > 170)
            {
                throw new InvalidOperationException("Invalid expression");
            }

            if (n <= 1)
            {
                return 1;
            }

            double result = 1;
            for (int i = 2; i <= n; i++)
            {
                result *= i;
            }

            return result;
        }

        private string ParseIdentifier()
        {
            int start = _index;
            while (_index < _chars.Length && (char.IsLetter(_chars[_index]) || _chars[_index] == '_'))
            {
                _index++;
            }

            return new string(_chars[start.._index]);
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

        private bool MatchKeyword(string keyword)
        {
            int end = _index + keyword.Length;
            if (end > _chars.Length)
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

            return true;
        }

        private bool ConsumeKeyword(string keyword)
        {
            if (!MatchKeyword(keyword))
            {
                return false;
            }

            _index += keyword.Length;
            return true;
        }
    }
}
