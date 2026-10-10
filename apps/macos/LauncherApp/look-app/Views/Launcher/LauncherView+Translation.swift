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

    /// The target language codes from `translate_languages`, in order and
    /// deduplicated. A code that is empty, duplicated, or absent from
    /// `languageLabels` is a typo or an unsupported language and is dropped from
    /// both the label list and the result sections. An absent, empty, or
    /// all-invalid list falls back to the built-in list, so a typo never blanks
    /// the panel.
    func configuredTranslateLanguages() -> [String] {
        let fallback = AppConstants.Launcher.Translate.defaultLanguages
        let labels = AppConstants.Launcher.Translate.languageLabels
        let path = URL(fileURLWithPath: ConfigPathResolver.resolvedPath())
        guard let raw = try? String(contentsOf: path, encoding: .utf8) else {
            return fallback
        }
        let value = ConfigFileLines.keyValues(raw)[AppConstants.Launcher.Translate.languagesConfigKey] ?? ""
        var seen = Set<String>()
        var codes: [String] = []
        for entry in ConfigFileLines.parseList(value) {
            let code = entry.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            guard !code.isEmpty, !seen.contains(code), labels[code] != nil else { continue }
            seen.insert(code)
            codes.append(code)
        }
        return codes.isEmpty ? fallback : codes
    }

    /// The built-in label for a supported code, or the uppercased code as a
    /// last resort (callers only pass codes from `configuredTranslateLanguages`).
    func translateLanguageLabel(for code: String) -> String {
        AppConstants.Launcher.Translate.languageLabels[code] ?? code.uppercased()
    }

    /// Builds the translation sections for the given language codes and lookup results.
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

    /// Handles a translation lookup command trigger for the given input text.
    func handleLookupTranslation(text: String) {
        let normalized = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else {
            showBanner("Type text after tw\" to translate", style: .error, duration: 3.2)
            return
        }

        Task {
            let codes = configuredTranslateLanguages()
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

    /// Debounces and previews dictionary and translation lookup definitions as the query changes.
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

            let codes = configuredTranslateLanguages()
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

    /// Concurrently fetches translations for all specified language codes using the engine bridge.
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

    /// Performs an on-demand network translation lookup across all configured languages.
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
            }
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
                    }
                )

                if !hasAnyResult {
                    let firstError = codes.compactMap { results[$0]?.errorMessage }.first
                    showBanner(firstError ?? "Translation failed", style: .error, duration: 3.2)
                }
            }
        }
    }

    /// Concurrently fetches network translation results for each specified language code.
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
