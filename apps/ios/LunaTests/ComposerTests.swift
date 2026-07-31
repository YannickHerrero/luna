import Foundation
import Testing
@testable import Luna

struct ComposerTests {
    @Test
    func classifiesMessagesStopsAndShellCommands() {
        #expect(ComposerSubmission.parse(text: "  ", attachmentCount: 0) == .empty)
        #expect(ComposerSubmission.parse(text: " /stop\n", attachmentCount: 0) == .stop)
        #expect(
            ComposerSubmission.parse(text: "!git status", attachmentCount: 1)
                == .invalidShellAttachments
        )
        #expect(
            ComposerSubmission.parse(text: "", attachmentCount: 1)
                == .message(text: "Please review the attached image.")
        )
        #expect(
            ComposerSubmission.parse(text: "  keep context  ", attachmentCount: 0)
                == .message(text: "keep context")
        )
    }

    @Test
    func persistsTextButKeepsAttachmentDataEphemeral() throws {
        let suite = try #require(UserDefaults(suiteName: "ComposerTests.\(UUID())"))
        let conversationId = UUID()
        let persistence = ComposerDraftPersistence(defaults: suite, prefix: "draft:")

        persistence.setText("Continue here", for: conversationId)
        #expect(persistence.text(for: conversationId) == "Continue here")
        persistence.setText("", for: conversationId)
        #expect(persistence.text(for: conversationId).isEmpty)
    }

    @Test
    func abbreviatesOnlyUserHomeDirectories() {
        #expect(abbreviatedWorkingDirectory("/Users/yannick/dev/luna") == "~/dev/luna")
        #expect(abbreviatedWorkingDirectory("/Users/yannick") == "~")
        #expect(abbreviatedWorkingDirectory("/srv/luna") == "/srv/luna")
    }
}
