import Foundation

/// Thin shell over the Rust-core planner (core/ai/src/planner.rs), which owns
/// the prompt, the tool aliases, and the model-output mapping for every shell.
/// This side keeps only the availability gate and the date seam: for add tools,
/// `when` is injected from the raw query when NSDataDetector can resolve it.
@MainActor
final class ActionPlanner {
    init() {}

    /// Whether a capable model is configured to plan actions. Apple Intelligence
    /// is not a planner; only a reachable Ollama with the model pulled qualifies.
    var isAvailable: Bool {
        let settings = ThemeStore.shared.settings
        guard settings.aiEnabled, settings.aiProvider == .ollama else { return false }
        return AIQueryRouter.shared.availability(of: .ollama).isAvailable
    }

    /// Primes the model + Ollama's prompt-prefix cache while the user types.
    func warmUp() async {
        guard isAvailable else { return }
        let settings = ThemeStore.shared.settings
        let host = settings.ollamaHost
        let model = settings.ollamaModel
        let bridge = EngineBridge.shared
        await Task.detached(priority: .utility) {
            bridge.aiWarmPlanner(host: host, model: model)
        }.value
    }

    /// Returns a `ToolCall`, or nil when there is no capable provider, the
    /// request is not an action, or the response is unusable.
    func plan(query: String) async -> ToolCall? {
        guard isAvailable else { return nil }
        let settings = ThemeStore.shared.settings
        let host = settings.ollamaHost
        let model = settings.ollamaModel
        let bridge = EngineBridge.shared

        let raw = await Task.detached(priority: .userInitiated) {
            bridge.aiPlan(host: host, model: model, query: query)
        }.value
        guard let raw else { return nil }

        var params = raw.params
        // Date seam: add tools get `when` = the raw query when a date resolves
        // in it (NSDataDetector, macOS-quality). No date -> all-day / undated.
        if raw.toolID == "calendar.add_event" || raw.toolID == "reminder.add",
           DatePhrase.resolve(query, now: Date()) != nil {
            params["when"] = query
        }
        return ToolCall(toolID: raw.toolID, params: params)
    }
}
