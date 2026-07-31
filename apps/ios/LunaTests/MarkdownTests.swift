import Testing
@testable import Luna

struct MarkdownTests {
    @Test
    func parsesGFMBlocksAndTaskLists() {
        let document = MarkdownDocument(
            """
            # Status

            Ready with **strong text**.

            - [x] Finished
            - [ ] Remaining

            | Client | State |
            | --- | --- |
            | iPhone | Ready |

            > Durable sessions

            ```swift
            let ready = true
            ```
            """
        )

        #expect(document.blocks.count == 6)
        #expect(document.blocks[0] == .heading(level: 1, text: "Status"))
        #expect(
            document.blocks[2] == .unorderedList([
                MarkdownListItem(text: "Finished", checked: true),
                MarkdownListItem(text: "Remaining", checked: false),
            ])
        )
        #expect(
            document.blocks[3] == .table(
                headers: ["Client", "State"],
                rows: [["iPhone", "Ready"]]
            )
        )
        #expect(document.blocks[4] == .blockquote("Durable sessions"))
        #expect(document.blocks[5] == .code(language: "swift", source: "let ready = true"))
    }

    @Test
    func parsesOrderedListsAndThematicBreaks() {
        let document = MarkdownDocument("1. First\n2. Second\n\n---")
        #expect(
            document.blocks == [
                .orderedList([
                    MarkdownListItem(text: "First", checked: nil),
                    MarkdownListItem(text: "Second", checked: nil),
                ]),
                .thematicBreak,
            ]
        )
    }

    @Test
    func highlightsCodeTokensWithoutChangingSource() {
        let source = "let answer = 42 // Luna\nprint(\"ready\")"
        let tokens = CodeHighlighter.tokenize(source, language: "swift")

        #expect(tokens.map(\.text).joined() == source)
        #expect(tokens.contains(CodeToken(text: "let", kind: .keyword)))
        #expect(tokens.contains(CodeToken(text: "42", kind: .number)))
        #expect(tokens.contains(CodeToken(text: "// Luna", kind: .comment)))
        #expect(tokens.contains(CodeToken(text: "\"ready\"", kind: .string)))
    }
}
