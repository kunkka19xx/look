import CoreServices
import Foundation

enum LauncherTranslationCommand {
    case network(String)
    case lookup(String)
}

final class LauncherTranslationService: Sendable {
    private struct LookupTranslationResult {
        let translated: String?
        let dictionaryDefinition: LookupPresentation?
    }

    private struct NetworkTranslationResult {
        let translated: String?
        let errorMessage: String?
    }

    private let bridge: EngineBridge

    init(bridge: EngineBridge = .shared) {
        self.bridge = bridge
    }

    func extractCommand(from input: String) -> LauncherTranslationCommand? {
        if input.hasPrefix("t\"") {
            let text = String(input.dropFirst(2)).trimmingCharacters(in: .whitespacesAndNewlines)
            return text.isEmpty ? nil : .network(text)
        }

        if input.count >= 3, input.prefix(3).lowercased() == "tw\"" {
            let text = String(input.dropFirst(3)).trimmingCharacters(in: .whitespacesAndNewlines)
            return text.isEmpty ? nil : .lookup(text)
        }

        if input.lowercased().hasPrefix("tr ") {
            let text = String(input.dropFirst(3)).trimmingCharacters(in: .whitespacesAndNewlines)
            return text.isEmpty ? nil : .network(text)
        }

        return nil
    }

    func fetchLookupDefinition(for text: String) async -> LookupDefinition {
        let normalized = text.trimmingCharacters(in: .whitespacesAndNewlines)
        let results = await fetchLookupTranslations(for: normalized)
        return LookupDefinition(
            query: normalized,
            sourceLabel: "Input",
            sections: [
                LookupTranslationSection(label: "English", translated: results.en.translated, dictionaryDefinition: results.en.dictionaryDefinition, failed: results.en.translated == nil),
                LookupTranslationSection(label: "Tiếng Việt", translated: results.vi.translated, dictionaryDefinition: results.vi.dictionaryDefinition, failed: results.vi.translated == nil),
                LookupTranslationSection(label: "日本語", translated: results.ja.translated, dictionaryDefinition: results.ja.dictionaryDefinition, failed: results.ja.translated == nil),
            ]
        )
    }

    func fetchNetworkDefinition(for text: String) async -> (definition: LookupDefinition, hasAnyResult: Bool, errorMessage: String?) {
        let normalized = text.trimmingCharacters(in: .whitespacesAndNewlines)
        let results = await fetchNetworkTranslations(for: normalized)
        let hasAnyResult = results.en.translated != nil
            || results.vi.translated != nil
            || results.ja.translated != nil
        let errorMessage = results.en.errorMessage
            ?? results.vi.errorMessage
            ?? results.ja.errorMessage

        let definition = LookupDefinition(
            query: normalized,
            sourceLabel: "Web",
            sections: [
                LookupTranslationSection(label: "Tiếng Việt", translated: results.vi.translated, dictionaryDefinition: nil, failed: results.vi.translated == nil),
                LookupTranslationSection(label: "English", translated: results.en.translated, dictionaryDefinition: nil, failed: results.en.translated == nil),
                LookupTranslationSection(label: "日本語", translated: results.ja.translated, dictionaryDefinition: nil, failed: results.ja.translated == nil),
            ]
        )
        return (definition, hasAnyResult, errorMessage)
    }

    private func fetchLookupTranslations(for text: String) async -> (en: LookupTranslationResult, vi: LookupTranslationResult, ja: LookupTranslationResult) {
        await withTaskGroup(of: (String, LookupTranslationResult).self) { group in
            group.addTask { @MainActor in
                let translated = self.bridge.translate(text: text, targetLang: "en")?.translated
                let definition = translated.flatMap { DictionaryParser.parse(self.fetchRawDefinition(for: $0) ?? "") }
                return ("en", LookupTranslationResult(translated: translated, dictionaryDefinition: definition))
            }
            group.addTask { @MainActor in
                let translated = self.bridge.translate(text: text, targetLang: "vi")?.translated
                let definition = translated.flatMap { DictionaryParser.parse(self.fetchRawDefinition(for: $0) ?? "") }
                return ("vi", LookupTranslationResult(translated: translated, dictionaryDefinition: definition))
            }
            group.addTask { @MainActor in
                let translated = self.bridge.translate(text: text, targetLang: "ja")?.translated
                let definition = translated.flatMap { DictionaryParser.parse(self.fetchRawDefinition(for: $0) ?? "") }
                return ("ja", LookupTranslationResult(translated: translated, dictionaryDefinition: definition))
            }

            var en = LookupTranslationResult(translated: nil, dictionaryDefinition: nil)
            var vi = LookupTranslationResult(translated: nil, dictionaryDefinition: nil)
            var ja = LookupTranslationResult(translated: nil, dictionaryDefinition: nil)
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

    private func fetchNetworkTranslations(for text: String) async -> (en: NetworkTranslationResult, vi: NetworkTranslationResult, ja: NetworkTranslationResult) {
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

    private func fetchRawDefinition(for text: String) -> String? {
        let nsText = text as NSString
        let range = CFRange(location: 0, length: nsText.length)
        guard let unmanaged = DCSCopyTextDefinition(nil, text as CFString, range) else {
            return nil
        }
        let raw = (unmanaged.takeRetainedValue() as String)
            .trimmingCharacters(in: .whitespacesAndNewlines)
        return raw.isEmpty ? nil : raw
    }
}
