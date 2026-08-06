import Foundation

/// Builds the JSON Schema handed to Ollama's `format` field so the model is
/// forced to emit `{ "steps": [{ "tool": <id>, "params": { "title": ... } }] }`.
/// `tool` is constrained to an enum, so the model cannot invent one. `params` is
/// title-only: everything else (dates, durations) is computed in code, which
/// keeps the model's output - and therefore latency - minimal.
nonisolated enum ActionPlanSchema {
    static func chatFormat(toolIDs: [String]) -> AIValue {
        let step: AIValue = .object([
            "type": .string("object"),
            "properties": .object([
                "tool": .object([
                    "type": .string("string"),
                    "enum": .array(toolIDs.map { .string($0) }),
                ]),
                "params": .schema(
                    properties: ["title": .schemaType("string")],
                    required: ["title"]),
            ]),
            "required": .array([.string("tool"), .string("params")]),
        ])
        return .object([
            "type": .string("object"),
            "properties": .object([
                "steps": .object([
                    "type": .string("array"),
                    "items": step,
                ]),
            ]),
            "required": .array([.string("steps")]),
        ])
    }
}
