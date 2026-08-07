import AppKit
import SwiftUI

/// The `/speed` panel: the dial in the middle, its two rates below it, and a
/// plain-language verdict under that. It owns no text field, so the test starts
/// when the panel opens and `R` runs another.
struct SpeedTestView: View {
    var controller: SpeedTestController
    let themeStore: ThemeStore

    private enum Layout {
        static let stackSpacing: CGFloat = 14
        static let legendSpacing: CGFloat = 28
        static let legendDotSize: CGFloat = 7
        static let verdictSpacing: CGFloat = 3
        static let gaugeMinSize: CGFloat = 210
        static let legendValueDelta = 2
        static let legendLabelDelta = -2
        /// Dims the standing reading while a fresh one is being measured.
        static let supersededOpacity = 0.5
        /// Stands in for the public address until it is revealed. Fixed width,
        /// so the line doesn't jump when it is.
        static let maskedAddress = "•••.•••.•••.•••"
        /// Matches core's own word for a phase that measured nothing.
        static let unavailable = "n/a"
        /// How long a chip reads "copied" before going back to the address.
        static let copiedFeedbackSeconds: Double = 1.4
    }

    /// The public address starts hidden: it identifies the connection, and this
    /// panel is a screenshot away from anywhere.
    @State private var revealsPublicAddress = false
    /// Which chip was last copied, for its brief confirmation.
    @State private var copiedAddress: AddressKind?

    private enum AddressKind: String {
        case lan = "LAN"
        case wan = "WAN"
    }

    var body: some View {
        VStack(spacing: Layout.stackSpacing) {
            addressLine

            SpeedGaugeView(
                downloadBitsPerSecond: controller.reading?.downloadBitsPerSecond ?? 0,
                uploadBitsPerSecond: controller.reading?.uploadBitsPerSecond ?? 0,
                latencyMs: controller.reading?.latencyMs,
                latencyDisplay: controller.reading?.latencyDisplay ?? Layout.unavailable,
                latencyLevel: controller.reading?.latencyLevel,
                themeStore: themeStore
            )
            .frame(minHeight: Layout.gaugeMinSize)
            .opacity(controller.isRunning ? Layout.supersededOpacity : 1)

            legend
            verdict
            carrierLine
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background {
            PlainKeyCommands(actions: [
                "r": { controller.start() },
                "e": { revealsPublicAddress.toggle() },
            ])
                .allowsHitTesting(false)
        }
        .onAppear { controller.startIfStale() }
        .onDisappear { controller.cancel() }
    }

    /// Both addresses, since they answer different questions: LAN is what you
    /// reach this machine on, WAN is what the far end of the test sees.
    private var addressLine: some View {
        HStack(spacing: 6) {
            Image(systemName: "network")

            if controller.localAddress == nil && controller.reading?.publicIp == nil {
                Text("No network")
            }

            if let local = controller.localAddress {
                addressChip(kind: .lan, address: local, shown: local)
            }

            if let publicAddress = controller.reading?.publicIp {
                Text("·")
                addressChip(
                    kind: .wan,
                    address: publicAddress,
                    shown: revealsPublicAddress ? publicAddress : Layout.maskedAddress
                )

                Button {
                    revealsPublicAddress.toggle()
                } label: {
                    Image(systemName: revealsPublicAddress ? "eye.slash" : "eye")
                }
                .buttonStyle(.plain)
                .pointingHandCursor()
                .help(revealsPublicAddress ? "Hide the public address (E)" : "Show the public address (E)")
            }
        }
        .font(mono(-2))
        .foregroundStyle(themeStore.mutedTextColor())
        .lineLimit(1)
        .task(id: copiedAddress) {
            guard copiedAddress != nil else { return }
            try? await Task.sleep(for: .seconds(Layout.copiedFeedbackSeconds))
            copiedAddress = nil
        }
    }

    /// One address, click to copy. A masked public address still copies in full:
    /// hiding it is about what the screen shows, not what you can take with you.
    private func addressChip(kind: AddressKind, address: String, shown: String) -> some View {
        let isCopied = copiedAddress == kind

        return Button {
            copy(address, as: kind)
        } label: {
            Text("\(kind.rawValue) \(shown)")
                // Swapped by an overlay rather than the text itself, so the
                // confirmation cannot shift the line.
                .opacity(isCopied ? 0 : 1)
                .overlay {
                    if isCopied {
                        Text("copied")
                            .foregroundStyle(themeStore.successColor())
                    }
                }
        }
        .buttonStyle(.plain)
        .pointingHandCursor()
        .help("Copy the \(kind.rawValue) address")
    }

    private func copy(_ address: String, as kind: AddressKind) {
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(address, forType: .string)
        copiedAddress = kind
    }

    /// Who carries the traffic and roughly where it lands, when the lookup
    /// answered. Sits under the verdict so the dial keeps the middle.
    @ViewBuilder
    private var carrierLine: some View {
        let parts = [
            controller.reading?.provider,
            controller.reading?.location,
            controller.reading?.downloadSource.map { "via \($0)" },
        ].compactMap { $0 }
        if !parts.isEmpty {
            Text(parts.joined(separator: "  ·  "))
                .font(mono(-3))
                .foregroundStyle(themeStore.mutedTextColor())
                .lineLimit(1)
        }
    }

    private var legend: some View {
        HStack(spacing: Layout.legendSpacing) {
            legendEntry(
                label: "DOWN",
                value: controller.reading?.downloadDisplay,
                color: themeStore.successColor()
            )
            legendEntry(
                label: "UP",
                value: controller.reading?.uploadDisplay,
                color: themeStore.accentColor()
            )
            legendEntry(
                label: "LATENCY",
                value: controller.reading?.latencyDisplay,
                color: themeStore.warningColor()
            )
        }
        .opacity(controller.isRunning ? Layout.supersededOpacity : 1)
    }

    private func legendEntry(label: String, value: String?, color: Color) -> some View {
        HStack(spacing: 7) {
            Circle()
                .fill(color)
                .frame(width: Layout.legendDotSize, height: Layout.legendDotSize)

            Text(label)
                .font(mono(Layout.legendLabelDelta))
                .foregroundStyle(themeStore.mutedTextColor())
                .tracking(1.4)

            Text(value ?? Layout.unavailable)
                .font(mono(Layout.legendValueDelta, weight: .medium))
                .foregroundStyle(themeStore.fontColor())
        }
    }

    @ViewBuilder
    private var verdict: some View {
        VStack(spacing: Layout.verdictSpacing) {
            Text(status)
                .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1)))
                .foregroundStyle(themeStore.secondaryTextColor())

