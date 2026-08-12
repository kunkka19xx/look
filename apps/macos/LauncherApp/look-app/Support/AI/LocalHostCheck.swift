import Foundation

/// Whether an Ollama endpoint is on this machine. Gates whether private context
/// (calendar, clipboard, remembered facts) may be attached to a prompt, so it
/// is deliberately strict: anything it cannot prove is local counts as remote.
///
/// Pure Foundation so it lives in the `LauncherLogic` package and is tested -
/// the failure mode of a sloppy check ("localhost.evil.com" reading as local)
/// is silently shipping the user's calendar off-device.
nonisolated enum LocalHostCheck {
    private static let localNames: Set<String> = [
        "localhost", "127.0.0.1", "0.0.0.0", "::1", "[::1]",
    ]

    /// Ollama can proxy a "cloud" model (`gpt-oss:120b-cloud`) through the
    /// LOCAL daemon to its hosted service, so a loopback host proves nothing
    /// about where inference happens. Treat those as remote.
    static func isCloudModel(_ model: String) -> Bool {
        let name = model.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard let tag = name.split(separator: ":").last else { return false }
        return tag == "cloud" || tag.hasSuffix("-cloud")
    }

    /// Whether inference for this host+model stays on the machine. Both must
    /// hold: a loopback endpoint AND a model that is not cloud-routed.
    static func isLocalInference(host: String, model: String) -> Bool {
        isLocal(host: host) && !isCloudModel(model)
    }

    /// True only for loopback endpoints. A hostname is matched whole, never by
    /// suffix or prefix.
    static func isLocal(host: String) -> Bool {
        let trimmed = host.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !trimmed.isEmpty else { return false }

        // Always parse as a URL so host extraction (ports, IPv6 brackets,
        // credentials, paths) is Foundation's job, not a hand-rolled split.
        let candidate = trimmed.contains("://") ? trimmed : "http://" + trimmed
        guard let parsed = URLComponents(string: candidate) else { return false }
        // Credentials or a non-http scheme mean this is not a plain local
        // daemon; don't reason loosely about it.
        guard parsed.user == nil, parsed.password == nil else { return false }
        guard parsed.scheme == "http" || parsed.scheme == "https" else { return false }
        guard let name = parsed.host?.lowercased(), !name.isEmpty else { return false }

        if localNames.contains(name) { return true }
        // 127.0.0.0/8 is all loopback.
        return name.hasPrefix("127.")
            && name.split(separator: ".").count == 4
            && name.split(separator: ".").allSatisfy { UInt8($0) != nil }
    }
}
