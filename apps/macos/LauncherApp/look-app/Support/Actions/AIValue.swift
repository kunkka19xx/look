import Foundation

/// A JSON-shaped value. Tool params and JSON Schemas both speak this, so what a
/// model emits maps 1:1 to what a tool consumes. Custom Codable because Swift has
/// no built-in JSON value type. Everything lives in the body so the type's
/// `nonisolated` covers it (extensions would not inherit it).
nonisolated enum AIValue: Equatable, Codable {
    case string(String)
    case number(Double)
    case bool(Bool)
    case array([AIValue])
    case object([String: AIValue])
    case null

    var stringValue: String? { if case .string(let s) = self { return s } else { return nil } }
    var numberValue: Double? { if case .number(let n) = self { return n } else { return nil } }
    var boolValue: Bool? { if case .bool(let b) = self { return b } else { return nil } }
    var arrayValue: [AIValue]? { if case .array(let a) = self { return a } else { return nil } }
    var objectValue: [String: AIValue]? { if case .object(let o) = self { return o } else { return nil } }

    /// Builds a JSON-Schema object node, the shape a tool returns from `paramsSchema`.
    static func schema(properties: [String: AIValue], required: [String]) -> AIValue {
        .object([
            "type": .string("object"),
            "properties": .object(properties),
            "required": .array(required.map { .string($0) }),
        ])
    }

    /// A leaf `{ "type": <t> }` schema node.
    static func schemaType(_ type: String) -> AIValue {
        .object(["type": .string(type)])
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        // Order matters: bool before number so `true` isn't read as a number.
        if container.decodeNil() {
            self = .null
        } else if let b = try? container.decode(Bool.self) {
            self = .bool(b)
        } else if let n = try? container.decode(Double.self) {
            self = .number(n)
        } else if let s = try? container.decode(String.self) {
            self = .string(s)
        } else if let a = try? container.decode([AIValue].self) {
            self = .array(a)
        } else if let o = try? container.decode([String: AIValue].self) {
            self = .object(o)
        } else {
            throw DecodingError.dataCorruptedError(
                in: container, debugDescription: "Unsupported AIValue")
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .null: try container.encodeNil()
        case .bool(let b): try container.encode(b)
        case .number(let n): try container.encode(n)
        case .string(let s): try container.encode(s)
        case .array(let a): try container.encode(a)
        case .object(let o): try container.encode(o)
        }
    }
}