            if let reading = controller.reading, !controller.isRunning {
                Text("\(reading.downloadVerdict) · latency \(reading.latencyVerdict)")
                    .font(mono(-3))
                    .foregroundStyle(themeStore.mutedTextColor())
                    .tracking(1)
                    .textCase(.uppercase)
            }

            if let message = controller.errorMessage {
                Label(message, systemImage: "exclamationmark.triangle")
                    .font(themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1)))
                    .foregroundStyle(themeStore.dangerColor())
            }
        }
        .multilineTextAlignment(.center)
    }

    private var status: String {
        if controller.isRunning {
            return "Measuring, \(controller.elapsedSeconds)s"
        }
        guard let reading = controller.reading else {
            return "Press R to measure"
        }
        return "Measured \(age(of: reading)), press R to run again"
    }

    /// Anything the controller would still reuse reads as "just now"; the
    /// relative formatter renders a fresh reading as "in 0 seconds".
    private func mono(_ delta: Int, weight: Font.Weight = .regular) -> Font {
        .system(
            size: CGFloat(themeStore.settings.fontSize + Double(delta)),
            weight: weight,
            design: .monospaced
        )
    }

    private func age(of reading: SpeedReading) -> String {
        if Date().timeIntervalSince(reading.measuredAt) < SpeedTestDefaults.autoRunFreshness {
            return "just now"
        }
        return Self.ageFormatter.localizedString(for: reading.measuredAt, relativeTo: Date())
    }

    private static let ageFormatter: RelativeDateTimeFormatter = {
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .full
        return formatter
    }()
}
