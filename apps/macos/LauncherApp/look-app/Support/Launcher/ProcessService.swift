import Darwin
import Foundation

/// One row in the `ps"` finder: a raw process. Kept minimal so enumeration stays
/// cheap; richer fields load per-selection via `ProcessService.detail`. Mirrors
/// the linows `ProcRow` (`apps/linows/src-tauri/src/process.rs`), minus the
/// icon: macOS resolves that in the view via `NSRunningApplication`.
struct ProcessRow: Identifiable, Equatable {
    let pid: Int32
    let name: String
    /// Listening TCP ports owned by this process, sorted. Shown in the row and
    /// matched against a numeric query so `ps"` doubles as a find-by-port.
    let ports: [Int]

    var id: Int32 { pid }
}

/// Per-process detail for the `ps"` preview pane. All fields are cheap single
/// reads; `startEpoch` is Unix seconds (formatted in the view).
struct ProcessDetail: Equatable {
    let cmdline: String
    /// Memory footprint in KB. Uses `ri_phys_footprint` (the app's private
    /// memory) to match Activity Monitor's "Memory" column, rather than
    /// `ri_resident_size` (RSS), which counts shared pages and reads much higher.
    let memoryKB: UInt64
    let user: String
    let ppid: Int32
    let startEpoch: UInt64?
}

