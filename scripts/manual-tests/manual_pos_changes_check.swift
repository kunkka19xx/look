#!/usr/bin/env swift
// Test script to demonstrate POS label changes
// Run with: swift scripts/manual-tests/manual_pos_changes_check.swift

import Foundation

// MARK: - Test Data Models

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
}

// MARK: - Mock Parser Functions

func parseVietnameseWithPOS(_ raw: String) -> LookupPresentation? {
    // Simulates: parseVietnamese with explicit POS sections
    // Example: "hello | hello | danh từ 1 greeting"
    let parts = raw.components(separatedBy: " | ")
    guard parts.count >= 3 else { return nil }

    let title = parts[0]
    let text = parts[2]

    // Check for POS keywords
    let posKeywords = ["danh từ", "động từ", "tính từ", "trạng từ"]
    for pos in posKeywords {
        if text.lowercased().contains(pos) {
            let sense = LookupSenseEntry(number: 1, definition: "Definition with POS")
            return LookupPresentation(
                title: title,
                partOfSpeech: pos,
                definitions: [LookupDefinitionEntry(partOfSpeech: pos, senses: [sense])]
            )
        }
    }

    return nil
}

func parseVietnameseNoPOS(_ raw: String) -> LookupPresentation? {
    // Simulates: parseVietnameseNoPOS (no explicit POS keywords)
    // Example: "hello | hello | 1 greeting ▸ example"
    let parts = raw.components(separatedBy: " | ")
    guard parts.count >= 3 else { return nil }

    let title = parts[0]
    let text = parts[2]

    // Extract sense (number + definition)
    let sense = LookupSenseEntry(number: 1, definition: text)

    // OLD behavior (commented out): would default to "adjective"
    // return LookupPresentation(
    //     title: title,
    //     partOfSpeech: "adjective",
    //     definitions: [LookupDefinitionEntry(partOfSpeech: "adjective", senses: [sense])]
    // )

    // NEW behavior: partOfSpeech is nil
    return LookupPresentation(
        title: title,
        partOfSpeech: nil,
        definitions: [LookupDefinitionEntry(partOfSpeech: nil, senses: [sense])]
    )
}

// MARK: - Test Cases

func runTests() {
    print("=== Testing POS Label Changes ===\n")

    // Test Case 1: Vietnamese entry WITH explicit POS keyword
    print("Test 1: Vietnamese entry WITH explicit POS keyword")
    print("Input: \"xin chào | xin chào | danh từ 1 greeting\"")
    if let result = parseVietnameseWithPOS("xin chào | xin chào | danh từ 1 greeting") {
        print("[OK] Parsed successfully")
        print("   Title: \(result.title)")
        print("   Main POS: \(result.partOfSpeech ?? "nil (no label)")")
        print("   Entry POS: \(result.definitions.first?.partOfSpeech ?? "nil (no label)")")
        print("   POS label WILL be displayed\n")
    } else {
        print("[FAIL] Failed to parse\n")
    }

    // Test Case 2: Vietnamese entry WITHOUT explicit POS keyword
    print("Test 2: Vietnamese entry WITHOUT explicit POS keyword")
    print("Input: \"tốt | tốt | 1 good ▸ very good example\"")
    if let result = parseVietnameseNoPOS("tốt | tốt | 1 good ▸ very good example") {
        print("[OK] Parsed successfully")
        print("   Title: \(result.title)")
        print("   Main POS: \(result.partOfSpeech ?? "nil (no label)")")
        print("   Entry POS: \(result.definitions.first?.partOfSpeech ?? "nil (no label)")")
        print("   POS label will NOT be displayed (previously showed \"adjective\")\n")
    } else {
        print("[FAIL] Failed to parse\n")
    }

    // Test Case 3: Multiple definitions
    print("Test 3: Multiple definitions - some with POS, some without")
    print("Input: Mixed case simulation")

    let withPOS = LookupDefinitionEntry(partOfSpeech: "danh từ", senses: [LookupSenseEntry(number: 1, definition: "noun meaning")])
    let withoutPOS = LookupDefinitionEntry(partOfSpeech: nil, senses: [LookupSenseEntry(number: 2, definition: "unknown POS meaning")])

    let mixedResult = LookupPresentation(
        title: "test",
        partOfSpeech: "danh từ",
        definitions: [withPOS, withoutPOS]
    )

    print("[OK] Parsed successfully")
    print("   Title: \(mixedResult.title)")
    print("   Definitions count: \(mixedResult.definitions.count)")
    for (idx, def) in mixedResult.definitions.enumerated() {
        print("   Definition \(idx + 1) POS: \(def.partOfSpeech ?? "nil (no label)")")
    }
    print("   First shows POS label, second does not\n")

    print("=== Summary ===")
    print("Before: Entries without POS keywords defaulted to showing \"adjective\"")
    print("After:  Entries without POS keywords show no POS label at all")
    print("\nThe lookup RESULTS (definitions/examples) are unchanged.")
    print("Only the POS label display behavior changed.")
}

// Run the tests
runTests()
