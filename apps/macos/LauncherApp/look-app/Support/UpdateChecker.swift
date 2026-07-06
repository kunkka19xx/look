import AppKit
import Combine
import Foundation
import OSLog

/// Checks GitHub Releases for a newer version of Look and, if one exists,
/// publishes it so the UI can show a non-intrusive "update available" notice.
///
/// This is notify-only: Look is distributed via Homebrew, so we never download
/// or replace the bundle ourselves. The notice links to the release page and
/// the user runs `brew upgrade --cask kunkka19xx/tap/look` (or downloads the
/// asset). Homebrew stays the source of truth for the installed version.
final class UpdateChecker: ObservableObject {
    static let shared = UpdateChecker()

    struct AvailableUpdate: Equatable {
        let version: String
        let releaseURL: URL
    }

    /// Non-nil when a newer release exists and the user hasn't dismissed it.
    /// Mutated only on the main actor so SwiftUI observers stay consistent.
    @Published private(set) var availableUpdate: AvailableUpdate?

    /// True while a network check is in flight (for "Checking…" UI).
    @Published private(set) var isChecking = false

    /// Human-readable result of the most recent *manual* check, for feedback
    /// in Settings. nil during automatic background checks so we never nag.
    @Published private(set) var statusMessage: String?

    private let latestReleaseURL = URL(string: "https://api.github.com/repos/kunkka19xx/look/releases/latest")!
    private let session: URLSession
    private let defaults: UserDefaults
    private let log = Logger(subsystem: "noah-code.Look", category: "UpdateChecker")

    /// Don't hit the network more than once per this interval across launches.
    private let minCheckInterval: TimeInterval = 60 * 60 * 12  // 12 hours

    private static let lastCheckKey = "look.update.lastCheckEpoch"
    private static let dismissedVersionKey = "look.update.dismissedVersion"

    init(session: URLSession = .shared, defaults: UserDefaults = .standard) {
        self.session = session
        self.defaults = defaults
    }

    /// Check for an update, honoring the throttle. Safe to call on every launch.
    /// Pass `force: true` to bypass the throttle (e.g. a manual "Check now").
    @MainActor
    func checkForUpdates(force: Bool = false) {
        guard !isChecking else { return }

        if !force {
            let last = defaults.double(forKey: Self.lastCheckKey)
            if last > 0, Date().timeIntervalSince1970 - last < minCheckInterval {
                return
            }
        }

        guard let current = AppVersion.short else {
            log.debug("Skipping update check: no current version in bundle")
            if force { statusMessage = "Couldn't read the current version" }
            return
        }

        isChecking = true
        // Only show inline feedback for user-initiated ("force") checks.
        statusMessage = force ? "Checking…" : nil
        Task { @MainActor in
            defer { self.isChecking = false }
            await self.performCheck(currentVersion: current, isManual: force)
        }
    }

    /// Hide the notice without persisting a dismissal - used once the user has
    /// kicked off an update. If that upgrade fails, a later check resurfaces it.
    @MainActor
    func hideNotice() {
        availableUpdate = nil
        statusMessage = nil
    }

    /// Launch the Homebrew upgrade and hide the notice. Convenience for the
    /// "Update" buttons so they don't have to coordinate the two calls.
    @MainActor
    func startUpdate() {
        UpdateChecker.runHomebrewUpgrade()
        hideNotice()
    }

    /// Hide the current notice and remember the version so we don't nag again
    /// until an even newer release appears.
    @MainActor
    func dismissCurrent() {
        if let version = availableUpdate?.version {
            defaults.set(version, forKey: Self.dismissedVersionKey)
        }
        availableUpdate = nil
    }

