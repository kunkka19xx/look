import Foundation

/// Decodes an Ollama `/api/chat` response (structured via `format`) into an
/// `ActionPlan`. The model returns the JSON in `message.content` as a string;
/// this pulls it out and decodes it. Returns nil on any shape mismatch, so the
/// planner treats a garbled response as "no plan".
nonisolated enum ActionPlanParser {
    /// The assistant's raw `message.content`, used both to decode the plan and to
    /// echo back during a repair round.
    static func messageContent(_ data: Data) -> String? {
        guard
            let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
            let message = root["message"] as? [String: Any],
            let content = message["content"] as? String
        else {
            return nil
        }
        return content
    }

    static func parse(chatResponse data: Data) -> ActionPlan? {
        guard
            let content = messageContent(data),
            let inner = content.data(using: .utf8)
        else {
            return nil
        }
        return try? JSONDecoder().decode(ActionPlan.self, from: inner)
    }
}
