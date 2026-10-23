import Foundation

// MARK: - Dictionary Lookup Data Models

struct LookupDefinition {
    let query: String
    let sourceLabel: String
    /// Replaced one entry at a time as each target answers, so a fast section
    /// reaches the panel while a slow one is still in flight.
    var sections: [LookupTranslationSection]
    /// Target language code for the "Open in Browser" link: the first configured
    /// `translate_languages` entry. nil outside web translation, where the link
    /// is not shown, and the web page then picks its own target.
    var browserTargetLanguage: String?
}

struct LookupTranslationSection {
    let label: String
    let translated: String?
    let dictionaryDefinition: LookupPresentation?
    let failed: Bool
}

struct LookupPresentation {
    let title: String
    let partOfSpeech: String?
    let definitions: [LookupDefinitionEntry]
}

struct LookupDefinitionEntry {
    let partOfSpeech: String?
    let senses: [LookupSenseEntry]
}

struct LookupSenseEntry {
    let number: Int
    let definition: String
    let examples: [String]
    let synonyms: [String]
    let antonyms: [String]
}
