import AppKit
import SwiftUI

/// Spotlight-style answer card pinned at the top of the results area. Shows one
/// block per source (Calculator / DuckDuckGo / Wikipedia), each appearing as it
/// resolves, and falls back to a streaming on-device answer when none hit.
struct AIAnswerCardView: View {
    /// How much themed floor sits under the card, so a light page behind the
    /// window cannot bleed through into the answer text.
    private static let legibilityFloorOpacity = 0.55

    private static let imageSide: CGFloat = 96
    /// Smaller beside the text, so the reading measure stays usable.
    private static let compactImageSide: CGFloat = 72
    private static let blockContentSpacing: CGFloat = 10

    @ObservedObject var controller: AIAnswerController
    @ObservedObject var themeStore: ThemeStore
    /// Compact has no preview pane, so the card is the only thing competing for
    /// vertical room: the thumbnail sits beside the text instead of above it.
    let isCompact: Bool

    private var fontSize: CGFloat { CGFloat(themeStore.settings.fontSize) }

    private var hasLLM: Bool { !controller.llmAnswer.isEmpty }
    private var isEmptySoFar: Bool { controller.items.isEmpty && !hasLLM }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            header

            ScrollView(.vertical, showsIndicators: false) {
                VStack(alignment: .leading, spacing: 14) {
                    ForEach(controller.items) { item in
                        answerBlock(
                            text: item.text, source: item.source, url: item.url,
                            imageURL: item.imageURL)
                    }
                    if hasLLM {
                        answerBlock(
                            text: controller.llmAnswer, source: controller.llmSourceLabel, url: nil,
                            imageURL: nil)
                    }
                    statusLine
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .topLeading)
        .background(
            // A themed floor under the fill. The panel behind this card blurs
            // the desktop, so over a light page the card had nothing to stand
            // against and its own text washed out (issue #398). The fill alone
            // is 0.30, which is a tint, not a substrate.
            RoundedRectangle(cornerRadius: themeStore.barRadius, style: .continuous)
                .fill(themeStore.commandModeBackgroundColor())
                .opacity(Self.legibilityFloorOpacity)
                .overlay(
                    RoundedRectangle(cornerRadius: themeStore.barRadius, style: .continuous)
                        .fill(themeStore.controlFillColor())
                )
        )
        .overlay(
            RoundedRectangle(cornerRadius: themeStore.barRadius, style: .continuous)
                .strokeBorder(themeStore.liftColor(opacity: 0.08), lineWidth: 1)
        )
    }

    private var header: some View {
        HStack(spacing: 6) {
            Image(systemName: "sparkles")
                .font(.system(size: fontSize, weight: .semibold))
                .foregroundStyle(themeStore.accentColor())

            Text(controller.question.isEmpty ? "Apple Intelligence" : controller.question)
                .font(themeStore.uiFont(size: fontSize, weight: .semibold))
                .foregroundStyle(themeStore.fontColor())
                .lineLimit(1)

            Spacer(minLength: 8)

            if controller.state == .streaming {
                ProgressView()
                    .controlSize(.small)
                    .scaleEffect(0.7)
            }
        }
    }

    private func answerBlock(text: String, source: String, url: URL?, imageURL: URL?) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 6) {
                sourceLabel(source, url: url)

                Spacer(minLength: 8)

                Button {
                    copy(text)
                } label: {
                    Image(systemName: "doc.on.doc")
                        .font(.system(size: fontSize - 2, weight: .semibold))
                        .foregroundStyle(themeStore.mutedTextColor())
                }
                .buttonStyle(.plain)
                .help("Copy this answer")
            }

            if isCompact, let imageURL {
                HStack(alignment: .top, spacing: Self.blockContentSpacing) {
                    thumbnail(imageURL, url: url, side: Self.compactImageSide)
                    answerText(text)
                }
            } else {
                VStack(alignment: .leading, spacing: Self.blockContentSpacing) {
                    if let imageURL {
                        thumbnail(imageURL, url: url, side: Self.imageSide)
                    }
                    answerText(text)
                }
            }
        }
    }

    private func thumbnail(_ imageURL: URL, url: URL?, side: CGFloat) -> some View {
        AsyncImage(url: imageURL) { phase in
            if let image = phase.image {
                image.resizable().aspectRatio(contentMode: .fill)
            } else {
                Color.clear
            }
        }
        .frame(width: side, height: side)
        .clipShape(RoundedRectangle(cornerRadius: themeStore.controlRadius, style: .continuous))
        .contentShape(RoundedRectangle(cornerRadius: themeStore.controlRadius, style: .continuous))
        .pointingHandCursor(enabled: url != nil)
        .onTapGesture { open(url) }
    }

    private func answerText(_ text: String) -> some View {
        Text(text)
            .font(themeStore.uiFont(size: fontSize, weight: .regular))
            .foregroundStyle(themeStore.secondaryTextColor())
            .lineSpacing(fontSize * 0.18)
            .textSelection(.enabled)
            .fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity, alignment: .leading)
    }

    @ViewBuilder
    private func sourceLabel(_ source: String, url: URL?) -> some View {
        let hasURL = url != nil
        HStack(spacing: 3) {
            Text(source.uppercased())
                .font(themeStore.uiFont(size: fontSize - 3, weight: .bold))
                .foregroundStyle(hasURL ? themeStore.accentColor() : themeStore.mutedTextColor())
            if hasURL {
                Image(systemName: "arrow.up.forward")
                    .font(.system(size: fontSize - 5, weight: .bold))
                    .foregroundStyle(themeStore.accentColor())
            }
        }
        // A generous, reliable hit area - the old Button was a tiny, flaky target
        // inside the borderless panel.
        .padding(.vertical, 2)
        .padding(.trailing, 6)
        .contentShape(Rectangle())
        .pointingHandCursor(enabled: hasURL)
        .onTapGesture { open(url) }
        .help(hasURL ? "Open source: \(url?.absoluteString ?? "")" : "")
    }

    private func open(_ url: URL?) {
        guard let url else { return }
        NSWorkspace.shared.open(url)
    }

    @ViewBuilder
    private var statusLine: some View {
        if controller.state == .streaming, isEmptySoFar {
            Text("Thinking…")
                .font(themeStore.uiFont(size: fontSize, weight: .regular))
                .foregroundStyle(themeStore.mutedTextColor())
        } else if controller.state == .failed, isEmptySoFar {
            Text("Couldn't find an answer.")
                .font(themeStore.uiFont(size: fontSize, weight: .regular))
                .foregroundStyle(themeStore.mutedTextColor())
        }
    }

    private func copy(_ text: String) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(trimmed, forType: .string)
    }
}
