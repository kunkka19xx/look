import AVFoundation
import AppKit
import Foundation
import SwiftUI

struct LookupDefinitionPanelView: View {
    let definition: LookupDefinition?
    let emptyHint: String?
    let isWebMode: Bool
    @ObservedObject var themeStore: ThemeStore
    @State private var speechSynthesizer = AVSpeechSynthesizer()

    var body: some View {
        VStack(spacing: 0) {
            ScrollView(.vertical, showsIndicators: false) {
                VStack(alignment: .leading, spacing: 16) {
                    if let definition {
                        definitionContent(definition)
                    } else {
                        emptyState
                    }
                    Spacer(minLength: 0)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }

            footerAction
        }
        .padding(.horizontal, 12)
        .padding(.top, 12)
        .padding(.bottom, 8)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }

    @ViewBuilder
    private func definitionContent(_ definition: LookupDefinition) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .top, spacing: 10) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(definition.query)
                        .font(
                            isWebMode
                                ? themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize + 6), weight: Font.Weight.semibold)
                                : .system(size: CGFloat(themeStore.settings.fontSize + 6), weight: Font.Weight.semibold, design: .serif)
                        )
                        .foregroundStyle(themeStore.fontColor())
                    Text(definition.sourceLabel.uppercased())
                        .font(
                            isWebMode
                                ? themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 2), weight: Font.Weight.semibold)
                                : .system(size: CGFloat(themeStore.settings.fontSize - 2), weight: Font.Weight.bold, design: .rounded)
                        )
                        .foregroundStyle(themeStore.mutedTextColor())
                }

                Spacer(minLength: 0)

                Button {
                    speakText(definition.query)
                } label: {
                    Image(systemName: "speaker.wave.2.fill")
                        .font(.system(size: 15, weight: .semibold))
                        .foregroundStyle(themeStore.secondaryTextColor())
                }
                .buttonStyle(.plain)
                .padding(.top, 3)
            }

            Divider()
                .overlay(themeStore.dividerColor())

            ForEach(Array(definition.sections.enumerated()), id: \.offset) { _, section in
                translationSection(section)
            }
        }
    }

    private func translationSection(_ section: LookupTranslationSection) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                Text(section.label.uppercased())
                    .font(
                        isWebMode
                            ? themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize - 1), weight: Font.Weight.semibold)
                            : .system(size: CGFloat(themeStore.settings.fontSize - 1), weight: Font.Weight.bold, design: .serif)
                    )
                    .foregroundStyle(themeStore.secondaryTextColor())

                Spacer(minLength: 0)

                if let translated = section.translated, !translated.isEmpty {
                    Button {
                        speakText(translated)
                    } label: {
                        Image(systemName: "speaker.wave.1")
                            .font(.system(size: CGFloat(themeStore.settings.fontSize)))
                            .foregroundStyle(themeStore.mutedTextColor())
                    }
                    .buttonStyle(.plain)

                    if isWebMode {
                        Button {
                            copyToPasteboard(translated)
                        } label: {
                            Image(systemName: "doc.on.doc")
                                .font(.system(size: CGFloat(themeStore.settings.fontSize - 1), weight: .semibold))
                                .foregroundStyle(themeStore.mutedTextColor())
                        }
                        .buttonStyle(.plain)
                    }
                }
            }

            if let translated = section.translated, !translated.isEmpty {
                Text(translated)
                    .font(
                        isWebMode
                            ? themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize + 2), weight: Font.Weight.regular)
                            : .system(size: CGFloat(themeStore.settings.fontSize + 2), weight: Font.Weight.regular, design: .serif)
                    )
                    .foregroundStyle(themeStore.secondaryTextColor())
                    .lineSpacing(CGFloat(themeStore.settings.fontSize * 0.15))
                    .textSelection(.enabled)
            } else if section.failed {
                Text("No translation")
                    .font(
                        isWebMode
                            ? themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize), weight: Font.Weight.regular)
                            : .system(size: CGFloat(themeStore.settings.fontSize), weight: Font.Weight.regular, design: .serif)
                    )
                    .foregroundStyle(themeStore.mutedTextColor())
            }

            if let presentation = section.dictionaryDefinition {
                dictionaryPresentationView(presentation)
            }

            if section.label != definition?.sections.last?.label {
                Divider()
                    .overlay(themeStore.dividerColor())
            }
        }
    }

    private func dictionaryPresentationView(_ presentation: LookupPresentation) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            ForEach(Array(presentation.definitions.enumerated()), id: \.offset) { _, entry in
                definitionEntryView(entry)
            }
        }
        .padding(.top, 4)
    }

    private func definitionEntryView(_ entry: LookupDefinitionEntry) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            if let pos = entry.partOfSpeech {
                Text(pos)
                    .font(.system(size: CGFloat(themeStore.settings.fontSize), weight: .semibold, design: .serif))
                    .foregroundStyle(themeStore.secondaryTextColor())
                    .padding(.horizontal, 8)
                    .padding(.vertical, 2)
                    .background(themeStore.controlFillColor())
                    .cornerRadius(4)
            }

            ForEach(Array(entry.senses.enumerated()), id: \.offset) { _, sense in
                senseEntryView(sense)
            }
        }
    }

    private func senseEntryView(_ sense: LookupSenseEntry) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(alignment: .top, spacing: 6) {
                Text("\(sense.number).")
                    .font(.system(size: CGFloat(themeStore.settings.fontSize), weight: .semibold, design: .serif))
                    .foregroundStyle(themeStore.mutedTextColor())
                    .frame(width: 20, alignment: .trailing)

                Text(sense.definition)
                    .font(.system(size: CGFloat(themeStore.settings.fontSize), design: .serif))
                    .foregroundStyle(themeStore.secondaryTextColor())
                    .textSelection(.enabled)
            }

            if !sense.examples.isEmpty {
                ForEach(Array(sense.examples.enumerated()), id: \.offset) { _, example in
                    HStack(alignment: .top, spacing: 4) {
                        Text("•")
                            .font(.system(size: CGFloat(themeStore.settings.fontSize - 1)))
                            .foregroundStyle(themeStore.mutedTextColor())
                        Text(example)
                            .font(.system(size: CGFloat(themeStore.settings.fontSize - 1), design: .serif))
                            .foregroundStyle(themeStore.secondaryTextColor())
                            .italic()
                    }
                    .padding(.leading, 24)
                }
            }

            if !sense.synonyms.isEmpty || !sense.antonyms.isEmpty {
                HStack(spacing: 12) {
                    if !sense.synonyms.isEmpty {
                        Text("Syn: \(sense.synonyms.joined(separator: ", "))")
                            .font(.system(size: CGFloat(themeStore.settings.fontSize - 2), design: .serif))
                            .foregroundStyle(themeStore.mutedTextColor())
                    }
                    if !sense.antonyms.isEmpty {
                        Text("Ant: \(sense.antonyms.joined(separator: ", "))")
                            .font(.system(size: CGFloat(themeStore.settings.fontSize - 2), design: .serif))
                            .foregroundStyle(themeStore.mutedTextColor())
                    }
                }
                .padding(.leading, 24)
            }
        }
    }

    private var emptyState: some View {
        VStack(spacing: 8) {
            Image(systemName: "globe")
                .font(.system(size: 28))
                .foregroundStyle(themeStore.mutedTextColor())

            if let emptyHint, !emptyHint.isEmpty {
                Text(emptyHint)
                    .font(
                        isWebMode
                            ? themeStore.uiFont(size: CGFloat(themeStore.settings.fontSize + 2), weight: Font.Weight.medium)
                            : .system(size: CGFloat(themeStore.settings.fontSize + 2), weight: Font.Weight.medium, design: .serif)
                    )
                    .foregroundStyle(themeStore.secondaryTextColor())
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 24)
    }

    @ViewBuilder
    private var footerAction: some View {
        if isWebMode {
            openInBrowserButton
        } else {
            dictionaryInstallHint
        }
    }

    private var openInBrowserButton: some View {
        Button {
            openInBrowserTranslation()
        } label: {
            HStack(spacing: 8) {
                Image(systemName: "safari")
                    .font(.system(size: 14))

                Text("Open in Browser")
                    .font(.system(size: CGFloat(themeStore.settings.fontSize - 1), weight: .medium))

                Spacer(minLength: 0)

                Image(systemName: "arrow.up.forward")
                    .font(.system(size: 11, weight: .semibold))
            }
            .foregroundStyle(themeStore.accentColor())
            .padding(.vertical, 8)
            .padding(.horizontal, 12)
            .background(themeStore.controlFillColor())
            .cornerRadius(8)
        }
        .buttonStyle(.plain)
    }

    private var dictionaryInstallHint: some View {
        Button {
            if let word = definition?.query {
                openInDictionary(word)
            } else {
                openDictionaryPreferences()
            }
        } label: {
            HStack(spacing: 8) {
                Image(systemName: "book.closed")
                    .font(.system(size: 14))

                Text("Open in Dictionary")
                    .font(.system(size: CGFloat(themeStore.settings.fontSize - 1), weight: .medium))

                Spacer(minLength: 0)

                Image(systemName: "arrow.up.forward")
                    .font(.system(size: 11, weight: .semibold))
            }
            .foregroundStyle(themeStore.accentColor())
            .padding(.vertical, 8)
            .padding(.horizontal, 12)
            .background(themeStore.controlFillColor())
            .cornerRadius(8)
        }
        .buttonStyle(.plain)
    }

    private func openInBrowserTranslation() {
        let query = definition?.query.trimmingCharacters(in: CharacterSet.whitespacesAndNewlines) ?? ""
        var components = URLComponents(string: "https://translate.google.com/")
        if !query.isEmpty {
            components?.queryItems = [
                URLQueryItem(name: "sl", value: "auto"),
                URLQueryItem(name: "tl", value: "en"),
                URLQueryItem(name: "text", value: query),
                URLQueryItem(name: "op", value: "translate"),
            ]
        }

        guard let url = components?.url else { return }
        NSWorkspace.shared.open(url)
    }

    private func copyToPasteboard(_ text: String) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(trimmed, forType: .string)
    }

    private func speakText(_ text: String) {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }

        if speechSynthesizer.isSpeaking {
            speechSynthesizer.stopSpeaking(at: .immediate)
        }

        let utterance = AVSpeechUtterance(string: trimmed)
        utterance.rate = 0.46

        if isJapaneseText(trimmed) {
            utterance.voice = AVSpeechSynthesisVoice(language: "ja-JP")
        } else if isVietnameseText(trimmed) {
            utterance.voice = AVSpeechSynthesisVoice(language: "vi-VN")
        } else if let englishVoice = AVSpeechSynthesisVoice(language: "en-US") {
            utterance.voice = englishVoice
        } else if let preferred = Locale.preferredLanguages.first,
                  let localeVoice = AVSpeechSynthesisVoice(language: preferred) {
            utterance.voice = localeVoice
        }

        speechSynthesizer.speak(utterance)
    }

    private func isJapaneseText(_ text: String) -> Bool {
        for scalar in text.unicodeScalars {
            let v = scalar.value
            if (0x3040...0x30FF).contains(v) || (0x4E00...0x9FFF).contains(v) || (0x3400...0x4DBF).contains(v) {
                return true
            }
        }
        return false
    }

    private func isVietnameseText(_ text: String) -> Bool {
        for scalar in text.unicodeScalars {
            let v = scalar.value
            if (0x00C0...0x00FF).contains(v) || (0x0100...0x017F).contains(v) || (0x1EA0...0x1EF9).contains(v) {
                return true
            }
        }
        return false
    }

    private func openInDictionary(_ word: String) {
        NSWorkspace.shared.open(URL(string: "dict:///\(word.addingPercentEncoding(withAllowedCharacters: .urlPathAllowed) ?? word)")!)
    }

    private func openDictionaryPreferences() {
        let script = """
        tell application "Dictionary"
            activate
            delay 0.3
            tell application "System Events"
                keystroke "," using command down
            end tell
        end tell
        """
        var error: NSDictionary?
        if let appleScript = NSAppleScript(source: script) {
            appleScript.executeAndReturnError(&error)
        }
    }
}
