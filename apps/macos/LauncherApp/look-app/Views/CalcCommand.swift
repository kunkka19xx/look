import Foundation

enum CalcResult {
    case value(String)
    case error(String)
}

struct CalcCommand {
    static let maxMagnitude: Double = 1_000_000_000_000.0

    static func evaluate(_ expression: String) -> CalcResult {
        guard isReadyForEvaluation(expression) else {
            return .error("Invalid expression")
        }

        let normalized = decimalizeIntegerTokens(in: normalizeExpression(expression))

        do {
            var parser = Parser(input: normalized)
            let evaluated = try parser.parse()
            if abs(evaluated) > maxMagnitude {
                return .error("Error: result out of range (±1,000,000,000,000)")
            }
            return .value(formatFloat(evaluated))
        } catch ParserError.divisionByZero {
            return .error("Error: division by zero")
        } catch {
            return .error("Invalid expression")
        }
    }

    static func isReadyForEvaluation(_ expression: String) -> Bool {
        let trimmed = expression.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty { return false }

        var balance = 0
        for ch in trimmed {
            if ch == "(" { balance += 1 }
            if ch == ")" {
                balance -= 1
                if balance < 0 { return false }
            }
        }
        if balance != 0 { return false }

        if let last = trimmed.last, "+-*/%^.(".contains(last) {
            return false
        }

        let allowedPattern = "^[0-9A-Za-z_+\\-*/%^().:xXvV\\s]+$"
        guard let regex = try? NSRegularExpression(pattern: allowedPattern) else { return false }
        let full = NSRange(trimmed.startIndex..<trimmed.endIndex, in: trimmed)
        guard let match = regex.firstMatch(in: trimmed, range: full), match.range == full else {
            return false
        }
        return true
    }

    private static func formatFloat(_ value: Double) -> String {
        if value.isNaN || value.isInfinite { return "nan" }

        let formatter = NumberFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.numberStyle = .decimal
        formatter.groupingSeparator = ","
        formatter.usesGroupingSeparator = true
        formatter.minimumFractionDigits = 4
        formatter.maximumFractionDigits = 4
        formatter.minimumIntegerDigits = 1
        return formatter.string(from: NSNumber(value: value)) ?? String(format: "%.4f", value)
    }

    private static func decimalizeIntegerTokens(in expression: String) -> String {
        let pattern = "(?<![A-Za-z0-9_\\.])([0-9]+)(?![A-Za-z0-9_\\.])"
        guard let regex = try? NSRegularExpression(pattern: pattern) else { return expression }

        let range = NSRange(expression.startIndex..<expression.endIndex, in: expression)
        let matches = regex.matches(in: expression, range: range)
        var output = expression
        for match in matches.reversed() {
            guard let tokenRange = Range(match.range(at: 1), in: output) else { continue }
            output.replaceSubrange(tokenRange, with: output[tokenRange] + ".0")
        }
        return output
    }

    private static func normalizeExpression(_ expression: String) -> String {
        let normalized = expression
            .replacingOccurrences(of: "x", with: "*")
            .replacingOccurrences(of: "X", with: "*")
            .replacingOccurrences(of: ":", with: "/")
        return replacePrefixSqrt(in: normalized)
    }

    private static func replacePrefixSqrt(in expression: String) -> String {
        var output = ""
        var index = expression.startIndex

        while index < expression.endIndex {
            let char = expression[index]
            if char == "v" || char == "V" {
                let prev = index > expression.startIndex ? expression[expression.index(before: index)] : " "
                let nextIndex = expression.index(after: index)
                let next = nextIndex < expression.endIndex ? expression[nextIndex] : " "
                let prevIsWord = prev.isLetter || prev.isNumber || prev == "_"
                let nextIsStart = next.isNumber || next == "." || next == "(" || next == " "
                if !prevIsWord && nextIsStart {
                    output.append("sqrt")
                    index = nextIndex
                    continue
                }
            }
            output.append(char)
            index = expression.index(after: index)
        }
        return output
    }
}

private enum ParserError: Error {
    case invalidExpression
    case divisionByZero
}

private struct Parser {
    private let chars: [Character]
    private var index: Int = 0

    init(input: String) {
        self.chars = Array(input)
    }

    mutating func parse() throws -> Double {
        let value = try parseExpression()
        skipWhitespace()
        guard index == chars.count else {
            throw ParserError.invalidExpression
        }
        return value
    }

    private mutating func parseExpression() throws -> Double {
        var value = try parseTerm()
        while true {
            skipWhitespace()
            if consume("+") {
                value += try parseTerm()
            } else if consume("-") {
                value -= try parseTerm()
            } else {
                return value
            }
        }
    }

    private mutating func parseTerm() throws -> Double {
        var value = try parsePower()
        while true {
            skipWhitespace()
            if consume("*") {
                value *= try parsePower()
            } else if consume("/") {
                let divisor = try parsePower()
                if divisor == 0 {
                    throw ParserError.divisionByZero
                }
                value /= divisor
            } else if consume("%") {
                let divisor = try parsePower()
                if divisor == 0 {
                    throw ParserError.divisionByZero
                }
                value = value.truncatingRemainder(dividingBy: divisor)
            } else {
                return value
            }
        }
    }

    private mutating func parsePower() throws -> Double {
        var value = try parseUnary()
        skipWhitespace()
        if consume("^") {
            let exponent = try parsePower()
            value = Foundation.pow(value, exponent)
            if value.isNaN || value.isInfinite {
                throw ParserError.invalidExpression
            }
        }
        return value
    }

    private mutating func parseUnary() throws -> Double {
        skipWhitespace()

        if consume("+") {
            return try parseUnary()
        }
        if consume("-") {
            return -(try parseUnary())
        }

        if matchKeyword("sqrt") {
            _ = consumeKeyword("sqrt")
            let inner = try parseUnary()
            if inner < 0 {
                throw ParserError.invalidExpression
            }
            return Foundation.sqrt(inner)
        }

        return try parsePrimary()
    }

    private mutating func parsePrimary() throws -> Double {
        skipWhitespace()

        if consume("(") {
            let value = try parseExpression()
            skipWhitespace()
            guard consume(")") else {
                throw ParserError.invalidExpression
            }
            return value
        }

        return try parseNumber()
    }

    private mutating func parseNumber() throws -> Double {
        skipWhitespace()
        let start = index
        var sawDigit = false
        var sawDot = false

        while index < chars.count {
            let ch = chars[index]
            if ch.isNumber {
                sawDigit = true
                index += 1
            } else if ch == "." && !sawDot {
                sawDot = true
                index += 1
            } else {
                break
            }
        }

        guard sawDigit else {
            throw ParserError.invalidExpression
        }

        let token = String(chars[start..<index])
        guard let value = Double(token) else {
            throw ParserError.invalidExpression
        }
        return value
    }

    private mutating func skipWhitespace() {
        while index < chars.count && chars[index].isWhitespace {
            index += 1
        }
    }

    private mutating func consume(_ ch: Character) -> Bool {
        guard index < chars.count, chars[index] == ch else { return false }
        index += 1
        return true
    }

    private func matchKeyword(_ keyword: String) -> Bool {
        let end = index + keyword.count
        guard end <= chars.count else { return false }
        let token = String(chars[index..<end])
        return token.lowercased() == keyword
    }

    private mutating func consumeKeyword(_ keyword: String) -> Bool {
        guard matchKeyword(keyword) else { return false }
        index += keyword.count
        return true
    }
}
