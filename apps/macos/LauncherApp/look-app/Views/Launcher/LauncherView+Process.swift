import AppKit
import SwiftUI

/// `ps"` process-finder mode: fuzzy-matches running processes and drives the
/// per-selection preview, on-demand CPU (Enter), kill (Cmd+D), and copy-PID
/// (Cmd+C). Keys follow macOS conventions (Cmd, not Ctrl).
extension LauncherView {
    var isProcessQuery: Bool { LauncherProcessFeature.isProcessQuery(query) }

    /// Scored process rows. Empty query → alphabetical listing; otherwise ranked
    /// by `ProcessScoring` (numeric port/PID tiers above fuzzy name matches),
    /// capped at the result limit. Scored against the cached snapshot.
    var processResults: [LauncherResult] {
        guard let term = LauncherProcessFeature.searchTerm(from: query) else { return [] }
        let candidates = processModel.candidates
        let limit = AppConstants.Launcher.Process.resultLimit

        let ranked: [ProcessScoring.Candidate]
        if term.isEmpty {
            ranked = candidates.sorted { $0.name.localizedCaseInsensitiveCompare($1.name) == .orderedAscending }
        } else {
            let lowered = term.lowercased()
            ranked = ProcessScoring.rankProcesses(candidates, query: term) { title in
                bridge.fuzzyScore(query: lowered, title: title)
            }
        }
        return ranked.prefix(limit).map(LauncherProcessFeature.makeResult)
    }

    /// The pid of the currently selected process row, or nil when the selection
    /// isn't a process.
    var selectedProcessPID: Int32? {
        guard isProcessQuery, let selectedResultID else { return nil }
        return LauncherProcessFeature.pid(fromResultID: selectedResultID)
    }

    func loadProcessDetailForSelection() {
        guard let pid = selectedProcessPID else { return }
        processModel.loadDetail(pid: pid)
    }

    func processDetail(for result: LauncherResult) -> ProcessDetail? {
        guard let pid = result.processPID else { return nil }
        return processModel.details[pid]
    }

    func processCPU(for result: LauncherResult) -> Double? {
        guard let pid = result.processPID else { return nil }
        return processModel.cpu[pid]
    }

    func isMeasuringCPU(for result: LauncherResult) -> Bool {
        guard let pid = result.processPID else { return false }
        return processModel.measuringCPU.contains(pid)
    }

    /// Enter measures the selected process's CPU% on demand (see `ProcessService.cpu`).
    func measureCPUForSelectedProcess() {
        guard let pid = selectedProcessPID else { return }
        processModel.measureCPU(pid: pid)
    }

    /// Cmd+D sends SIGKILL, surfacing a clear error on failure (protected /
    /// another user's process) rather than a silent no-op.
    func killSelectedProcess() {
        guard let selected = displayedResults.first(where: { $0.id == selectedResultID }),
              let pid = selected.processPID
        else { return }
        let name = selected.title
        switch processModel.kill(pid: pid) {
        case .success:
            showBanner("Killed \(name) (PID \(pid))", style: .success, duration: 1.6)
            DispatchQueue.main.async {
                if !displayedResults.contains(where: { $0.id == selectedResultID }) {
                    selectedResultID = displayedResults.first?.id
                }
            }
        case .failure(let error):
            showBanner("Could not kill \(name): \(error.message)", style: .error, duration: 2.6)
        }
    }

    /// Cmd+C copies the selected process's PID.
    func copySelectedProcessPID() {
        guard let pid = selectedProcessPID else { return }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(String(pid), forType: .string)
        showBanner("Copied PID \(pid)", style: .success, duration: 1.2)
    }
}
