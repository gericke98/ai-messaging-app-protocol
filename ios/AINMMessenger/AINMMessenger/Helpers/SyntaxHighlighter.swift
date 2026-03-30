import SwiftUI
import Foundation

/// Regex-based syntax highlighting for ~10 common languages per D-08.
/// Uses NSRegularExpression to apply foreground colors to AttributedString.
/// No third-party dependencies.
enum SyntaxHighlighter {
    struct Rule {
        let pattern: String
        let color: Color
    }

    // MARK: - Shared Rules

    static let stringRule = Rule(
        pattern: #"\"[^\"\\]*(?:\\.[^\"\\]*)*\""#,
        color: Color(red: 0.84, green: 0.36, blue: 0.36)
    )
    static let singleQuoteStringRule = Rule(
        pattern: #"'[^'\\]*(?:\\.[^'\\]*)*'"#,
        color: Color(red: 0.84, green: 0.36, blue: 0.36)
    )
    static let singleLineCommentRule = Rule(
        pattern: #"//[^\n]*"#,
        color: .secondary
    )
    static let hashCommentRule = Rule(
        pattern: #"#[^\n]*"#,
        color: .secondary
    )
    static let multiLineCommentRule = Rule(
        pattern: #"/\*[\s\S]*?\*/"#,
        color: .secondary
    )
    static let numberRule = Rule(
        pattern: #"\b\d+(\.\d+)?\b"#,
        color: Color(red: 0.69, green: 0.56, blue: 0.99)
    )

    // MARK: - Language Keyword Sets

    static let swiftKeywords = [
        "func", "var", "let", "if", "else", "return", "struct", "class",
        "enum", "import", "guard", "switch", "case", "for", "while", "try",
        "catch", "throw", "async", "await", "protocol", "extension", "private",
        "public", "static", "self", "nil", "true", "false"
    ]
    static let pythonKeywords = [
        "def", "class", "if", "elif", "else", "return", "import", "from",
        "for", "while", "try", "except", "raise", "with", "as", "lambda",
        "yield", "pass", "break", "continue", "None", "True", "False", "in",
        "not", "and", "or", "is", "async", "await"
    ]
    static let jsKeywords = [
        "function", "const", "let", "var", "if", "else", "return", "class",
        "import", "export", "from", "for", "while", "try", "catch", "throw",
        "new", "this", "async", "await", "switch", "case", "default", "break",
        "continue", "null", "undefined", "true", "false", "typeof", "instanceof"
    ]
    static let rustKeywords = [
        "fn", "let", "mut", "if", "else", "return", "struct", "enum", "impl",
        "use", "pub", "mod", "match", "for", "while", "loop", "break",
        "continue", "async", "await", "self", "Self", "true", "false", "crate",
        "super", "where", "trait", "type"
    ]
    static let goKeywords = [
        "func", "var", "const", "if", "else", "return", "struct", "interface",
        "import", "package", "for", "range", "switch", "case", "default", "go",
        "chan", "select", "defer", "map", "type", "nil", "true", "false", "make",
        "append"
    ]
    static let javaKeywords = [
        "public", "private", "protected", "class", "interface", "extends",
        "implements", "import", "return", "if", "else", "for", "while", "try",
        "catch", "throw", "new", "this", "static", "final", "void", "int",
        "String", "boolean", "null", "true", "false", "switch", "case", "break"
    ]

    // MARK: - Public API

    /// Highlight code with language-specific syntax coloring.
    /// Returns an AttributedString with monospaced font and syntax colors applied.
    static func highlight(_ code: String, language: String) -> AttributedString {
        var attributed = AttributedString(code)
        attributed.font = .system(.body, design: .monospaced)
        attributed.foregroundColor = .primary

        let rules = rulesForLanguage(language.lowercased())
        let nsString = code as NSString
        let fullRange = NSRange(location: 0, length: nsString.length)

        for rule in rules {
            guard let regex = try? NSRegularExpression(
                pattern: rule.pattern,
                options: [.dotMatchesLineSeparators]
            ) else { continue }
            let matches = regex.matches(in: code, range: fullRange)
            for match in matches {
                guard let swiftRange = Range(match.range, in: code),
                      let attrRange = Range(swiftRange, in: attributed) else { continue }
                attributed[attrRange].foregroundColor = rule.color
            }
        }
        return attributed
    }

    // MARK: - Private Helpers

    private static func keywordRule(for keywords: [String]) -> Rule {
        let escaped = keywords.map { NSRegularExpression.escapedPattern(for: $0) }
        let pattern = "\\b(" + escaped.joined(separator: "|") + ")\\b"
        return Rule(
            pattern: pattern,
            color: Color(red: 0.37, green: 0.61, blue: 0.84)
        )
    }

    private static func rulesForLanguage(_ language: String) -> [Rule] {
        var rules: [Rule] = []

        // Language-specific keywords
        switch language {
        case "swift":
            rules.append(keywordRule(for: swiftKeywords))
        case "python", "py":
            rules.append(keywordRule(for: pythonKeywords))
        case "javascript", "js":
            rules.append(keywordRule(for: jsKeywords))
        case "typescript", "ts":
            rules.append(keywordRule(for: jsKeywords))
        case "rust", "rs":
            rules.append(keywordRule(for: rustKeywords))
        case "go", "golang":
            rules.append(keywordRule(for: goKeywords))
        case "java":
            rules.append(keywordRule(for: javaKeywords))
        default:
            break
        }

        // Shared rules (order matters: strings > comments > numbers)
        rules.append(stringRule)
        rules.append(singleQuoteStringRule)

        switch language {
        case "python", "py", "bash", "sh", "yaml", "yml":
            rules.append(hashCommentRule)
        default:
            rules.append(singleLineCommentRule)
            rules.append(multiLineCommentRule)
        }

        rules.append(numberRule)
        return rules
    }
}
