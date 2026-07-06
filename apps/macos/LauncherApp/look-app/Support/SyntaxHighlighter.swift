import AppKit
import SwiftUI

/// Lightweight syntax highlighter for the file preview pane.
/// Tokenizes by language and applies foreground colors via
/// AttributedString. Not as accurate as a real parser - designed for
/// "readable at a glance," not editor-quality fidelity.
nonisolated enum SyntaxHighlighter {
    enum Language {
        case c, cpp, css, go, html, java, javascript, json, markdown,
             python, ruby, rust, shell, swift, typescript, yaml, plain

        static func from(path: String) -> Language {
            switch (path as NSString).pathExtension.lowercased() {
            case "go": return .go
            case "swift": return .swift
            case "rs": return .rust
            case "py": return .python
            case "rb": return .ruby
            case "js", "mjs", "cjs", "jsx": return .javascript
            case "ts", "tsx": return .typescript
            case "c", "h", "m": return .c
            case "cpp", "cc", "cxx", "hpp", "hh", "hxx", "mm": return .cpp
            case "java", "kt", "kts", "scala", "groovy": return .java
            case "sh", "bash", "zsh", "fish": return .shell
            case "html", "htm", "xml", "svg", "vue", "svelte": return .html
            case "css", "scss", "sass", "less": return .css
            case "json": return .json
            case "yaml", "yml", "toml": return .yaml
            case "md", "markdown", "rst": return .markdown
            default: return .plain
            }
        }
    }

    static func highlight(_ source: String, path: String) -> NSAttributedString {
        let lang = Language.from(path: path)
        if lang == .plain { return NSAttributedString(string: source) }

        // Build with NSMutableAttributedString + NSRange so each token's
        // attribute set is O(1). Using AttributedString.Index per token
        // would walk the String from startIndex each time - O(n²) on a
        // 64 KB file with thousands of tokens.
        let keywordC = NSColor(srgbRed: 0.82, green: 0.49, blue: 0.79, alpha: 1)
        let stringC  = NSColor(srgbRed: 0.91, green: 0.66, blue: 0.42, alpha: 1)
        let commentC = NSColor(srgbRed: 0.45, green: 0.47, blue: 0.45, alpha: 1)
        let numberC  = NSColor(srgbRed: 0.55, green: 0.78, blue: 0.84, alpha: 1)

        let mut = NSMutableAttributedString(string: source)
        let keywords = keywords(for: lang)
        let (lineCmt, blockCmt) = commentDelimiters(for: lang)
        let ns = source as NSString
        let len = ns.length

        func paint(_ start: Int, _ end: Int, _ color: NSColor) {
            mut.addAttribute(.foregroundColor, value: color,
                             range: NSRange(location: start, length: end - start))
        }

        var i = 0
        while i < len {
            // Line comment
            if let pfx = lineCmt, i + pfx.count <= len,
               ns.substring(with: NSRange(location: i, length: pfx.count)) == pfx {
                let nl = ns.range(of: "\n", options: [],
                                  range: NSRange(location: i, length: len - i))
                let end = nl.location == NSNotFound ? len : nl.location
                paint(i, end, commentC)
                i = end; continue
            }
            // Block comment
            if let bc = blockCmt, i + bc.0.count <= len,
               ns.substring(with: NSRange(location: i, length: bc.0.count)) == bc.0 {
                let after = i + bc.0.count
                let r = ns.range(of: bc.1, options: [],
                                 range: NSRange(location: after, length: len - after))
                let end = r.location == NSNotFound ? len : r.location + r.length
                paint(i, end, commentC)
                i = end; continue
            }
            let c = ns.character(at: i)
            // String: " ' `
            if c == 34 || c == 39 || c == 96 {
                var j = i + 1
                while j < len {
                    let k = ns.character(at: j)
                    if k == 92, j + 1 < len { j += 2; continue }  // backslash escape
                    if k == c { j += 1; break }
                    if k == 10 { break }  // newline → unterminated
                    j += 1
                }
                paint(i, j, stringC); i = j; continue
            }
            // Number
            if c >= 48 && c <= 57 {
                var j = i
                while j < len {
                    let k = ns.character(at: j)
                    if (k >= 48 && k <= 57) || k == 46 || k == 95 || k == 120
                        || (k >= 97 && k <= 102) || (k >= 65 && k <= 70) { j += 1 }
                    else { break }
                }
                paint(i, j, numberC); i = j; continue
            }
            // Identifier / keyword
            if (c >= 65 && c <= 90) || (c >= 97 && c <= 122) || c == 95 {
                var j = i
                while j < len {
                    let k = ns.character(at: j)
                    if (k >= 48 && k <= 57) || (k >= 65 && k <= 90)
                        || (k >= 97 && k <= 122) || k == 95 { j += 1 }
                    else { break }
                }
                let word = ns.substring(with: NSRange(location: i, length: j - i))
                if keywords.contains(word) { paint(i, j, keywordC) }
                i = j; continue
            }
            i += 1
        }
        return mut
    }

    private static func keywords(for lang: Language) -> Set<String> {
        switch lang {
        case .go: return ["break","case","chan","const","continue","default","defer","else","fallthrough","for","func","go","goto","if","import","interface","map","package","range","return","select","struct","switch","type","var","nil","true","false","iota"]
        case .swift: return ["associatedtype","class","deinit","enum","extension","fileprivate","func","import","init","inout","internal","let","open","operator","private","protocol","public","static","struct","subscript","typealias","var","break","case","continue","default","defer","do","else","fallthrough","for","guard","if","in","repeat","return","switch","where","while","as","Any","catch","false","is","nil","rethrows","self","Self","super","throw","throws","true","try","async","await","actor"]
        case .rust: return ["as","async","await","break","const","continue","crate","dyn","else","enum","extern","false","fn","for","if","impl","in","let","loop","match","mod","move","mut","pub","ref","return","self","Self","static","struct","super","trait","true","type","unsafe","use","where","while"]
        case .python: return ["False","None","True","and","as","assert","async","await","break","class","continue","def","del","elif","else","except","finally","for","from","global","if","import","in","is","lambda","nonlocal","not","or","pass","raise","return","try","while","with","yield"]
        case .javascript, .typescript: return ["var","let","const","function","return","if","else","for","while","do","switch","case","default","break","continue","new","this","super","class","extends","import","export","from","async","await","yield","typeof","instanceof","in","of","try","catch","finally","throw","true","false","null","undefined","void","delete","interface","type","enum","public","private","protected","readonly"]
        case .c: return ["auto","break","case","char","const","continue","default","do","double","else","enum","extern","float","for","goto","if","inline","int","long","register","restrict","return","short","signed","sizeof","static","struct","switch","typedef","union","unsigned","void","volatile","while"]
        case .cpp: return ["alignas","alignof","auto","bool","break","case","catch","char","class","const","constexpr","continue","decltype","default","delete","do","double","else","enum","explicit","export","extern","false","float","for","friend","goto","if","inline","int","long","mutable","namespace","new","noexcept","nullptr","operator","private","protected","public","return","short","signed","sizeof","static","static_cast","struct","switch","template","this","throw","true","try","typedef","typeid","typename","union","unsigned","using","virtual","void","volatile","while"]
        case .java: return ["abstract","assert","boolean","break","byte","case","catch","char","class","const","continue","default","do","double","else","enum","extends","false","final","finally","float","for","goto","if","implements","import","instanceof","int","interface","long","native","new","null","package","private","protected","public","return","short","static","strictfp","super","switch","synchronized","this","throw","throws","transient","true","try","void","volatile","while","fun","val","var"]
        case .ruby: return ["alias","and","begin","break","case","class","def","do","else","elsif","end","ensure","false","for","if","in","module","next","nil","not","or","redo","rescue","retry","return","self","super","then","true","undef","unless","until","when","while","yield"]
        case .shell: return ["if","then","else","elif","fi","case","esac","for","while","do","done","in","function","return","local","export","unset","readonly","declare","let","alias","source"]
        case .json, .yaml: return ["true","false","null"]
        default: return []
        }
    }

    private static func commentDelimiters(for lang: Language) -> (String?, (String, String)?) {
        switch lang {
        case .python, .ruby, .shell, .yaml: return ("#", nil)
        case .html: return (nil, ("<!--", "-->"))
        case .css: return (nil, ("/*", "*/"))
        case .json, .markdown, .plain: return (nil, nil)
        default: return ("//", ("/*", "*/"))
        }
    }
}
