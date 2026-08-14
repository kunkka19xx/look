import Foundation

/// Whether this submit's message is already in the transcript.
///
/// The planner shows the message BEFORE it thinks, so a later path cannot know
/// whether the turn exists - and the `userItemAppended: Bool` this replaces got
/// it wrong both ways. One `startTurn()` per submit; first claimer wins.
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
