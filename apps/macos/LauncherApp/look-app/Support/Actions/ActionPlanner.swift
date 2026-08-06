import Foundation

/// Turns a natural-language request into a `ToolCall` using the configured Ollama
/// model. Latency is the design driver: the model emits ONLY intent + a clean
/// title (~15 tokens); the `when` date is extracted from the query in code by
/// NSDataDetector, whose date *value* is robust (only its text range was ever
/// unreliable, and we no longer need it). Static system prompt so Ollama's
/// prompt-prefix cache eliminates reprocessing; temperature 0; single-shot (the
/// fields a repair round could fix no longer come from the model).
@MainActor
final class ActionPlanner {
    private let registry: ActionRegistry

    /// Short aliases keep the emitted `tool` to ~1 token; mapped to real ids here.
    private static let aliasToToolID = [
        "event": "calendar.add_event",
        "reminder": "reminder.add",
        "cancel": "calendar.cancel_event",
        "move": "calendar.move_event",
        "complete": "reminder.complete",
        "delete": "reminder.remove",
    ]

    init(registry: ActionRegistry) {
        self.registry = registry
    }

    /// Whether a capable model is configured to plan actions. Apple Intelligence
    /// is not a planner; only a reachable Ollama with the model pulled qualifies.
    var isAvailable: Bool {
        let settings = ThemeStore.shared.settings
        guard settings.aiEnabled, settings.aiProvider == .ollama else { return false }
        return AIQueryRouter.shared.availability(of: .ollama).isAvailable
    }

    /// Primes both the model (loaded) and Ollama's prompt-prefix cache with the
    /// exact planner system prompt, so the first real plan skips model load and
    /// prompt processing. Fired when the user starts a `>` query, while they
    /// finish typing. Result is discarded.
    func warmUp() async {
        guard isAvailable else { return }
        let settings = ThemeStore.shared.settings
        _ = await OllamaProvider.chatJSON(
            host: settings.ollamaHost, model: settings.ollamaModel,
            messages: [
                ["role": "system", "content": Self.systemPrompt],
                ["role": "user", "content": "hi"],
            ],
            format: Self.format())
    }

    /// Returns a validated `ToolCall`, or nil when there is no capable provider,
    /// the request is not an action (empty steps), or the response is unusable.
    func plan(query: String) async -> ToolCall? {
        guard isAvailable else { return nil }
        let settings = ThemeStore.shared.settings

        guard
            let data = await OllamaProvider.chatJSON(
                host: settings.ollamaHost, model: settings.ollamaModel,
                messages: [
                    ["role": "system", "content": Self.systemPrompt],
                    ["role": "user", "content": query],
                ],
                format: Self.format()),
            let plan = ActionPlanParser.parse(chatResponse: data),
            let step = plan.steps.first
        else {
            return nil
        }
        // No registry validation here: an .invalid ("no event matching ...") or
        // .needsChoice outcome is informative and handled by the controller.
        return resolveCall(step: step, query: query)
    }

    /// Maps the model's minimal step to a full `ToolCall`. Add tools: clean
    /// title from the model, `when` injected from the raw query in code. Mutate
    /// tools: `match` (and, for move, the model-supplied NEW time phrase).
    private func resolveCall(step: PlanStep, query: String) -> ToolCall? {
        guard let toolID = Self.aliasToToolID[step.tool] else { return nil }
        var params: [String: AIValue] = [:]
        switch toolID {
        case "calendar.add_event", "reminder.add":
            guard let title = step.params["title"]?.stringValue, !title.isEmpty else { return nil }
            params["title"] = .string(title)
            if DatePhrase.resolve(query, now: Date()) != nil {
                params["when"] = .string(query)
            }
        case "calendar.move_event":
            guard
                let match = step.params["match"]?.stringValue, !match.isEmpty,
                let when = step.params["when"]?.stringValue, !when.isEmpty
            else { return nil }
            params["match"] = .string(match)
            params["when"] = .string(when)
        default:  // cancel, complete: just a match (title tolerated as fallback)
            guard
                let match = step.params["match"]?.stringValue
                    ?? step.params["title"]?.stringValue,
                !match.isEmpty
            else { return nil }
            params["match"] = .string(match)
        }
        return ToolCall(toolID: toolID, params: params)
    }

    private static func format() -> [String: Any]? {
        ActionPlanSchema.chatFormat(toolIDs: Array(aliasToToolID.keys).sorted())
            .jsonObject as? [String: Any]
    }

    /// Static (never includes the date/time), so Ollama caches the prompt prefix
    /// across calls and prompt processing stays near-zero.
    private static let systemPrompt = """
        Classify the request into ONE tool and extract its params:
        - "event": add a calendar event. params: title (clean short title; drop \
        the leading verb, filler words, and all date/time words; capitalize the \
        first word).
        - "reminder": add a reminder. params: title (same rules).
        - "cancel": remove an EXISTING event. params: match (the words that \
        identify which event, e.g. "dentist").
        - "move": reschedule an EXISTING event. params: match, when (the NEW \
        time phrase copied verbatim, e.g. "4pm", "friday 9am").
        - "complete": mark an EXISTING reminder done. params: match.
        - "delete": remove an EXISTING reminder from the list. params: match.
        Pronouns and references are valid match values: "remove it" -> match \
        "it"; "cancel this event" -> match "this event".
        Reply with JSON only: {"steps":[{"tool":"...","params":{...}}]}.
        If it is none of these, reply {"steps":[]}.
        """
}
