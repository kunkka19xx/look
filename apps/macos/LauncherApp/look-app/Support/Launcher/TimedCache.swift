import Foundation

/// One value, held for a short while.
///
/// Exists because the launcher's pinned rows are COMPUTED properties: SwiftUI
/// re-reads them on every view update, so anything expensive behind one (an
/// EventKit fetch, a Contacts lookup) runs many times per keystroke without a
/// cache. Two services hand-rolled the same lock, timestamp, and key check
/// before this.
///
/// Deliberately not a general-purpose cache: one slot, one key, no eviction
/// policy. The keyed slot is what makes it correct for a query that changes as
/// the user types, since a new key is a miss rather than a stale hit.
nonisolated final class TimedCache<Key: Equatable, Value>: @unchecked Sendable {
    private let ttl: TimeInterval
    private let lock = NSLock()
    private var key: Key?
    private var value: Value?
    private var storedAt = Date.distantPast

    init(ttl: TimeInterval) {
        self.ttl = ttl
    }

    /// The cached value for `key`, or `make()` if it is missing or stale.
    ///
    /// `make` runs while the lock is held: these callers are all on the main
    /// actor, and letting two of them fetch at once would defeat the point.
    func value(for key: Key, now: Date = Date(), make: () -> Value) -> Value {
        lock.lock()
        defer { lock.unlock() }
        if let value, self.key == key, now.timeIntervalSince(storedAt) < ttl {
            return value
        }
        let fresh = make()
        self.key = key
        self.value = fresh
        storedAt = now
        return fresh
    }

    /// Drop what is held, for when the source changed underneath.
    func invalidate() {
        lock.lock()
        defer { lock.unlock() }
        key = nil
        value = nil
        storedAt = .distantPast
    }
}
