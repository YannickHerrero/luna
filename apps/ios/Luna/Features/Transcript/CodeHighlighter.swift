import Foundation

enum CodeTokenKind: Equatable, Sendable {
    case plain
    case keyword
    case string
    case number
    case comment
}

struct CodeToken: Equatable, Sendable {
    let text: String
    let kind: CodeTokenKind
}

enum CodeHighlighter {
    private static let keywords: Set<String> = [
        "actor", "async", "await", "break", "case", "catch", "class", "const", "continue",
        "default", "defer", "do", "else", "enum", "export", "extension", "false", "final",
        "for", "from", "func", "function", "guard", "if", "import", "in", "interface", "let",
        "nil", "null", "private", "protocol", "public", "return", "some", "static", "struct",
        "switch", "throw", "throws", "true", "try", "typealias", "var", "while",
    ]

    static func tokenize(_ source: String, language: String? = nil) -> [CodeToken] {
        var tokens: [CodeToken] = []
        var index = source.startIndex
        let hashComments = ["bash", "sh", "shell", "python", "py", "ruby", "rb", "yaml", "yml"]
            .contains(language?.lowercased() ?? "")

        while index < source.endIndex {
            if source[index...].hasPrefix("//") || source[index...].hasPrefix("/*") {
                let block = source[index...].hasPrefix("/*")
                let end: String.Index
                if block, let closing = source[index...].range(of: "*/") {
                    end = closing.upperBound
                } else {
                    end = source[index...].firstIndex(of: "\n") ?? source.endIndex
                }
                append(String(source[index..<end]), kind: .comment, to: &tokens)
                index = end
                continue
            }

            if hashComments, source[index] == "#" {
                let end = source[index...].firstIndex(of: "\n") ?? source.endIndex
                append(String(source[index..<end]), kind: .comment, to: &tokens)
                index = end
                continue
            }

            if source[index] == "\"" || source[index] == "'" || source[index] == "`" {
                let quote = source[index]
                var end = source.index(after: index)
                var escaped = false
                while end < source.endIndex {
                    let character = source[end]
                    end = source.index(after: end)
                    if character == quote, !escaped { break }
                    if character == "\\", !escaped {
                        escaped = true
                    } else {
                        escaped = false
                    }
                }
                append(String(source[index..<end]), kind: .string, to: &tokens)
                index = end
                continue
            }

            if source[index].isNumber {
                var end = source.index(after: index)
                while end < source.endIndex,
                      source[end].isNumber || [".", "_", "x", "a", "b", "c", "d", "e", "f"]
                        .contains(source[end].lowercased().first!)
                {
                    end = source.index(after: end)
                }
                append(String(source[index..<end]), kind: .number, to: &tokens)
                index = end
                continue
            }

            if source[index].isLetter || source[index] == "_" {
                var end = source.index(after: index)
                while end < source.endIndex, source[end].isLetter || source[end].isNumber || source[end] == "_" {
                    end = source.index(after: end)
                }
                let word = String(source[index..<end])
                append(word, kind: keywords.contains(word) ? .keyword : .plain, to: &tokens)
                index = end
                continue
            }

            let end = source.index(after: index)
            append(String(source[index..<end]), kind: .plain, to: &tokens)
            index = end
        }

        return tokens
    }

    private static func append(_ text: String, kind: CodeTokenKind, to tokens: inout [CodeToken]) {
        guard !text.isEmpty else { return }
        if tokens.last?.kind == kind {
            let previous = tokens.removeLast()
            tokens.append(CodeToken(text: previous.text + text, kind: kind))
        } else {
            tokens.append(CodeToken(text: text, kind: kind))
        }
    }
}
