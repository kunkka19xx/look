import Foundation

/// Whether the user's message has already been put in the transcript for the
/// submit being handled.
///
/// It exists because "who appends the turn" was spread across three call paths
/// coordinated by a `userItemAppended: Bool` threaded through arguments, and
/// getting it wrong was silent: pass it and the message vanishes, forget it and
/// the message appears twice. Both happened. The planner deliberately shows the
/// message BEFORE it thinks, so by the time a plan comes back the turn may or
/// may not exist - a caller cannot know, and now does not have to.
///
/// One rule: `startTurn()` per submit, then any path may `shouldAppend(id:)`.
/// The first asker wins and everyone after it is told no.
nonisolated struct TurnLedger: Equatable {
    private(set) var currentID: UUID?

    /// A new user message is being handled; the next claim materializes it.
    mutating func startTurn() {
        currentID = nil
    }

    /// True when the caller should append the turn, false when it already
    /// exists. Records the id on the first claim.
    mutating func shouldAppend(id: UUID) -> Bool {
        guard currentID == nil else { return false }
        currentID = id
        return true
    }

    /// No turn is in flight (session ended or restored).
    mutating func reset() {
        currentID = nil
    }
}
