import Foundation

/// One value, held for a short while, behind a lock.
///
/// The launcher's pinned rows are computed properties SwiftUI re-reads on every
/// update, so an EventKit or Contacts lookup behind one needs a cache. One slot,
/// one key, no eviction: a new key is a miss, not a stale hit.
nonisolated final class TimedCache<Key: Equatable, Value>: @unchecked Sendable {
    private let ttl: TimeInterval
    private let lock = NSLock()
    private var key: Key?
    private var value: Value?
    private var storedAt = Date.distantPast

    init(ttl: TimeInterval) {
        self.ttl = ttl
    }

    /// The cached value for `key`, or `make()` when missing or stale. `make`
    /// runs under the lock, so two callers cannot fetch at once.
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

    /// Drop what is held.
    func invalidate() {
        lock.lock()
        defer { lock.unlock() }
        key = nil
        value = nil
        storedAt = .distantPast
    }
}