/// Native process enumeration / detail / CPU / kill for the macOS `ps"` finder,
/// scoped to the current user (own-uid reads need no SIP entitlement; killing
/// another user's process needs privilege and surfaces a clear error).
/// `nonisolated` so callers can run it on a detached task - it walks the process
/// table and sleeps, so it must stay off the main thread.
nonisolated enum ProcessService {
    /// Own-user processes: pid, name, listening ports. Uses `sysctl
    /// KERN_PROC_ALL` (what `ps` uses), **not** `proc_listallpids` - the latter
    /// is silently throttled in hardened contexts and returns a partial list
    /// (~146 of ~584 procs), which hid the user's own apps.
    static func enumerate() -> [ProcessRow] {
        let ownUID = getuid()
        var mib: [Int32] = [CTL_KERN, KERN_PROC, KERN_PROC_ALL, 0]

        // Size the table, then fill it with slack (it can grow between calls).
        // Retry once on ENOMEM if it grew past even the padded buffer.
        var procs: [kinfo_proc] = []
        for _ in 0..<3 {
            var size = 0
            guard sysctl(&mib, 4, nil, &size, nil, 0) == 0, size > 0 else { return [] }
            let stride = MemoryLayout<kinfo_proc>.stride
            let capacity = size / stride + 64 // slack for newly-spawned processes
            procs = [kinfo_proc](repeating: kinfo_proc(), count: capacity)
            var filled = capacity * stride
            let rc = sysctl(&mib, 4, &procs, &filled, nil, 0)
            if rc == 0 {
                procs.removeLast(procs.count - filled / stride)
                break
            }
            if errno != ENOMEM { return [] } // real error; retry only on "grew"
            procs = []
        }

        var rows: [ProcessRow] = []
        rows.reserveCapacity(procs.count)
        for var proc in procs {
            let pid = proc.kp_proc.p_pid
            guard pid > 0, proc.kp_eproc.e_ucred.cr_uid == ownUID else { continue }
            // p_comm is truncated to 16 chars; proc_name gives the full name.
            let name = processName(pid: pid, commFallback: cString(from: &proc.kp_proc.p_comm))
            rows.append(ProcessRow(pid: pid, name: name, ports: listeningPorts(pid: pid)))
        }
        return rows
    }

    /// Per-selection detail. Cheap reads; safe to call on arrow-key selection.
    static func detail(pid: Int32) -> ProcessDetail? {
        guard let info = bsdInfo(pid: pid) else { return nil }
        let user = userName(uid: info.pbi_uid)
        let start = info.pbi_start_tvsec == 0 ? nil : UInt64(info.pbi_start_tvsec)
        return ProcessDetail(
            cmdline: commandLine(pid: pid) ?? processName(pid: pid, commFallback: comm(from: info)),
            memoryKB: memoryFootprintKB(pid: pid),
            user: user,
            ppid: Int32(bitPattern: info.pbi_ppid),
            startEpoch: start
        )
    }

    /// CPU% over ~200 ms (two `proc_pid_rusage` reads). On-demand (bound to
    /// Enter), never per-selection - the sampling sleep would jank arrow-key
    /// nav. Percent of one core (may exceed 100, like `top`).
    static func cpu(pid: Int32) -> Double? {
        guard let first = cpuTimeNs(pid: pid) else { return nil }
        let intervalNs: UInt64 = 200_000_000
        let startNs = clock_gettime_nsec_np(CLOCK_MONOTONIC_RAW)
        var ts = timespec(tv_sec: 0, tv_nsec: Int(intervalNs))
        nanosleep(&ts, nil)
        guard let second = cpuTimeNs(pid: pid) else { return nil }

        // Measured elapsed, not the requested interval: nanosleep can overshoot
        // or return early on EINTR.
        let deltaCPU = Double(second >= first ? second - first : 0)
        let deltaWall = Double(clock_gettime_nsec_np(CLOCK_MONOTONIC_RAW) - startNs)
        guard deltaWall > 0 else { return nil }
        return 100.0 * (deltaCPU / deltaWall)
    }

    /// SIGKILL a process. Native `kill(2)` rather than shelling out; EPERM
    /// (another user's / protected process) surfaces a clear message instead of
    /// failing silently.
    static func kill(pid: Int32) -> Result<String, ProcessKillError> {
        if Darwin.kill(pid, SIGKILL) == 0 {
            return .success("Killed PID \(pid)")
        }
        let err = errno
        switch err {
        case EPERM:
            return .failure(.permissionDenied)
        case ESRCH:
            return .failure(.noSuchProcess)
        default:
            return .failure(.other(String(cString: strerror(err))))
        }
    }

    // MARK: - BSD info

    private static func bsdInfo(pid: Int32) -> proc_bsdinfo? {
        var info = proc_bsdinfo()
        let size = Int32(MemoryLayout<proc_bsdinfo>.size)
        let rc = proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, &info, size)
        return rc == size ? info : nil
    }

    /// Full process name via `proc_name`, falling back to a caller-supplied
    /// short name (the 16-char `comm`) when `proc_name` is unavailable. Never
    /// returns empty for a live process, so enumeration never drops a row just
    /// because `proc_name` failed.
    private static func processName(pid: Int32, commFallback: String) -> String {
        var buf = [CChar](repeating: 0, count: 2 * Int(MAXPATHLEN))
        let n = proc_name(pid, &buf, UInt32(buf.count))
        if n > 0 {
            let name = buf.withUnsafeBytes {
                String(decoding: $0.prefix(while: { $0 != 0 }), as: UTF8.self)
            }
            if !name.isEmpty { return name }
        }
        return commFallback.isEmpty ? "Process \(pid)" : commFallback
    }

    /// The 16-char `comm` from a `proc_bsdinfo` (prefers the longer `pbi_name`).
    private static func comm(from info: proc_bsdinfo) -> String {
        var info = info
        let name = cString(from: &info.pbi_name)
        return name.isEmpty ? cString(from: &info.pbi_comm) : name
    }

    /// Read a fixed-size C `char[]` tuple (imported as a Swift tuple) as a
    /// String. The scan stays inside the tuple: an unterminated field would
    /// make `String(cString:)` read past it.
    private static func cString<T>(from tuple: inout T) -> String {
        withUnsafeBytes(of: &tuple) { buffer in
            String(decoding: buffer.prefix(while: { $0 != 0 }), as: UTF8.self)
        }
    }

    private static func userName(uid: UInt32) -> String {
        guard let pw = getpwuid(uid) else { return "" }
        return String(cString: pw.pointee.pw_name)
    }

    // MARK: - Memory / CPU

    private static func rusage(pid: Int32) -> rusage_info_v2? {
        var usage = rusage_info_v2()
        let rc = withUnsafeMutablePointer(to: &usage) { ptr in
            ptr.withMemoryRebound(to: rusage_info_t?.self, capacity: 1) {
                proc_pid_rusage(pid, RUSAGE_INFO_V2, $0)
            }
        }
        return rc == 0 ? usage : nil
    }

    /// Memory footprint (`ri_phys_footprint`) in KB, matching Activity Monitor's
    /// "Memory" column. RSS (`ri_resident_size`) would over-count shared pages.
    private static func memoryFootprintKB(pid: Int32) -> UInt64 {
        (rusage(pid: pid)?.ri_phys_footprint ?? 0) / 1024
    }

    private static func cpuTimeNs(pid: Int32) -> UInt64? {
        rusage(pid: pid).map { $0.ri_user_time &+ $0.ri_system_time }
    }

    // MARK: - Command line (argv)

    /// Full argv via `sysctl KERN_PROCARGS2`. Layout: `Int32 argc`, then the
    /// exec path (NUL-terminated), NUL padding, then `argc` NUL-separated argv
    /// strings. We skip the exec path and join the argv strings with spaces.
    private static func commandLine(pid: Int32) -> String? {
        var argmax: Int = 0
        var sizeMax = MemoryLayout<Int>.size
        var mibMax = [CTL_KERN, KERN_ARGMAX]
        guard sysctl(&mibMax, 2, &argmax, &sizeMax, nil, 0) == 0, argmax > 0 else { return nil }

        var buffer = [CChar](repeating: 0, count: argmax)
        var size = argmax
        var mib = [CTL_KERN, KERN_PROCARGS2, Int32(pid)]
        guard sysctl(&mib, 3, &buffer, &size, nil, 0) == 0, size > MemoryLayout<Int32>.size else {
            return nil
        }

        return buffer.withUnsafeBufferPointer { raw -> String? in
            let base = raw.baseAddress!
            var argc: Int32 = 0
            memcpy(&argc, base, MemoryLayout<Int32>.size)
            guard argc > 0 else { return nil }

            var cursor = MemoryLayout<Int32>.size
            let end = size

            // Skip the exec path (NUL-terminated) then any NUL padding before argv[0].
            while cursor < end && base[cursor] != 0 { cursor += 1 }
            while cursor < end && base[cursor] == 0 { cursor += 1 }

            var args: [String] = []
            var collected: Int32 = 0
            while cursor < end && collected < argc {
                let startIdx = cursor
                while cursor < end && base[cursor] != 0 { cursor += 1 }
                let arg = String(cString: Array(raw[startIdx..<cursor]) + [0])
                if !arg.isEmpty { args.append(arg) }
                collected += 1
                cursor += 1 // step over the NUL separator
            }
            return args.isEmpty ? nil : args.joined(separator: " ")
        }
    }

    // MARK: - Ports

    /// Listening TCP ports for a process: walk its fd table for socket fds, keep
    /// TCP sockets in the LISTEN state, collect the local port. Mirrors the
    /// linows `/proc/[pid]/fd` + socket-inode scan.
    private static func listeningPorts(pid: Int32) -> [Int] {
        let bufferSize = proc_pidinfo(pid, PROC_PIDLISTFDS, 0, nil, 0)
        guard bufferSize > 0 else { return [] }

        let count = Int(bufferSize) / MemoryLayout<proc_fdinfo>.stride
        var fds = [proc_fdinfo](repeating: proc_fdinfo(), count: count)
        let filled = proc_pidinfo(pid, PROC_PIDLISTFDS, 0, &fds, bufferSize)
        guard filled > 0 else { return [] }
        let actual = Int(filled) / MemoryLayout<proc_fdinfo>.stride

        var ports = Set<Int>()
        for i in 0..<actual {
            guard fds[i].proc_fdtype == UInt32(PROX_FDTYPE_SOCKET) else { continue }
            var socketInfo = socket_fdinfo()
            let size = Int32(MemoryLayout<socket_fdinfo>.size)
            let rc = proc_pidfdinfo(pid, fds[i].proc_fd, PROC_PIDFDSOCKETINFO, &socketInfo, size)
            guard rc == size else { continue }
            guard socketInfo.psi.soi_kind == SOCKINFO_TCP else { continue }
            let tcp = socketInfo.psi.soi_proto.pri_tcp
            guard tcp.tcpsi_state == Int32(TSI_S_LISTEN) else { continue }
            // insi_lport is network byte order; ntohs -> host order.
            let netPort = UInt16(truncatingIfNeeded: tcp.tcpsi_ini.insi_lport)
            let port = Int(CFSwapInt16BigToHost(netPort))
            if port > 0 { ports.insert(port) }
        }
        return ports.sorted()
    }
}

/// Why a kill failed, so the UI can show a clear reason rather than a silent
/// no-op (per the handoff's "surface a clear error" rule for privileged kills).
enum ProcessKillError: Error, Equatable {
    case permissionDenied
    case noSuchProcess
    case other(String)

    var message: String {
        switch self {
        case .permissionDenied: return "permission denied (protected or another user's process)"
        case .noSuchProcess: return "process no longer exists"
        case .other(let detail): return detail
        }
    }
}
