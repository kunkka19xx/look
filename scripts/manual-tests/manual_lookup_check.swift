#!/usr/bin/env swift

import Foundation
import CoreServices

func fetchDefinition(word: String) -> String? {
    let range = CFRangeMake(0, word.count)
    guard let def = DCSCopyTextDefinition(nil, word as CFString, range) else { return nil }
    return def.takeRetainedValue() as String
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

// MARK: - Helper Functions

func containsJapanese(_ text: String) -> Bool {
    let japaneseRange = text.range(of: "[\\u3040-\\u309F\\u30A0-\\u30FF\\u4E00-\\u9FAF]", options: .regularExpression)
    return japaneseRange != nil
}

func truncateText(_ text: String, maxLength: Int) -> String {
    if text.count <= maxLength { return text }
    return String(text.prefix(maxLength)) + "…"
}

func cleanJapaneseDefinition(_ text: String) -> String {
    var result = text
    while let bracketStart = result.range(of: "【"),
          let bracketEnd = result.range(of: "】", range: bracketStart.upperBound..<result.endIndex) {
        result = result.replacingCharacters(in: bracketStart.lowerBound..<bracketEnd.upperBound, with: "")
    }
    if let refMarker = result.range(of: "（⇨", options: .backwards),
       refMarker.lowerBound > result.startIndex {
        result = String(result[..<refMarker.lowerBound])
    }
    return result.trimmingCharacters(in: .whitespacesAndNewlines)
}

func extractWordChoiceDef(_ text: String) -> String? {
    var result = text
    if let bulletPos = result.range(of: "▸") {
        result = String(result[..<bulletPos.lowerBound])
    }
    result = result.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !result.isEmpty else { return nil }
    return result.count > 150 ? truncateText(result, maxLength: 150) : result
}

func findPOSSections(raw: String, keywords: [String]) -> [(pos: String, startIndex: String.Index)] {
    var sections: [(pos: String, startIndex: String.Index)] = []
    for keyword in keywords {
        var searchStart = raw.startIndex
        while let range = raw.range(of: keyword.lowercased(), range: searchStart..<raw.endIndex) {
            let afterKeyword = range.upperBound
            if afterKeyword < raw.endIndex {
                let nextChar = raw[afterKeyword]
                if nextChar.isWhitespace || nextChar.isNumber {
                    sections.append((keyword, range.lowerBound))
                }
            }
            searchStart = range.upperBound
        }
    }
    return sections.sorted { $0.startIndex < $1.startIndex }
}

func extractSensesWithAntonyms(_ text: String, maxSenses: Int) -> [LookupSenseEntry] {
    var senses: [LookupSenseEntry] = []
    let segments = text.components(separatedBy: "ANTONYMS")

    for (index, segment) in segments.enumerated() {
        let trimmed = segment.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { continue }

        var antonym = ""
        var rest = trimmed

        if index > 0, let first = trimmed.components(separatedBy: " ").first, !first.isEmpty {
            antonym = first.trimmingCharacters(in: CharacterSet(charactersIn: ".,;: "))
            if let dotRange = trimmed.range(of: ". ") {
                rest = String(trimmed[dotRange.upperBound...]).trimmingCharacters(in: .whitespacesAndNewlines)
            }
        }

        let sentences = rest.components(separatedBy: ". ")
        for sentence in sentences {
            let s = sentence.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !s.isEmpty, let num = extractNumberPrefix(s) else { continue }

            let def = String(s.dropFirst(String(num).count + 1)).trimmingCharacters(in: .whitespacesAndNewlines)
            guard def.count > 5 && senses.count < maxSenses else { continue }

            senses.append(LookupSenseEntry(
                number: num,
                definition: truncate(def, maxLength: 80),
                examples: [],
                synonyms: [],
                antonyms: (index > 0 && !antonym.isEmpty) ? [antonym] : []
            ))
            if index > 0 { antonym = "" }
        }

        if senses.count >= maxSenses { break }
    }

    return senses
}

// MARK: - Main Parser

func parse(_ raw: String) -> LookupPresentation? {
    let text = raw.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !text.isEmpty else { return nil }

    let parts = text.components(separatedBy: " | ")
    let isVietnamese = parts.count >= 3 && parts[0] == parts[1]

    if isVietnamese {
        return parseVietnamese(text)
    }

    if containsJapanese(text) {
        return parseJapanese(text)
    }

    return parseEnglish(text)
}

// MARK: - Vietnamese Parser

func parseVietnamese(_ raw: String) -> LookupPresentation? {
    let bulletParts = raw.components(separatedBy: "▸")
    guard bulletParts.count >= 1 else { return nil }

    let defParts = bulletParts[0].components(separatedBy: " | ")
    guard defParts.count >= 3 else { return nil }

    let title = defParts[0].trimmingCharacters(in: .whitespacesAndNewlines)
    let allPosKeywords = ["thán từ", "danh từ", "động từ", "tính từ", "trạng từ", "giới từ", "liên từ", "đại từ",
                          "interjection", "noun", "verb", "adjective", "adverb", "preposition", "conjunction", "pronoun"]

    let posSections = findPOSSections(raw: raw, keywords: allPosKeywords)
    let definitions = posSections.isEmpty
        ? parseVietnameseNoPOS(raw: raw, allPosKeywords: allPosKeywords)
        : parseVietnameseWithPOS(raw: raw, posSections: posSections, bulletParts: bulletParts, allPosKeywords: allPosKeywords)

    guard !definitions.isEmpty else { return nil }
    return LookupPresentation(title: title, partOfSpeech: definitions.first?.partOfSpeech, definitions: definitions)
}

func parseVietnameseWithPOS(raw: String, posSections: [(pos: String, startIndex: String.Index)], bulletParts: [String], allPosKeywords: [String]) -> [LookupDefinitionEntry] {
    var definitions: [LookupDefinitionEntry] = []
    let senseNumbers = Array(1...50)

    for (sectionIdx, posInfo) in posSections.enumerated() {
        let sectionEnd = sectionIdx + 1 < posSections.count ? posSections[sectionIdx + 1].startIndex : raw.endIndex
        let sectionText = String(raw[posInfo.startIndex..<sectionEnd])

        var senses = parseVietnameseSenses(sectionText, maxSenses: 10, posKeywords: allPosKeywords, senseNumbers: senseNumbers)
        if senses.isEmpty {
            senses = parseVietnameseFallbackSense(sectionText, posKeywords: allPosKeywords)
        }
        senses = distributeExamplesToSenses(senses: senses, bulletParts: bulletParts, sectionStart: posInfo.startIndex, sectionEnd: sectionEnd, raw: raw)

        if !senses.isEmpty {
            definitions.append(LookupDefinitionEntry(partOfSpeech: posInfo.pos, senses: senses))
        }
    }
    return definitions
}

func parseVietnameseSenses(_ text: String, maxSenses: Int, posKeywords: [String], senseNumbers: [Int]) -> [LookupSenseEntry] {
    var senses: [LookupSenseEntry] = []

    for num in senseNumbers {
        guard let range = text.range(of: "\(num) ") else { continue }

        let afterNum = range.upperBound
        var defEnd = text.endIndex

        if let bulletRange = text.range(of: "▸", range: afterNum..<text.endIndex) {
            defEnd = bulletRange.lowerBound
        }

        for nextNum in senseNumbers where nextNum > num {
            if let nextRange = text.range(of: "\(nextNum) "), nextRange.lowerBound < defEnd {
                defEnd = nextRange.lowerBound
            }
        }

        guard afterNum < defEnd else { continue }
        var def = String(text[afterNum..<defEnd]).trimmingCharacters(in: .whitespacesAndNewlines)

        for nextNum in (num + 1)...50 {
            if let cutRange = def.range(of: " \(nextNum) ") {
                def = String(def[..<cutRange.lowerBound]).trimmingCharacters(in: .whitespacesAndNewlines)
                break
            }
        }

        for keyword in posKeywords {
            if let cutRange = def.range(of: " \(keyword)".lowercased()) {
                def = String(def[..<cutRange.lowerBound]).trimmingCharacters(in: .whitespacesAndNewlines)
                break
            }
        }

        guard !def.isEmpty, senses.count < maxSenses else { continue }
        def = def.count > 150 ? truncateText(def, maxLength: 150) : def
        senses.append(LookupSenseEntry(number: num, definition: def, examples: [], synonyms: [], antonyms: []))
    }

    return senses
}

func parseVietnameseFallbackSense(_ text: String, posKeywords: [String]) -> [LookupSenseEntry] {
    guard let bulletRange = text.range(of: "▸") else { return [] }
    var def = String(text[..<bulletRange.lowerBound]).trimmingCharacters(in: .whitespacesAndNewlines)

    for keyword in posKeywords {
        if let cutRange = def.lowercased().range(of: keyword.lowercased()), cutRange.upperBound < def.endIndex {
            def = String(def[cutRange.upperBound...]).trimmingCharacters(in: .whitespacesAndNewlines)
            break
        }
    }

    guard !def.isEmpty, def.count < 100 else { return [] }
    return [LookupSenseEntry(number: 1, definition: def, examples: [], synonyms: [], antonyms: [])]
}

func parseVietnameseNoPOS(raw: String, allPosKeywords: [String]) -> [LookupDefinitionEntry] {
    let defParts2 = raw.components(separatedBy: " | ")
    guard defParts2.count >= 3 else { return [] }
    let textAfterPipe = defParts2.dropFirst(2).joined(separator: " | ")

    let senseNumbers = Array(1...50)
    var senses: [LookupSenseEntry] = []
    var searchStart = textAfterPipe.startIndex

    for num in senseNumbers {
        guard let range = textAfterPipe.range(of: "\(num) ", range: searchStart..<textAfterPipe.endIndex) else { continue }

        let afterNum = range.upperBound
        var defEnd = textAfterPipe.endIndex

        if let bulletPos = textAfterPipe.range(of: "▸", range: afterNum..<textAfterPipe.endIndex)?.lowerBound {
            defEnd = bulletPos
        }
        for nextNum in senseNumbers where nextNum > num {
            if let nextPos = textAfterPipe.range(of: "\(nextNum) ", range: afterNum..<textAfterPipe.endIndex)?.lowerBound,
               nextPos < defEnd {
                defEnd = nextPos
            }
        }

        if afterNum < defEnd {
            let def = String(textAfterPipe[afterNum..<defEnd]).trimmingCharacters(in: .whitespacesAndNewlines)
            if !def.isEmpty && def.count < 100 && senses.count < 10 {
                senses.append(LookupSenseEntry(number: num, definition: def, examples: [], synonyms: [], antonyms: []))
            }
        }

        if senses.count >= 10 { break }
        searchStart = afterNum
    }

    guard !senses.isEmpty else { return [] }

    for idx in 0..<senses.count - 1 {
        let nextSenseNum = senses[idx + 1].number
        guard let currentRange = textAfterPipe.range(of: "\(senses[idx].number) "),
              let nextRange = textAfterPipe.range(of: "\(nextSenseNum) ") else { continue }

        let segment = String(textAfterPipe[currentRange.lowerBound..<nextRange.lowerBound])
        if let lastBulletRange = segment.range(of: "▸", options: .backwards) {
            var exampleText = String(segment[lastBulletRange.upperBound...]).trimmingCharacters(in: .whitespacesAndNewlines)

            if exampleText.contains("\(nextSenseNum) "),
               let cutRange = exampleText.range(of: "\(nextSenseNum) ") {
                exampleText = String(exampleText[..<cutRange.lowerBound]).trimmingCharacters(in: .whitespacesAndNewlines)
            }

            if !exampleText.isEmpty && exampleText.count > 5 {
                senses[idx] = LookupSenseEntry(
                    number: senses[idx].number,
                    definition: senses[idx].definition,
                    examples: [exampleText],
                    synonyms: senses[idx].synonyms,
                    antonyms: senses[idx].antonyms
                )
            }
        }
    }

    return [LookupDefinitionEntry(partOfSpeech: nil, senses: senses)]
}

func distributeExamplesToSenses(senses: [LookupSenseEntry], bulletParts: [String], sectionStart: String.Index, sectionEnd: String.Index, raw: String) -> [LookupSenseEntry] {
    guard !senses.isEmpty else { return senses }
    var result = senses
    var senseIndex = 0

    for partIdx in 1..<bulletParts.count {
        guard senseIndex < result.count else { break }
        let exampleText = bulletParts[partIdx].trimmingCharacters(in: .whitespacesAndNewlines)
        guard !exampleText.isEmpty else { continue }

        guard let partRange = raw.range(of: "▸" + bulletParts[partIdx]),
              partRange.lowerBound >= sectionStart && partRange.lowerBound < sectionEnd else { continue }

        if result[senseIndex].examples.count < 2 {
            result[senseIndex] = LookupSenseEntry(
                number: result[senseIndex].number,
                definition: result[senseIndex].definition,
                examples: result[senseIndex].examples + [exampleText],
                synonyms: result[senseIndex].synonyms,
                antonyms: result[senseIndex].antonyms
            )
        } else {
            senseIndex += 1
        }
    }

    return result
}

// MARK: - Japanese Parser

func parseJapanese(_ raw: String) -> LookupPresentation? {
    let text = raw.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !text.isEmpty else { return nil }

    let title: String
    if let bracketRange = text.range(of: "【") {
        title = String(text[..<bracketRange.lowerBound]).trimmingCharacters(in: .whitespacesAndNewlines)
    } else {
        title = String(text.prefix(while: { !$0.isWhitespace }))
    }

    var definitions: [LookupDefinitionEntry] = []
    var senses: [LookupSenseEntry] = []

    let senseNumbers = Array(1...20)
    var numberedSenseRanges: [(num: Int, range: Range<String.Index>)] = []
    for num in senseNumbers {
        let pattern = "\(num) 【"
        if let range = text.range(of: pattern) {
            numberedSenseRanges.append((num, range))
        }
    }

    guard let wordChoiceRange = text.range(of: "WORD CHOICE :") else {
        guard !numberedSenseRanges.isEmpty else {
            let simpleDef = extractWordChoiceDef(text)
            if let def = simpleDef {
                definitions.append(LookupDefinitionEntry(
                    partOfSpeech: "word",
                    senses: [LookupSenseEntry(number: 1, definition: def, examples: [], synonyms: [], antonyms: [])]
                ))
            }
            return LookupPresentation(title: title, partOfSpeech: "word", definitions: definitions)
        }
        return parseJapaneseSenses(text, title: title, senses: &senses, definitions: &definitions)
    }

    let wcPos = wordChoiceRange.lowerBound
    let firstSensePos = numberedSenseRanges.first?.range.lowerBound

    if let firstPos = firstSensePos, firstPos < wcPos {
        guard let wcDef = extractWordChoiceDef(String(text[wordChoiceRange.upperBound...])) else {
            return parseJapaneseSenses(text, title: title, senses: &senses, definitions: &definitions)
        }
        definitions.append(LookupDefinitionEntry(
            partOfSpeech: "WORD CHOICE",
            senses: [LookupSenseEntry(number: 1, definition: wcDef, examples: [], synonyms: [], antonyms: [])]
        ))
        let sensesText = String(text[..<wcPos])
        return parseJapaneseSenses(sensesText, title: title, senses: &senses, definitions: &definitions)
    }

    let afterWC = String(text[wordChoiceRange.upperBound...])
    if afterWC.range(of: "1 【") != nil {
        guard let firstSenseRange = afterWC.range(of: "1 【"),
              let wcDef = extractWordChoiceDef(String(afterWC[..<firstSenseRange.lowerBound])) else {
            return nil
        }
        definitions.append(LookupDefinitionEntry(
            partOfSpeech: "WORD CHOICE",
            senses: [LookupSenseEntry(number: 1, definition: wcDef, examples: [], synonyms: [], antonyms: [])]
        ))
        let sensesText = String(afterWC[firstSenseRange.lowerBound...])
        return parseJapaneseSenses(sensesText, title: title, senses: &senses, definitions: &definitions)
    }

    if let wcDef = extractWordChoiceDef(afterWC) {
        definitions.append(LookupDefinitionEntry(
            partOfSpeech: "WORD CHOICE",
            senses: [LookupSenseEntry(number: 1, definition: wcDef, examples: [], synonyms: [], antonyms: [])]
        ))
    }
    return LookupPresentation(title: title, partOfSpeech: "WORD CHOICE", definitions: definitions)
}

func parseJapaneseSenses(_ text: String, title: String, senses: inout [LookupSenseEntry], definitions: inout [LookupDefinitionEntry]) -> LookupPresentation {
    let senseNumbers = Array(1...20)
    var searchStart = text.startIndex

    for num in senseNumbers {
        let pattern = "\(num) 【"
        guard let range = text.range(of: pattern, range: searchStart..<text.endIndex) else { continue }

        let afterNum = range.upperBound
        var defEnd = text.endIndex

        for nextNum in senseNumbers where nextNum > num {
            let nextPattern = "\(nextNum) 【"
            if let nextRange = text.range(of: nextPattern, range: afterNum..<text.endIndex) {
                defEnd = nextRange.lowerBound
                break
            }
        }

        var defText: String
        if let firstCloseBracket = text.range(of: "】", range: afterNum..<defEnd) {
            defText = String(text[firstCloseBracket.upperBound..<defEnd])
        } else {
            defText = String(text[afterNum..<defEnd])
        }

        defText = cleanJapaneseDefinition(defText)
        defText = truncateText(defText, maxLength: 80)

        if !defText.isEmpty && senses.count < 10 {
            senses.append(LookupSenseEntry(number: num, definition: defText, examples: [], synonyms: [], antonyms: []))
        }

        searchStart = range.upperBound
        if senses.count >= 10 { break }
    }

    if !senses.isEmpty {
        definitions.append(LookupDefinitionEntry(partOfSpeech: "word", senses: senses))
    }

    let mainPos = definitions.first?.partOfSpeech ?? "word"
    return LookupPresentation(title: title, partOfSpeech: mainPos, definitions: definitions)
}

// MARK: - English Parser

func parseEnglish(_ raw: String) -> LookupPresentation? {
    let text = raw.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !text.isEmpty else { return nil }

    let title = String(text.prefix(while: { !$0.isWhitespace }))
    var definitions: [LookupDefinitionEntry] = []

    let posKeywordsOrdered = ["adjective", "noun", "verb", "adverb", "exclamation", "interjection"]

    for pos in posKeywordsOrdered {
        let lowercased = text.lowercased()
        guard let range = lowercased.range(of: " \(pos) ") else { continue }

        let afterPos = String(text[range.upperBound...])
        var endIndex = afterPos.count

        for nextPos in posKeywordsOrdered {
            guard nextPos != pos else { continue }
            if let nextRange = afterPos.lowercased().range(of: " \(nextPos) ") {
                endIndex = afterPos.distance(from: afterPos.startIndex, to: nextRange.lowerBound)
                break
            }
        }

        let sectionText = String(afterPos.prefix(endIndex)).trimmingCharacters(in: .whitespacesAndNewlines)

        if pos == "exclamation" {
            if let phrasesRange = sectionText.lowercased().range(of: " phrases") {
                let defText = String(sectionText[..<phrasesRange.lowerBound]).trimmingCharacters(in: .whitespacesAndNewlines)
                if !defText.isEmpty && defText.count < 300 {
                    definitions.append(LookupDefinitionEntry(
                        partOfSpeech: pos,
                        senses: [LookupSenseEntry(number: 1, definition: defText, examples: [], synonyms: [], antonyms: [])]
                    ))
                }
            }
        } else {
            let senses = extractSensesWithAntonyms(sectionText, maxSenses: 10)
            if !senses.isEmpty {
                definitions.append(LookupDefinitionEntry(partOfSpeech: pos, senses: senses))
            }
        }
    }

    if let phrasesRange = text.lowercased().range(of: " phrases") {
        let afterPhrases = String(text[phrasesRange.upperBound...])
        let phrases = extractPhrases(afterPhrases, maxItems: 3)
        if !phrases.isEmpty {
            definitions.append(LookupDefinitionEntry(partOfSpeech: "phrases", senses: phrases))
        }
    }

    if definitions.isEmpty {
        let senses = extractSensesWithAntonyms(text, maxSenses: 3)
        if !senses.isEmpty {
            definitions.append(LookupDefinitionEntry(partOfSpeech: nil, senses: senses))
        }
    }

    let partOfSpeech = definitions.first?.partOfSpeech
    return LookupPresentation(title: title, partOfSpeech: partOfSpeech, definitions: definitions)
}

func extractPhrases(_ text: String, maxItems: Int) -> [LookupSenseEntry] {
    var phrases: [LookupSenseEntry] = []
    let segments = text.components(separatedBy: "ANTONYMS")

    for segment in segments.prefix(1) {
        let parts = segment.components(separatedBy: ". ")

        for part in parts {
            let trimmed = part.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty && trimmed.count >= 5 && !trimmed.lowercased().contains("antonym") else { continue }

            phrases.append(LookupSenseEntry(
                number: phrases.count + 1,
                definition: truncate(trimmed, maxLength: 80),
                examples: [],
                synonyms: [],
                antonyms: []
            ))

            if phrases.count >= maxItems { break }
        }
    }

    return phrases
}

func extractNumberPrefix(_ text: String) -> Int? {
    let chars = text.prefix(while: { $0.isNumber })
    guard let num = Int(chars), num > 0, num < 1000 else { return nil }

    let afterIdx = text.index(text.startIndex, offsetBy: chars.count)
    guard afterIdx < text.endIndex else { return nil }

    let nextChar = text[afterIdx]
    return (nextChar == "." || nextChar == " ") ? num : nil
}

func truncate(_ text: String, maxLength: Int) -> String {
    if text.count <= maxLength { return text }
    if let range = text.range(of: "; ", range: text.index(text.startIndex, offsetBy: 30)..<text.endIndex) {
        return String(text[..<range.lowerBound]).trimmingCharacters(in: .whitespacesAndNewlines)
    }
    return String(text.prefix(maxLength)) + "…"
}

// MARK: - Test

print("=== Testing 'good' (English) ===\n")

if let raw = fetchDefinition(word: "good") {
    if let result = parse(raw) {
        print("Title: \(result.title)")
        print("POS: \(result.partOfSpeech ?? "nil")")
        print()

        for entry in result.definitions {
            print("=== \(entry.partOfSpeech) ===")
            for sense in entry.senses {
                print("\(sense.number). \(sense.definition)")
                if !sense.antonyms.isEmpty {
                    print("    ↳ Antonym: \(sense.antonyms.joined(separator: ", "))")
                }
            }
        }
    } else {
        print("Parse returned nil")
    }
} else {
    print("No definition found")
}

print("\n=== Testing 'tốt' (Vietnamese) ===\n")

if let raw = fetchDefinition(word: "tốt") {
    if let result = parse(raw) {
        print("Title: \(result.title)")
        print("POS: \(result.partOfSpeech ?? "nil")")
        print()

        for entry in result.definitions {
            print("=== \(entry.partOfSpeech) ===")
            for sense in entry.senses {
                print("\(sense.number). \(sense.definition)")
                if !sense.examples.isEmpty {
                    for ex in sense.examples.prefix(3) {
                        print("  ▸ \(ex)")
                    }
                }
            }
        }
    }
}

print("\n=== Testing 'chạy' (Vietnamese) ===\n")

if let raw = fetchDefinition(word: "chạy") {
    if let result = parse(raw) {
        print("Title: \(result.title)")
        print("POS: \(result.partOfSpeech ?? "nil")")
        print()

        for entry in result.definitions {
            print("=== \(entry.partOfSpeech) ===")
            for sense in entry.senses {
                print("\(sense.number). \(sense.definition)")
                if !sense.examples.isEmpty {
                    for ex in sense.examples.prefix(3) {
                        print("  ▸ \(ex)")
                    }
                    if sense.examples.count > 3 {
                        print("  ... and \(sense.examples.count - 3) more")
                    }
                }
            }
        }
    }
}

print("\n=== Testing '良い' (Japanese) ===\n")

if let raw = fetchDefinition(word: "良い") {
    if let result = parse(raw) {
        print("Title: \(result.title)")
        print("POS: \(result.partOfSpeech ?? "nil")")
        print()

        for entry in result.definitions {
            print("=== \(entry.partOfSpeech) ===")
            for sense in entry.senses.prefix(5) {
                print("\(sense.number). \(sense.definition)")
                if !sense.examples.isEmpty {
                    for ex in sense.examples.prefix(2) {
                        print("  ▸ \(ex)")
                    }
                }
            }
            if entry.senses.count > 5 {
                print("... and \(entry.senses.count - 5) more senses")
            }
        }
    }
}

print("\n=== Testing '走る' (Japanese) ===\n")

if let raw = fetchDefinition(word: "走る") {
    if let result = parse(raw) {
        print("Title: \(result.title)")
        print("POS: \(result.partOfSpeech ?? "nil")")
        print()

        for entry in result.definitions {
            print("=== \(entry.partOfSpeech) ===")
            for sense in entry.senses.prefix(5) {
                print("\(sense.number). \(sense.definition)")
                if !sense.examples.isEmpty {
                    for ex in sense.examples.prefix(2) {
                        print("  ▸ \(ex)")
                    }
                }
            }
            if entry.senses.count > 5 {
                print("... and \(entry.senses.count - 5) more senses")
            }
        }
    }
}
