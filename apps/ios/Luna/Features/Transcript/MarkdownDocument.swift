import Foundation

struct MarkdownDocument: Equatable, Sendable {
    let blocks: [MarkdownBlock]

    init(_ source: String) {
        blocks = MarkdownParser.parse(source)
    }
}

enum MarkdownBlock: Equatable, Sendable {
    case heading(level: Int, text: String)
    case paragraph(String)
    case unorderedList([MarkdownListItem])
    case orderedList([MarkdownListItem])
    case blockquote(String)
    case code(language: String?, source: String)
    case table(headers: [String], rows: [[String]])
    case thematicBreak
}

struct MarkdownListItem: Equatable, Sendable {
    let text: String
    let checked: Bool?
}

enum MarkdownParser {
    static func parse(_ source: String) -> [MarkdownBlock] {
        let lines = source.replacingOccurrences(of: "\r\n", with: "\n")
            .split(separator: "\n", omittingEmptySubsequences: false)
            .map(String.init)
        var blocks: [MarkdownBlock] = []
        var index = 0

        while index < lines.count {
            let line = lines[index]
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            if trimmed.isEmpty {
                index += 1
                continue
            }

            if trimmed.hasPrefix("```") {
                let language = String(trimmed.dropFirst(3)).trimmingCharacters(in: .whitespaces)
                var code: [String] = []
                index += 1
                while index < lines.count,
                      !lines[index].trimmingCharacters(in: .whitespaces).hasPrefix("```")
                {
                    code.append(lines[index])
                    index += 1
                }
                if index < lines.count { index += 1 }
                blocks.append(
                    .code(
                        language: language.isEmpty ? nil : language,
                        source: code.joined(separator: "\n")
                    )
                )
                continue
            }

            if let heading = heading(in: trimmed) {
                blocks.append(.heading(level: heading.level, text: heading.text))
                index += 1
                continue
            }

            if isThematicBreak(trimmed) {
                blocks.append(.thematicBreak)
                index += 1
                continue
            }

            if index + 1 < lines.count,
               isTableRow(line),
               isTableDivider(lines[index + 1])
            {
                let headers = tableCells(line)
                var rows: [[String]] = []
                index += 2
                while index < lines.count, isTableRow(lines[index]), !lines[index].isEmpty {
                    rows.append(tableCells(lines[index]))
                    index += 1
                }
                blocks.append(.table(headers: headers, rows: rows))
                continue
            }

            if trimmed.hasPrefix(">") {
                var quoted: [String] = []
                while index < lines.count {
                    let candidate = lines[index].trimmingCharacters(in: .whitespaces)
                    guard candidate.hasPrefix(">") else { break }
                    quoted.append(
                        String(candidate.dropFirst()).trimmingCharacters(in: .whitespaces)
                    )
                    index += 1
                }
                blocks.append(.blockquote(quoted.joined(separator: "\n")))
                continue
            }

            if unorderedContent(trimmed) != nil {
                var items: [MarkdownListItem] = []
                while index < lines.count,
                      let content = unorderedContent(
                        lines[index].trimmingCharacters(in: .whitespaces)
                      )
                {
                    items.append(listItem(content))
                    index += 1
                }
                blocks.append(.unorderedList(items))
                continue
            }

            if orderedContent(trimmed) != nil {
                var items: [MarkdownListItem] = []
                while index < lines.count,
                      let content = orderedContent(
                        lines[index].trimmingCharacters(in: .whitespaces)
                      )
                {
                    items.append(listItem(content))
                    index += 1
                }
                blocks.append(.orderedList(items))
                continue
            }

            var paragraph = [trimmed]
            index += 1
            while index < lines.count {
                let candidate = lines[index].trimmingCharacters(in: .whitespaces)
                guard !candidate.isEmpty, !startsBlock(lines: lines, at: index) else { break }
                paragraph.append(candidate)
                index += 1
            }
            blocks.append(.paragraph(paragraph.joined(separator: "\n")))
        }

        return blocks
    }

    private static func heading(in line: String) -> (level: Int, text: String)? {
        let count = line.prefix(while: { $0 == "#" }).count
        guard (1...6).contains(count), line.dropFirst(count).first == " " else { return nil }
        return (count, String(line.dropFirst(count + 1)))
    }

    private static func isThematicBreak(_ line: String) -> Bool {
        let compact = line.filter { !$0.isWhitespace }
        guard compact.count >= 3, let first = compact.first, ["-", "_", "*"].contains(first) else {
            return false
        }
        return compact.allSatisfy { $0 == first }
    }

    private static func unorderedContent(_ line: String) -> String? {
        guard line.count >= 2,
              ["-", "*", "+"].contains(line.first!),
              line.dropFirst().first == " "
        else { return nil }
        return String(line.dropFirst(2))
    }

    private static func orderedContent(_ line: String) -> String? {
        guard let dot = line.firstIndex(of: "."), dot != line.startIndex else { return nil }
        let prefix = line[..<dot]
        guard prefix.allSatisfy(\.isNumber) else { return nil }
        let afterDot = line.index(after: dot)
        guard afterDot < line.endIndex, line[afterDot] == " " else { return nil }
        return String(line[line.index(after: afterDot)...])
    }

    private static func listItem(_ content: String) -> MarkdownListItem {
        let lowercased = content.lowercased()
        if lowercased.hasPrefix("[x] ") {
            return MarkdownListItem(text: String(content.dropFirst(4)), checked: true)
        }
        if content.hasPrefix("[ ] ") {
            return MarkdownListItem(text: String(content.dropFirst(4)), checked: false)
        }
        return MarkdownListItem(text: content, checked: nil)
    }

    private static func isTableRow(_ line: String) -> Bool {
        line.contains("|") && tableCells(line).count > 1
    }

    private static func isTableDivider(_ line: String) -> Bool {
        let cells = tableCells(line)
        guard cells.count > 1 else { return false }
        return cells.allSatisfy { cell in
            let value = cell.trimmingCharacters(in: CharacterSet(charactersIn: ":"))
            return value.count >= 3 && value.allSatisfy { $0 == "-" }
        }
    }

    private static func tableCells(_ line: String) -> [String] {
        var value = line.trimmingCharacters(in: .whitespaces)
        if value.hasPrefix("|") { value.removeFirst() }
        if value.hasSuffix("|") { value.removeLast() }
        return value.split(separator: "|", omittingEmptySubsequences: false)
            .map { $0.trimmingCharacters(in: .whitespaces) }
    }

    private static func startsBlock(lines: [String], at index: Int) -> Bool {
        let line = lines[index].trimmingCharacters(in: .whitespaces)
        return line.hasPrefix("```")
            || heading(in: line) != nil
            || isThematicBreak(line)
            || line.hasPrefix(">")
            || unorderedContent(line) != nil
            || orderedContent(line) != nil
            || (index + 1 < lines.count && isTableRow(line) && isTableDivider(lines[index + 1]))
    }
}
