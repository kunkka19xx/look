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

    private func lookupSections(
        for targets: [TranslateLanguage],
        results: [String: TranslationResult]
    ) -> [LookupTranslationSection] {
        targets.map { target in
            LookupTranslationSection(
                label: target.label,
                translated: results[target.code]?.translated,
                dictionaryDefinition: results[target.code]?.dictionaryDefinition,
                failed: results[target.code]?.translated == nil
            )
        }
    }

    func handleLookupTranslation(text: String) {
        let normalized = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalized.isEmpty else {
            showBanner("Type text after tw\" to translate", style: .error, duration: 3.2)
            return
        }

        let targets = translateLanguages
        Task {
            let results = await fetchAllTranslations(for: normalized, targets: targets)
            await MainActor.run {
                lookupDefinition = LookupDefinition(
                    query: normalized,
                    sourceLabel: "Input",
                    sections: lookupSections(for: targets, results: results)
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
        let targets = translateLanguages
        lookupPreviewTask = Task {
            // The one guard that stops a request: a translate call already in
            // flight cannot be cancelled, so anything past this point is spent.
            try? await Task.sleep(for: AppConstants.Launcher.Translate.previewDebounce)
            guard !Task.isCancelled else { return }

            let results = await fetchAllTranslations(for: normalizedText, targets: targets)

            guard !Task.isCancelled else { return }
            await MainActor.run {
                let latestQuery = query.trimmingCharacters(in: .whitespacesAndNewlines)
                guard latestQuery == expectedQuery else { return }
                lookupDefinition = LookupDefinition(
                    query: normalizedText,
                    sourceLabel: "Input",
                    sections: lookupSections(for: targets, results: results)
                )
            }
        }
    }

    func fetchAllTranslations(
        for text: String,
        targets: [TranslateLanguage]
    ) async -> [String: TranslationResult] {
        await withTaskGroup(of: (String, TranslationResult).self) { group in
            for code in targets.map(\.code) {
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

        let targets = translateLanguages
        lookupDefinition = LookupDefinition(
            query: normalized,
            sourceLabel: "Web",
            sections: targets.map { target in
                LookupTranslationSection(
                    label: target.label,
                    translated: nil,
                    dictionaryDefinition: nil,
                    failed: false
                )
            },
            browserTargetLanguage: targets.first?.code
        )

        Task {
            var results: [String: NetworkTranslationResult] = [:]
            for await (code, result) in networkTranslations(for: normalized, targets: targets) {
                results[code] = result
                await MainActor.run {
                    landNetworkSection(code, result, query: normalized, targets: targets)
                }
            }
            await MainActor.run {
                // Every target has answered by here, so a banner is the only
                // thing left to decide and the panel already shows the rest.
                guard lookupDefinition?.query == normalized else { return }
                guard !targets.contains(where: { results[$0.code]?.translated != nil }) else {
                    return
                }
                let firstError = targets.compactMap { results[$0.code]?.errorMessage }.first
                showBanner(firstError ?? "Translation failed", style: .error, duration: 3.2)
            }
        }
    }

    /// Replaces the one section `code` belongs to, leaving the others as they
    /// are. Does nothing once the panel has moved on to another query, so a
    /// slow answer cannot write into the definition that replaced it.
    @MainActor
    private func landNetworkSection(
        _ code: String,
        _ result: NetworkTranslationResult,
        query normalized: String,
        targets: [TranslateLanguage]
    ) {
        guard lookupDefinition?.query == normalized,
            let index = targets.firstIndex(where: { $0.code == code }),
            index < lookupDefinition?.sections.count ?? 0
        else {
            return
        }
        lookupDefinition?.sections[index] = LookupTranslationSection(
            label: targets[index].label,
            translated: result.translated,
            dictionaryDefinition: nil,
            failed: result.translated == nil
        )
    }

    /// One request per target, yielded as each answers rather than collected at
    /// the speed of the slowest. Google replies in about half a second while a
    /// target that falls through to MyMemory takes three times that, so waiting
    /// for the set made every translation as slow as its worst language.
    func networkTranslations(
        for text: String,
        targets: [TranslateLanguage]
    ) -> AsyncStream<(String, NetworkTranslationResult)> {
        AsyncStream { continuation in
            let task = Task {
                await withTaskGroup(of: (String, NetworkTranslationResult).self) { group in
                    for code in targets.map(\.code) {
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
                    for await pair in group {
                        continuation.yield(pair)
                    }
                }
                continuation.finish()
            }
            continuation.onTermination = { _ in task.cancel() }
        }
    }
}
