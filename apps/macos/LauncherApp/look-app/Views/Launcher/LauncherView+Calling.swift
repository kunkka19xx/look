import Foundation

/// The pinned "Call <name>" rows. The grammar ("is this a call request?") and
/// the URL each modality needs live in the shared `core/ai` crate via
/// `EngineBridge`; Contacts is the platform's. This file is presentation and
/// placement only. Mirrors `LauncherView+Meeting.swift`.
extension LauncherView {
    private enum Copy {
        static let enterHint = "Enter to call"
        static let enterMessageHint = "Enter to message"
        /// A main-bar row per way to reach someone, but not a screenful: past
        /// this the query should say which person or which number.
        static let rowLimit = 5
    }

    /// Rows for `call`-style queries, best first, or empty. Unlike the meeting
    /// row there can be several - Mom's mobile and her work number are both
    /// answers - and the results list is already a list, so they go in as rows
    /// rather than behind a picker.
    ///
    /// The Contacts read behind this is cached (see `ContactsService`), so it
    /// is safe to evaluate on every keystroke like the other pinned rows.
    var callResults: [LauncherResult] {
        guard allowsSuggestionRows, let request = bridge.callQuery(query) else { return [] }
        // A name that matches nobody shows no row at all, which is what keeps
        // "call stack" an ordinary file search.
        let wanted = request.modality ?? bridge.defaultCallModality
        let matches = ContactsService.shared.matches(name: request.name)

        // Deduped by id, which is the URL: a number shared by two contacts (a
        // family landline) would otherwise be two rows that do exactly the
        // same thing, with the same id - and a duplicate id breaks row
        // identity in the results list.
        var seen = Set<String>()
        return matches.flatMap { match in
            match.handles
                .filter { $0.modalityID == wanted }
                .compactMap { handle in row(match: match, handle: handle) }
        }
        .filter { seen.insert($0.id).inserted }
        .prefix(Copy.rowLimit)
        .enumerated()
        .map { index, result in
            var ranked = result
            // Descending from `.max` keeps the first row above the calc row and
            // the rest in the order Contacts gave them.
            ranked.score = Int.max - index
            return ranked
        }
    }

    private func row(match: ContactMatch, handle: ContactHandle) -> LauncherResult? {
        guard let url = bridge.callURL(modality: handle.modalityID, handle: handle.handle) else {
            return nil
        }
        let isMessage = handle.modalityID == "message"
        var detail = [handle.modalityLabel]
        if let label = handle.handleLabel, !label.isEmpty { detail.append(label) }
        detail.append(handle.handle)
        detail.append(isMessage ? Copy.enterMessageHint : Copy.enterHint)

        var result = LauncherResult(
            id: AppConstants.Launcher.Call.resultID(url: url),
            kind: .app,
            title: "\(isMessage ? "Message" : "Call") \(match.name)",
            subtitle: detail.joined(separator: "  •  "),
            // No path: the row opens a URL, and there is no file behind it.
            path: "",
            score: .max
        )
        result.linkKindLabel = handle.modalityLabel
        result.linkDetail = [handle.handleLabel, handle.handle]
            .compactMap { $0 }
            .joined(separator: "  ·  ")
        return result
    }
}
