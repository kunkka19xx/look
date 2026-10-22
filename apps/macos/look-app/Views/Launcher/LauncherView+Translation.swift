import AppKit
import CoreServices
import SwiftUI

extension LauncherView {
    struct TranslationResult {
        let translated: String?
        let dictionaryDefinition: LookupPresentation?
    }

    struct NetworkTranslationResult {
        let translated: String?
        let errorMessage: String?
    }

    func extractTranslationQuery(from input: String) -> TranslationCommand? {
        let translate = AppConstants.Launcher.QueryPrefix.translate
        let translateWord = AppConstants.Launcher.QueryPrefix.translateWord
        if input.hasPrefix(translate) {
            let text = String(input.dropFirst(translate.count)).trimmingCharacters(in: .whitespacesAndNewlines)
            return text.isEmpty ? nil : .network(text)
        }

        if input.count >= translateWord.count,
            input.prefix(translateWord.count).lowercased() == translateWord {
            let text = String(input.dropFirst(translateWord.count)).trimmingCharacters(in: .whitespacesAndNewlines)
            return text.isEmpty ? nil : .lookup(text)
        }

        if input.lowercased().hasPrefix("tr ") {
            let text = String(input.dropFirst(3)).trimmingCharacters(in: .whitespacesAndNewlines)
            return text.isEmpty ? nil : .network(text)
        }

        return nil
    }

    func handleTranslation(command: TranslationCommand) {
        switch command {
        case .network(let text):
            handleNetworkTranslation(text: text)
        case .lookup(let text):
            handleLookupTranslation(text: text)
        }
    }

    func handleLookupTranslation(text: String) {
        let normalized = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else {
            showBanner("Type text after tw\" to translate", style: .error, duration: 3.2)
            return
        }

        Task {
            let results = await fetchAllTranslations(for: normalized)
            await MainActor.run {
                lookupDefinition = LookupDefinition(
                    query: normalized,
                    sourceLabel: "Input",
                    sections: [
                        LookupTranslationSection(label: "English", translated: results.en.translated, dictionaryDefinition: results.en.dictionaryDefinition, failed: results.en.translated == nil),
                        LookupTranslationSection(label: "Tiếng Việt", translated: results.vi.translated, dictionaryDefinition: results.vi.dictionaryDefinition, failed: results.vi.translated == nil),
                        LookupTranslationSection(label: "日本語", translated: results.ja.translated, dictionaryDefinition: results.ja.dictionaryDefinition, failed: results.ja.translated == nil),
                    ]
                )
            }
        }
    }

    func previewLookupDefinition(for input: String) {
        lookupPreviewTask?.cancel()

        let trimmed = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard case .lookup(let text) = extractTranslationQuery(from: trimmed) else {
            lookupDefinition = nil
            return
        }

        let normalizedText = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalizedText.isEmpty else {
            lookupDefinition = nil
            return
        }

        let expectedQuery = trimmed
        lookupPreviewTask = Task {
            try? await Task.sleep(nanoseconds: 220_000_000)
            guard !Task.isCancelled else { return }

            let results = await fetchAllTranslations(for: normalizedText)

            guard !Task.isCancelled else { return }
            await MainActor.run {
                let latestQuery = query.trimmingCharacters(in: .whitespacesAndNewlines)
                guard latestQuery == expectedQuery else { return }
                lookupDefinition = LookupDefinition(
                    query: normalizedText,
                    sourceLabel: "Input",
                    sections: [
                        LookupTranslationSection(label: "English", translated: results.en.translated, dictionaryDefinition: results.en.dictionaryDefinition, failed: results.en.translated == nil),
                        LookupTranslationSection(label: "Tiếng Việt", translated: results.vi.translated, dictionaryDefinition: results.vi.dictionaryDefinition, failed: results.vi.translated == nil),
                        LookupTranslationSection(label: "日本語", translated: results.ja.translated, dictionaryDefinition: results.ja.dictionaryDefinition, failed: results.ja.translated == nil),
                    ]
                )
            }
        }
    }

