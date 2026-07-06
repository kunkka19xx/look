import AppKit
import SwiftUI

/// Spotlight-style answer card pinned at the top of the results area. Shows one
/// block per source (Calculator / DuckDuckGo / Wikipedia), each appearing as it
/// resolves, and falls back to a streaming on-device answer when none hit.
struct AIAnswerCardView: View {
    @ObservedObject var controller: AIAnswerController
    @ObservedObject var themeStore: ThemeStore

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
                            text: controller.llmAnswer, source: "Apple Intelligence", url: nil,
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
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(themeStore.controlFillColor())
        )
        .overlay(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .strokeBorder(.white.opacity(0.08), lineWidth: 1)
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

            VStack(alignment: .leading, spacing: 10) {
                if let imageURL {
                    AsyncImage(url: imageURL) { phase in
                        if let image = phase.image {
                            image.resizable().aspectRatio(contentMode: .fill)
                        } else {
                            Color.clear
                        }
                    }
                    .frame(width: 96, height: 96)
                    .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
                    .contentShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
                    .pointingHandCursor(enabled: url != nil)
                    .onTapGesture { open(url) }
                }

                Text(text)
                    .font(themeStore.uiFont(size: fontSize, weight: .regular))
                    .foregroundStyle(themeStore.secondaryTextColor())
                    .lineSpacing(fontSize * 0.18)
                    .textSelection(.enabled)
                    .fixedSize(horizontal: false, vertical: true)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
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

extension View {
    /// Shows the pointing-hand cursor on hover when `enabled`.
    @ViewBuilder
    fileprivate func pointingHandCursor(enabled: Bool) -> some View {
        if enabled {
            onHover { inside in
                if inside { NSCursor.pointingHand.push() } else { NSCursor.pop() }
            }
        } else {
            self
        }
    }
}
