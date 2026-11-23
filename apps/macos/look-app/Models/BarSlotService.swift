import Combine
import Foundation

@MainActor
final class BarSlotService: ObservableObject {
    /// Missing pins included: Settings lists them too.
    @Published private(set) var pins: [BarPin] = []
    @Published private(set) var todoPick = 0
    @Published private(set) var lastScript: (command: String, outcome: BarScriptOutcome)?

    private var pinsGeneration = 0
    private var scriptRunning = false
    private var wantedScript = ""

    var shownPins: [BarPin] {
        pins.filter { $0.title != nil && $0.path != nil }
    }

    func refresh(slot: BarSlot, pinnedIDs: [String], script: String) {
        refreshPins(pinnedIDs)
        switch slot {
        case .todo:
            todoPick = Int.random(in: 0..<Int.max)
        case .script:
            runScript(script)
        case .running, .pinned, .none:
            break
        }
    }

    func refreshPins(_ ids: [String]) {
        pinsGeneration &+= 1
        guard !ids.isEmpty else {
            pins = []
            return
        }
        let generation = pinsGeneration
        Task.detached(priority: .userInitiated) {
            let found = EngineBridge.shared.barPins(ids: ids)
            await MainActor.run { [weak self] in
                guard let self, generation == self.pinsGeneration else { return }
                self.pins = found
            }
        }
    }

    func scriptOutcome(for command: String) -> BarScriptOutcome? {
        guard let lastScript, lastScript.command == command else { return nil }
        return lastScript.outcome
    }

    func scriptLine(for command: String) -> String? {
        guard let line = scriptOutcome(for: command)?.line, !line.isEmpty else { return nil }
        return line
    }

    /// One run at a time; a command changed meanwhile runs once it ends.
    private func runScript(_ command: String) {
        wantedScript = command
        guard !command.isEmpty, !scriptRunning else { return }
        scriptRunning = true
        Task.detached(priority: .utility) {
            let outcome = EngineBridge.shared.barScriptLine(command)
            await MainActor.run { [weak self] in
                guard let self else { return }
                self.scriptRunning = false
                self.lastScript = (command, outcome)
                if self.wantedScript != command {
                    self.runScript(self.wantedScript)
                }
            }
        }
    }
}
