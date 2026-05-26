import Foundation

// MARK: - Dictionary Lookup Data Models

struct LookupDefinition {
    let query: String
    let sourceLabel: String
    let sections: [LookupTranslationSection]
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
