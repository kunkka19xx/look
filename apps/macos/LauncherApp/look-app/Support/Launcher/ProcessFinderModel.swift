import Combine
import Foundation

/// Backing state for the `ps"` process finder: an enumerated snapshot re-scored
/// per keystroke (never re-enumerated) plus per-selection detail and on-demand
/// CPU caches. `ProcessService` calls run detached; results land on the main
/// actor.
@MainActor
final class ProcessFinderModel: ObservableObject {
    /// Scoring input, derived once per snapshot so per-keystroke scoring doesn't
    /// rebuild it.
    @Published private(set) var candidates: [ProcessScoring.Candidate] = []
    /// Per-selection detail, cached by pid so arrow-key revisits are instant.
    @Published private(set) var details: [Int32: ProcessDetail] = [:]
    /// CPU% measured on-demand (Enter), cached by pid. Absent = not yet measured.
    @Published private(set) var cpu: [Int32: Double] = [:]
    /// True while a CPU sample is in flight, so the view can show a hint.
    @Published private(set) var measuringCPU: Set<Int32> = []

    private var snapshotLoaded = false
    private var refreshTask: Task<Void, Never>?
    /// Detail loads in flight, so arrow-key nav doesn't stack duplicate fetches.
    private var loadingDetail: Set<Int32> = []
    /// Bumped on every snapshot swap; the ranking memo keys off it.
    private var snapshotRevision = 0
    private var rankedTerm: String?
    private var rankedRevision = -1
    private var rankedRows: [ProcessScoring.Candidate] = []

    /// Re-enumerate the process table. Called on mode entry and after a kill.
    /// Clears the detail/cpu caches since pids may have been reused.
    func refreshSnapshot() {
        refreshTask?.cancel()
        refreshTask = Task {
            let snapshot = await Task.detached(priority: .userInitiated) {
                ProcessService.enumerate()
            }.value
            guard !Task.isCancelled else { return }
            candidates = snapshot.map { .init(name: $0.name, pid: $0.pid, ports: $0.ports) }
            snapshotRevision += 1
            details.removeAll()
            cpu.removeAll()
            snapshotLoaded = true
        }
    }

    /// Rows for `term`, memoized against the snapshot: the view reads this
    /// several times per body pass and a miss costs one FFI call per candidate.
    /// Empty term lists alphabetically.
    func ranked(term: String, fuzzy: (String) -> Int?) -> [ProcessScoring.Candidate] {
        if rankedTerm == term, rankedRevision == snapshotRevision { return rankedRows }

        if term.isEmpty {
            rankedRows = candidates.sorted {
                $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending
            }
        } else {
            rankedRows = ProcessScoring.rankProcesses(candidates, query: term, fuzzy: fuzzy)
        }
        rankedTerm = term
        rankedRevision = snapshotRevision
        return rankedRows
    }

    /// Enumerate once on first entry; later entries reuse the snapshot until an
    /// explicit `refreshSnapshot()`.
    func loadSnapshotIfNeeded() {
        guard !snapshotLoaded else { return }
        refreshSnapshot()
    }

    /// Load detail for a selected pid (cheap reads), caching the result. No-op
    /// if already cached.
    func loadDetail(pid: Int32) {
        guard details[pid] == nil, !loadingDetail.contains(pid) else { return }
        loadingDetail.insert(pid)
        Task {
            let detail = await Task.detached(priority: .userInitiated) {
                ProcessService.detail(pid: pid)
            }.value
            loadingDetail.remove(pid)
            guard let detail else { return }
            details[pid] = detail
        }
    }

    /// Sample CPU% for a pid over ~200 ms (on-demand, bound to Enter). Caches
    /// the measurement; safe to call repeatedly (re-measures).
    func measureCPU(pid: Int32) {
        guard !measuringCPU.contains(pid) else { return }
        measuringCPU.insert(pid)
        Task {
            let value = await Task.detached(priority: .userInitiated) {
                ProcessService.cpu(pid: pid)
            }.value
            measuringCPU.remove(pid)
            if let value { cpu[pid] = value }
        }
    }

    /// SIGKILL a process, then refresh the snapshot so it drops from the list.
    /// Returns the outcome for the caller to surface (banner).
    func kill(pid: Int32) -> Result<String, ProcessKillError> {
        let result = ProcessService.kill(pid: pid)
        if case .success = result {
            refreshSnapshot()
        }
        return result
    }

    /// Reset so the next entry re-enumerates fresh (called when leaving mode).
    func invalidate() {
        refreshTask?.cancel()
        refreshTask = nil
        loadingDetail.removeAll()
        snapshotLoaded = false
    }
}
