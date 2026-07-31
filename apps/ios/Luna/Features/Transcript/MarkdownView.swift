import SwiftUI

struct MarkdownView: View {
    let source: String

    @Environment(\.lunaPalette) private var palette
    private let document: MarkdownDocument

    init(_ source: String) {
        self.source = source
        document = MarkdownDocument(source)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            ForEach(Array(document.blocks.enumerated()), id: \.offset) { _, block in
                blockView(block)
            }
        }
        .textSelection(.enabled)
    }

    @ViewBuilder
    private func blockView(_ block: MarkdownBlock) -> some View {
        switch block {
        case let .heading(level, text):
            Text(inline(text, font: headingFont(level)))
                .lineSpacing(3)
                .padding(.top, level == 1 ? 4 : 1)
        case let .paragraph(text):
            Text(inline(text, font: LunaFont.body(14)))
                .lineSpacing(5)
        case let .unorderedList(items):
            list(items: items, ordered: false)
        case let .orderedList(items):
            list(items: items, ordered: true)
        case let .blockquote(text):
            Text(inline(text, font: LunaFont.body(14)))
                .foregroundStyle(palette.muted)
                .lineSpacing(5)
                .padding(.leading, 14)
                .overlay(alignment: .leading) {
                    Capsule()
                        .fill(palette.accent)
                        .frame(width: 3)
                }
        case let .code(language, source):
            codeBlock(source, language: language)
        case let .table(headers, rows):
            table(headers: headers, rows: rows)
        case .thematicBreak:
            Rectangle()
                .fill(palette.border)
                .frame(height: 1)
                .padding(.vertical, 3)
        }
    }

    private func headingFont(_ level: Int) -> Font {
        switch level {
        case 1: LunaFont.display(27, weight: .bold)
        case 2: LunaFont.display(22, weight: .bold)
        case 3: LunaFont.display(18, weight: .bold)
        default: LunaFont.display(16, weight: .bold)
        }
    }

    private func list(items: [MarkdownListItem], ordered: Bool) -> some View {
        VStack(alignment: .leading, spacing: 7) {
            ForEach(Array(items.enumerated()), id: \.offset) { index, item in
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    listMark(item: item, index: index, ordered: ordered)
                        .frame(width: 18, alignment: .trailing)
                    Text(inline(item.text, font: LunaFont.body(14)))
                        .lineSpacing(4)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
        }
        .padding(.leading, 3)
    }

    @ViewBuilder
    private func listMark(item: MarkdownListItem, index: Int, ordered: Bool) -> some View {
        if let checked = item.checked {
            LunaIconView(icon: checked ? .check : .circle, size: checked ? 14 : 11)
                .foregroundStyle(checked ? palette.green : palette.muted)
                .accessibilityLabel(checked ? "Completed" : "Not completed")
        } else if ordered {
            Text("\(index + 1).")
                .font(LunaFont.mono(11))
                .foregroundStyle(palette.muted)
        } else {
            Text("•")
                .font(LunaFont.body(14, weight: .bold))
                .foregroundStyle(palette.accent)
        }
    }

    private func codeBlock(_ source: String, language: String?) -> some View {
        ScrollView(.horizontal) {
            Text(highlightedCode(source, language: language))
                .font(LunaFont.mono(12))
                .lineSpacing(5)
                .textSelection(.enabled)
                .fixedSize(horizontal: true, vertical: true)
                .padding(15)
        }
        .scrollIndicators(.visible)
        .background(palette.raised)
        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .stroke(palette.border, lineWidth: 1)
        }
        .accessibilityLabel(language.map { "\($0) code" } ?? "Code")
    }

    private func table(headers: [String], rows: [[String]]) -> some View {
        ScrollView(.horizontal) {
            Grid(horizontalSpacing: 0, verticalSpacing: 0) {
                GridRow {
                    ForEach(Array(headers.enumerated()), id: \.offset) { _, header in
                        tableCell(header, isHeader: true)
                    }
                }
                ForEach(Array(rows.enumerated()), id: \.offset) { _, row in
                    GridRow {
                        ForEach(headers.indices, id: \.self) { column in
                            tableCell(column < row.count ? row[column] : "", isHeader: false)
                        }
                    }
                }
            }
            .overlay {
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .stroke(palette.border, lineWidth: 1)
            }
        }
        .scrollIndicators(.visible)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Table")
    }

    private func tableCell(_ text: String, isHeader: Bool) -> some View {
        Text(inline(text, font: LunaFont.body(12, weight: isHeader ? .bold : .regular)))
            .frame(minWidth: 90, maxWidth: 220, alignment: .leading)
            .padding(.horizontal, 9)
            .padding(.vertical, 7)
            .background(isHeader ? palette.raised : palette.surface)
            .overlay {
                Rectangle().stroke(palette.border, lineWidth: 0.5)
            }
    }

    private func inline(_ source: String, font: Font) -> AttributedString {
        var value = (try? AttributedString(
            markdown: source,
            options: AttributedString.MarkdownParsingOptions(
                interpretedSyntax: .inlineOnlyPreservingWhitespace,
                failurePolicy: .returnPartiallyParsedIfPossible
            )
        )) ?? AttributedString(source)
        value.font = font
        value.foregroundColor = palette.foreground
        for run in value.runs {
            if run.link != nil {
                value[run.range].foregroundColor = palette.blue
                value[run.range].underlineStyle = .single
            }
            guard let intent = run.inlinePresentationIntent else { continue }
            if intent.contains(.code) {
                value[run.range].font = LunaFont.mono(12)
                value[run.range].backgroundColor = palette.raised
            }
        }
        return value
    }

    private func highlightedCode(_ source: String, language: String?) -> AttributedString {
        var result = AttributedString()
        for token in CodeHighlighter.tokenize(source, language: language) {
            var value = AttributedString(token.text)
            value.foregroundColor = switch token.kind {
            case .plain: palette.foreground
            case .keyword: palette.mauve
            case .string: palette.green
            case .number: palette.peach
            case .comment: palette.muted
            }
            result.append(value)
        }
        return result
    }
}
