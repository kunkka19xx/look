#!/usr/bin/env swift

import Foundation
import CoreServices

func fetchDefinition(word: String) -> String? {
    let range = CFRangeMake(0, word.count)
    guard let def = DCSCopyTextDefinition(nil, word as CFString, range) else { return nil }
    return def.takeRetainedValue() as String
}

// Test Lac Viet dictionary format
let testWords = ["good", "bad", "beautiful", "run", "eat", "nói", "chạy", "tốt"]

for word in testWords {
    print("=== \(word) ===")
    if let raw = fetchDefinition(word: word) {
        // Check if it's Lac Viet format (contains | pronunciation |)
        let parts = raw.components(separatedBy: " | ")
        if parts.count >= 3 && parts[0] == word {
            print("Lac Viet format detected!")
            print("First 200 chars: \(String(raw.prefix(200)))")
        } else if raw.contains(" | ") {
            print("Apple Dictionary format")
            print("First 100 chars: \(String(raw.prefix(100)))")
        } else {
            print("Unknown format")
        }
    } else {
        print("No definition")
    }
    print()
}