    func fetchAllTranslations(for text: String) async -> (en: TranslationResult, vi: TranslationResult, ja: TranslationResult) {
        await withTaskGroup(of: (String, TranslationResult).self) { group in
            group.addTask {
                let translated = self.bridge.translate(text: text, targetLang: "en")?.translated
                let definition = await MainActor.run {
                    translated.flatMap { DictionaryParser.parse(self.fetchRawDefinition(for: $0) ?? "") }
                }
                return ("en", TranslationResult(translated: translated, dictionaryDefinition: definition))
            }
            group.addTask {
                let translated = self.bridge.translate(text: text, targetLang: "vi")?.translated
                let definition = await MainActor.run {
                    translated.flatMap { DictionaryParser.parse(self.fetchRawDefinition(for: $0) ?? "") }
                }
                return ("vi", TranslationResult(translated: translated, dictionaryDefinition: definition))
            }
            group.addTask {
                let translated = self.bridge.translate(text: text, targetLang: "ja")?.translated
                let definition = await MainActor.run {
                    translated.flatMap { DictionaryParser.parse(self.fetchRawDefinition(for: $0) ?? "") }
                }
                return ("ja", TranslationResult(translated: translated, dictionaryDefinition: definition))
            }

            var en = TranslationResult(translated: nil, dictionaryDefinition: nil)
            var vi = TranslationResult(translated: nil, dictionaryDefinition: nil)
            var ja = TranslationResult(translated: nil, dictionaryDefinition: nil)
            for await (lang, result) in group {
                switch lang {
                case "en": en = result
                case "vi": vi = result
                case "ja": ja = result
                default: break
                }
            }
            return (en, vi, ja)
        }
    }

    func fetchRawDefinition(for text: String) -> String? {
        let nsText = text as NSString
        let range = CFRange(location: 0, length: nsText.length)
        guard let unmanaged = DCSCopyTextDefinition(nil, text as CFString, range) else {
            return nil
        }
        let raw = (unmanaged.takeRetainedValue() as String)
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return raw.isEmpty ? nil : raw
    }

    func handleNetworkTranslation(text: String) {
        let normalized = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else {
            showBanner("Type text after t\" to translate", style: .error, duration: 3.2)
            return
        }

        lookupDefinition = LookupDefinition(
            query: normalized,
            sourceLabel: "Web",
            sections: [
                LookupTranslationSection(label: "Tiếng Việt", translated: nil, dictionaryDefinition: nil, failed: false),
                LookupTranslationSection(label: "English", translated: nil, dictionaryDefinition: nil, failed: false),
                LookupTranslationSection(label: "日本語", translated: nil, dictionaryDefinition: nil, failed: false),
            ]
        )

        Task {
            let results = await fetchNetworkTranslations(for: normalized)
            await MainActor.run {
                let hasAnyResult = results.en.translated != nil
                    || results.vi.translated != nil
                    || results.ja.translated != nil

                lookupDefinition = LookupDefinition(
                    query: normalized,
                    sourceLabel: "Web",
                    sections: [
                        LookupTranslationSection(label: "Tiếng Việt", translated: results.vi.translated, dictionaryDefinition: nil, failed: results.vi.translated == nil),
                        LookupTranslationSection(label: "English", translated: results.en.translated, dictionaryDefinition: nil, failed: results.en.translated == nil),
                        LookupTranslationSection(label: "日本語", translated: results.ja.translated, dictionaryDefinition: nil, failed: results.ja.translated == nil),
                    ]
                )

                if !hasAnyResult {
                    let message = results.en.errorMessage
                        ?? results.vi.errorMessage
                        ?? results.ja.errorMessage
                        ?? "Translation failed"
                    showBanner(message, style: .error, duration: 3.2)
                }
            }
        }
    }

    func fetchNetworkTranslations(for text: String) async -> (en: NetworkTranslationResult, vi: NetworkTranslationResult, ja: NetworkTranslationResult) {
        await withTaskGroup(of: (String, NetworkTranslationResult).self) { group in
            group.addTask {
                let result = self.bridge.translate(text: text, targetLang: "en")
                let translated = result?.translated.trimmingCharacters(in: .whitespacesAndNewlines)
                return (
                    "en",
                    NetworkTranslationResult(
                        translated: (translated?.isEmpty == false) ? translated : nil,
                        errorMessage: result?.error?.userFacingMessage
                    )
                )
            }
            group.addTask {
                let result = self.bridge.translate(text: text, targetLang: "vi")
                let translated = result?.translated.trimmingCharacters(in: .whitespacesAndNewlines)
                return (
                    "vi",
                    NetworkTranslationResult(
                        translated: (translated?.isEmpty == false) ? translated : nil,
                        errorMessage: result?.error?.userFacingMessage
                    )
                )
            }
            group.addTask {
                let result = self.bridge.translate(text: text, targetLang: "ja")
                let translated = result?.translated.trimmingCharacters(in: .whitespacesAndNewlines)
                return (
                    "ja",
                    NetworkTranslationResult(
                        translated: (translated?.isEmpty == false) ? translated : nil,
                        errorMessage: result?.error?.userFacingMessage
                    )
                )
            }

            var en = NetworkTranslationResult(translated: nil, errorMessage: nil)
            var vi = NetworkTranslationResult(translated: nil, errorMessage: nil)
            var ja = NetworkTranslationResult(translated: nil, errorMessage: nil)
            for await (lang, result) in group {
                switch lang {
                case "en": en = result
                case "vi": vi = result
                case "ja": ja = result
                default: break
                }
            }
            return (en, vi, ja)
        }
    }
}
