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

        if let last = trimmed.last, "+-*/^.(".contains(last) {
            return false
        }

        let allowedPattern = "^[0-9A-Za-z_+\\-*/%^!().:xXvV\\s]+$"
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
        var value = try parseUnary()
        while true {
            skipWhitespace()
            if consume("*") {
                value *= try parseUnary()
            } else if consume("/") {
                let divisor = try parseUnary()
                if divisor == 0 {
                    throw ParserError.divisionByZero
                }
                value /= divisor
            } else if consume("%") {
                let divisor = try parseUnary()
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
        var value = try parsePrimary()
        skipWhitespace()
        if consume("^") {
            let exponent = try parseUnary()
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
            return try applyFunction("sqrt", to: try parseFunctionArgument())
        }

        if matchKeyword("abs") {
            _ = consumeKeyword("abs")
            return try applyFunction("abs", to: try parseFunctionArgument())
        }

        if matchKeyword("round") {
            _ = consumeKeyword("round")
            return try applyFunction("round", to: try parseFunctionArgument())
        }

        if matchKeyword("floor") {
            _ = consumeKeyword("floor")
            return try applyFunction("floor", to: try parseFunctionArgument())
        }

        if matchKeyword("ceil") {
            _ = consumeKeyword("ceil")
            return try applyFunction("ceil", to: try parseFunctionArgument())
        }

        return try parsePower()
    }

    private mutating func parsePrimary() throws -> Double {
        skipWhitespace()

        if consume("(") {
            let value = try parseExpression()
            skipWhitespace()
            guard consume(")") else {
                throw ParserError.invalidExpression
            }
            return try applyPostfixOperators(to: value)
        }

        if index < chars.count, chars[index].isLetter {
            let ident = parseIdentifier()
            switch ident.lowercased() {
            case "pi":
                return try applyPostfixOperators(to: .pi)
            case "e":
                return try applyPostfixOperators(to: M_E)
            default:
                throw ParserError.invalidExpression
            }
        }

        let number = try parseNumber()
        return try applyPostfixOperators(to: number)
    }

    private mutating func applyPostfixOperators(to seed: Double) throws -> Double {
        var value = seed
        while true {
            skipWhitespace()
            if consume("!") {
                value = try factorial(value)
                continue
            }
            if shouldConsumePostfixPercent() {
                _ = consume("%")
                value /= 100
                continue
            }
            return value
        }
    }

    private mutating func shouldConsumePostfixPercent() -> Bool {
        guard index < chars.count, chars[index] == "%" else { return false }
        var lookahead = index + 1
        while lookahead < chars.count && chars[lookahead].isWhitespace {
            lookahead += 1
        }
        guard lookahead < chars.count else { return true }
        let next = chars[lookahead]
        if next.isNumber || next == "." || next == "(" || next.isLetter {
            return false
        }
        return true
    }

    private mutating func parseFunctionArgument() throws -> Double {
        skipWhitespace()
        if consume("(") {
            let value = try parseExpression()
            skipWhitespace()
            guard consume(")") else { throw ParserError.invalidExpression }
            return value
        }
        return try parseUnary()
    }

    private func applyFunction(_ name: String, to value: Double) throws -> Double {
        switch name {
        case "sqrt":
            guard value >= 0 else { throw ParserError.invalidExpression }
            return Foundation.sqrt(value)
        case "abs":
            return Swift.abs(value)
        case "round":
            return Foundation.round(value)
        case "floor":
            return Foundation.floor(value)
        case "ceil":
            return Foundation.ceil(value)
        default:
            throw ParserError.invalidExpression
        }
    }

    private func factorial(_ value: Double) throws -> Double {
        guard value >= 0, value.rounded() == value else {
            throw ParserError.invalidExpression
        }
        let n = Int(value)
        guard n <= 170 else {
            throw ParserError.invalidExpression
        }
        if n <= 1 { return 1 }
        return (2...n).reduce(1.0) { $0 * Double($1) }
    }

    private mutating func parseIdentifier() -> String {
        let start = index
        while index < chars.count, chars[index].isLetter || chars[index] == "_" {
            index += 1
        }
        return String(chars[start..<index])
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
