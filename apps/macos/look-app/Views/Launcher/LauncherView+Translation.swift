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

    /// The raw `translate_languages` value from the config file, or empty.
    private func rawTranslateLanguages() -> String {
        let path = URL(fileURLWithPath: ConfigPathResolver.resolvedPath())
        let raw = (try? String(contentsOf: path, encoding: .utf8)) ?? ""
        return ConfigFileLines.keyValues(raw)[AppConstants.Launcher.Translate.languagesConfigKey] ?? ""
    }

    /// The target language codes from `translate_languages`, in order. Parsing,
    /// validation, and the fallback list live in `look_answers` (reached through
    /// the bridge), so every shell reads the same table. The built-in list below
    /// only covers the bridge call itself failing, so the panel is never blank.
    func configuredTranslateLanguages() -> [String] {
        let codes = bridge.translateLanguages(rawConfig: rawTranslateLanguages()).map(\.code)
        return codes.isEmpty ? ["vi", "en", "ja"] : codes
    }

    /// The built-in label for a supported code, or the uppercased code as a
    /// last resort (callers only pass codes from `configuredTranslateLanguages`).
    func translateLanguageLabel(for code: String) -> String {
        bridge.translateLanguages(rawConfig: code).first { $0.code == code }?.label
            ?? code.uppercased()
    }

    private func lookupSections(
        for codes: [String],
        results: [String: TranslationResult]
    ) -> [LookupTranslationSection] {
        codes.map { code in
            LookupTranslationSection(
                label: translateLanguageLabel(for: code),
                translated: results[code]?.translated,
                dictionaryDefinition: results[code]?.dictionaryDefinition,
                failed: results[code]?.translated == nil
            )
        }
    }

    func handleLookupTranslation(text: String) {
        let normalized = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else {
            showBanner("Type text after tw\" to translate", style: .error, duration: 3.2)
            return
        }

        let codes = configuredTranslateLanguages()
        Task {
            let results = await fetchAllTranslations(for: normalized, codes: codes)
            await MainActor.run {
                lookupDefinition = LookupDefinition(
                    query: normalized,
                    sourceLabel: "Input",
                    sections: lookupSections(for: codes, results: results)
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
        let codes = configuredTranslateLanguages()
        lookupPreviewTask = Task {
            try? await Task.sleep(nanoseconds: 220_000_000)
            guard !Task.isCancelled else { return }

            let results = await fetchAllTranslations(for: normalizedText, codes: codes)

            guard !Task.isCancelled else { return }
            await MainActor.run {
                let latestQuery = query.trimmingCharacters(in: .whitespacesAndNewlines)
                guard latestQuery == expectedQuery else { return }
                lookupDefinition = LookupDefinition(
                    query: normalizedText,
                    sourceLabel: "Input",
                    sections: lookupSections(for: codes, results: results)
                )
            }
        }
    }

    func fetchAllTranslations(
        for text: String,
        codes: [String]
    ) async -> [String: TranslationResult] {
        await withTaskGroup(of: (String, TranslationResult).self) { group in
            for code in codes {
                group.addTask {
                    let translated = self.bridge.translate(text: text, targetLang: code)?.translated
                    let definition = await MainActor.run {
                        translated.flatMap { DictionaryParser.parse(self.fetchRawDefinition(for: $0) ?? "") }
                    }
                    return (code, TranslationResult(translated: translated, dictionaryDefinition: definition))
                }
            }

            var results: [String: TranslationResult] = [:]
            for await (code, result) in group {
                results[code] = result
            }
            return results
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

        let codes = configuredTranslateLanguages()
        lookupDefinition = LookupDefinition(
            query: normalized,
            sourceLabel: "Web",
            sections: codes.map { code in
                LookupTranslationSection(
                    label: translateLanguageLabel(for: code),
                    translated: nil,
                    dictionaryDefinition: nil,
                    failed: false
                )
            },
            browserTargetLanguage: codes.first ?? "en"
        )

        Task {
            let results = await fetchNetworkTranslations(for: normalized, codes: codes)
            await MainActor.run {
                let hasAnyResult = codes.contains { results[$0]?.translated != nil }

                lookupDefinition = LookupDefinition(
                    query: normalized,
                    sourceLabel: "Web",
                    sections: codes.map { code in
                        LookupTranslationSection(
                            label: translateLanguageLabel(for: code),
                            translated: results[code]?.translated,
                            dictionaryDefinition: nil,
                            failed: results[code]?.translated == nil
                        )
                    },
                    browserTargetLanguage: codes.first ?? "en"
                )

                if !hasAnyResult {
                    let firstError = codes.compactMap { results[$0]?.errorMessage }.first
                    showBanner(firstError ?? "Translation failed", style: .error, duration: 3.2)
                }
            }
        }
    }

    func fetchNetworkTranslations(
        for text: String,
        codes: [String]
    ) async -> [String: NetworkTranslationResult] {
        await withTaskGroup(of: (String, NetworkTranslationResult).self) { group in
            for code in codes {
                group.addTask {
                    let result = self.bridge.translate(text: text, targetLang: code)
                    let translated = result?.translated.trimmingCharacters(in: .whitespacesAndNewlines)
                    return (
                        code,
                        NetworkTranslationResult(
                            translated: (translated?.isEmpty == false) ? translated : nil,
                            errorMessage: result?.error?.userFacingMessage
                        )
                    )
                }
            }

            var results: [String: NetworkTranslationResult] = [:]
            for await (code, result) in group {
                results[code] = result
            }
            return results
        }
    }
}
