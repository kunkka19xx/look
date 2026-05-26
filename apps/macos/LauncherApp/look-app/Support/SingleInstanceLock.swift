import Darwin
import Foundation

nonisolated enum SingleInstanceLockResult {
    case acquired(CInt)
    case heldByOtherInstance
    case unavailable
}

nonisolated enum SingleInstanceLock {
    static let defaultPollIntervalMicros: useconds_t = 50_000
    // FNV-1a 64-bit constants used for stable lock-file naming.
    private static let fnv1a64OffsetBasis: UInt64 = 1_469_598_103_934_665_603
    private static let fnv1a64Prime: UInt64 = 1_099_511_628_211

    static func lockPath(for bundlePath: String, tempDirectory: String = NSTemporaryDirectory()) -> String {
        let hash = stablePathHash(bundlePath)
        let fileName = "look-single-instance-\(hash).lock"
        return (tempDirectory as NSString).appendingPathComponent(fileName)
    }

    static func stablePathHash(_ value: String) -> UInt64 {
        var hash = fnv1a64OffsetBasis
        for byte in value.utf8 {
            hash ^= UInt64(byte)
            hash &*= fnv1a64Prime
        }
        return hash
    }

    static func acquire(
        lockPath: String,
        timeoutSeconds: TimeInterval,
        pollIntervalMicros: useconds_t = defaultPollIntervalMicros
    ) -> SingleInstanceLockResult {
        let fd = open(lockPath, O_CREAT | O_RDWR, S_IRUSR | S_IWUSR)
        guard fd >= 0 else {
            return .unavailable
        }

        let deadline = Date().addingTimeInterval(timeoutSeconds)
        while true {
            if flock(fd, LOCK_EX | LOCK_NB) == 0 {
                return .acquired(fd)
            }

            if errno != EWOULDBLOCK && errno != EAGAIN {
                close(fd)
                return .unavailable
            }

            if Date() >= deadline {
                close(fd)
                return .heldByOtherInstance
            }

            usleep(pollIntervalMicros)
        }
    }

    static func release(_ fd: CInt) {
        if fd >= 0 {
            close(fd)
        }
    }
}