    @MainActor
    private func performCheck(currentVersion: String, isManual: Bool) async {
        var request = URLRequest(url: latestReleaseURL)
        request.setValue("application/vnd.github+json", forHTTPHeaderField: "Accept")
        request.timeoutInterval = 15

        do {
            let (data, response) = try await session.data(for: request)
            guard let http = response as? HTTPURLResponse, http.statusCode == 200 else {
                log.debug("Update check non-200 response")
                if isManual { statusMessage = "Couldn't check for updates" }
                return
            }

            defaults.set(Date().timeIntervalSince1970, forKey: Self.lastCheckKey)

            guard
                let json = try JSONSerialization.jsonObject(with: data) as? [String: Any],
                let tag = json["tag_name"] as? String
            else {
                if isManual { statusMessage = "Couldn't check for updates" }
                return
            }

            // Ignore drafts/prereleases - only stable updates should nag users.
            if (json["prerelease"] as? Bool) == true || (json["draft"] as? Bool) == true {
                if isManual { statusMessage = "You're on the latest version (\(currentVersion))" }
                return
            }

            let latest = Self.normalizedVersion(tag)
            guard Self.isVersion(latest, newerThan: Self.normalizedVersion(currentVersion)) else {
                availableUpdate = nil
                if isManual { statusMessage = "You're on the latest version (\(currentVersion))" }
                return
            }

            let urlString = (json["html_url"] as? String) ?? "https://github.com/kunkka19xx/look/releases/latest"
            guard let releaseURL = URL(string: urlString) else { return }

            // A manual check overrides a prior dismissal of this version.
            if !isManual, defaults.string(forKey: Self.dismissedVersionKey) == latest {
                availableUpdate = nil
                return
            }

            availableUpdate = AvailableUpdate(version: latest, releaseURL: releaseURL)
            if isManual { statusMessage = "Update available: Look \(latest)" }
            log.info("Update available: \(latest, privacy: .public) (current \(currentVersion, privacy: .public))")
        } catch {
            log.debug("Update check failed: \(error.localizedDescription, privacy: .public)")
            if isManual { statusMessage = "Couldn't check for updates" }
        }
    }

    /// The Homebrew command users run to upgrade Look.
    static let homebrewUpgradeCommand = "brew upgrade --cask kunkka19xx/tap/look"

    /// Bundle id of the release app the Homebrew cask installs. The dev build
    /// uses the ".Dev" suffix, but brew always upgrades the release app - so the
    /// relaunch targets this id, not the running bundle.
    static let productionBundleID = "noah-code.Look"

    /// Launch Terminal running the Homebrew upgrade. The app isn't sandboxed, so
    /// we drop a temporary executable `.command` file and open it - Terminal runs
    /// it without needing Automation (Apple Events) permission. The user sees the
    /// output and can authenticate if Homebrew asks. Returns false if we couldn't
    /// stage the script. Note: brew may quit/replace Look while upgrading, which
    /// is expected for a cask.
    @discardableResult
    static func runHomebrewUpgrade() -> Bool {
        // After a cask upgrade the new bundle is on disk, but the running
        // process is still the old binary - so on success we quit and relaunch
        // Look for the user instead of asking them to restart it manually.
        let script = """
        #!/bin/bash
        export PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"
        echo "Updating Look via Homebrew…"
        echo ""
        \(homebrewUpgradeCommand)
        status=$?
        echo ""
        if [ $status -eq 0 ]; then
          echo "Update complete - relaunching Look…"
          osascript -e 'tell application id "\(productionBundleID)" to quit' >/dev/null 2>&1
          sleep 1
          open -b "\(productionBundleID)" 2>/dev/null || open "/Applications/Look.app"
          echo "Look has been relaunched. You can close this window."
        else
          echo "Update failed (exit $status). See the output above, or run:"
          echo "  \(homebrewUpgradeCommand)"
        fi
        """
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("look-update.command")
        do {
            try script.write(to: url, atomically: true, encoding: .utf8)
            try FileManager.default.setAttributes(
                [.posixPermissions: 0o755], ofItemAtPath: url.path)
            NSWorkspace.shared.open(url)
            return true
        } catch {
            return false
        }
    }

    /// Strip a leading "v" and surrounding whitespace: "v1.2.0" -> "1.2.0".
    static func normalizedVersion(_ raw: String) -> String {
        var s = raw.trimmingCharacters(in: .whitespaces)
        if s.hasPrefix("v") || s.hasPrefix("V") {
            s.removeFirst()
        }
        return s
    }

    /// Numeric dotted-version comparison. "1.10.0" > "1.9.0". Non-numeric or
    /// missing components are treated as 0, so it degrades gracefully.
    static func isVersion(_ lhs: String, newerThan rhs: String) -> Bool {
        let a = components(lhs)
        let b = components(rhs)
        let count = max(a.count, b.count)
        for i in 0..<count {
            let x = i < a.count ? a[i] : 0
            let y = i < b.count ? b[i] : 0
            if x != y { return x > y }
        }
        return false
    }

    private static func components(_ version: String) -> [Int] {
        version
            .split(separator: ".")
            .map { Int($0.prefix { $0.isNumber }) ?? 0 }
    }
}
