import XCTest

final class LunaUITests: XCTestCase {
    @MainActor
    func testApplicationLaunches() throws {
        let application = XCUIApplication()
        application.launch()
        XCTAssertTrue(application.staticTexts["Pair with Luna"].waitForExistence(timeout: 5))
        XCTAssertTrue(application.textFields["pairing-code"].exists)
        XCTAssertTrue(application.textFields["device-name"].exists)
        try application.performAccessibilityAudit(for: .dynamicType)
    }

    @MainActor
    func testReadyScreensPassAccessibilityAudit() throws {
        let application = XCUIApplication()
        application.launchArguments = [
            "-ui-testing-ready", "-ui-testing-list", "-luna-theme", "latte",
        ]
        application.launch()

        XCTAssertTrue(application.staticTexts["Luna"].waitForExistence(timeout: 5))
        try application.performAccessibilityAudit(for: [.dynamicType, .hitRegion])

        application.buttons["conversation-00000000-0000-0000-0000-000000000001"].tap()
        XCTAssertTrue(application.staticTexts["Native client parity"].waitForExistence(timeout: 5))
        try application.performAccessibilityAudit(for: [.dynamicType, .hitRegion])

        application.buttons["Back"].tap()
        let agentConversation = application.buttons[
            "conversation-00000000-0000-0000-0000-000000000003"
        ]
        XCTAssertTrue(agentConversation.waitForExistence(timeout: 5))
        agentConversation.tap()
        XCTAssertTrue(application.buttons["Agent settings"].waitForExistence(timeout: 5))
        application.buttons["Agent settings"].tap()
        XCTAssertTrue(application.staticTexts["Agent settings"].waitForExistence(timeout: 5))
        try application.performAccessibilityAudit(for: [.dynamicType, .hitRegion])
    }

    @MainActor
    func testTranscriptShowsProgressAndWorkingActivity() {
        let application = XCUIApplication()
        application.launchArguments = ["-ui-testing-ready", "-luna-theme", "latte"]
        application.launch()

        XCTAssertTrue(application.buttons["Back"].waitForExistence(timeout: 5))
        XCTAssertTrue(application.staticTexts["Native client parity"].waitForExistence(timeout: 5))
        XCTAssertTrue(application.staticTexts["Verifying Markdown and task progress"].exists)
        XCTAssertTrue(
            application.staticTexts.matching(
                NSPredicate(format: "label CONTAINS 'await luna.connect()'")
            ).firstMatch.exists
        )
    }

    @MainActor
    func testComposerSendsDraft() {
        let application = XCUIApplication()
        application.launchArguments = ["-ui-testing-ready", "-luna-theme", "latte"]
        application.launch()

        let editor = application.textViews["Steer Pi"]
        XCTAssertTrue(editor.waitForExistence(timeout: 5))
        editor.tap()
        editor.typeText("Review this draft")
        XCTAssertEqual(editor.value as? String, "Review this draft")
        let keyboardScreenshot = XCTAttachment(screenshot: application.screenshot())
        keyboardScreenshot.name = "Composer with keyboard"
        keyboardScreenshot.lifetime = .keepAlways
        add(keyboardScreenshot)
        application.buttons["send-message"].tap()
        XCTAssertTrue(application.staticTexts["Review this draft"].waitForExistence(timeout: 5))
    }

    @MainActor
    func testComposerWrapsLongDraft() {
        let application = XCUIApplication()
        application.launchArguments = ["-ui-testing-ready", "-luna-theme", "latte"]
        application.launch()

        let editor = application.textViews["Steer Pi"]
        XCTAssertTrue(editor.waitForExistence(timeout: 5))
        let singleLineHeight = editor.frame.height
        editor.tap()
        editor.typeText(
            Array(
                repeating: "This long Luna prompt should wrap inside the composer.",
                count: 8
            ).joined(separator: " ")
        )

        let wraps = NSPredicate { _, _ in
            editor.frame.height > singleLineHeight + 1
        }
        expectation(for: wraps, evaluatedWith: editor)
        waitForExpectations(timeout: 5)
        XCTAssertLessThanOrEqual(editor.frame.height, 176)

        let screenshot = XCTAttachment(screenshot: application.screenshot())
        screenshot.name = "Composer wrapping a long prompt"
        screenshot.lifetime = .keepAlways
        add(screenshot)
    }

    @MainActor
    func testConversationControlsCompactRenameAndArchive() {
        let application = XCUIApplication()
        application.launchArguments = [
            "-ui-testing-ready", "-ui-testing-list", "-luna-theme", "latte",
        ]
        application.launch()

        let conversation = application.buttons.matching(
            NSPredicate(format: "label CONTAINS 'Notification service'")
        ).firstMatch
        XCTAssertTrue(conversation.waitForExistence(timeout: 5))
        conversation.tap()

        application.buttons["Agent settings"].tap()
        XCTAssertTrue(application.staticTexts["Agent settings"].waitForExistence(timeout: 5))
        let modelControl = application.buttons["agent-model"]
        let thinkingControl = application.buttons["thinking-level"]
        XCTAssertTrue(modelControl.exists)
        XCTAssertTrue(thinkingControl.exists)
        XCTAssertTrue(application.staticTexts["48K / 200K tokens"].exists)

        modelControl.tap()
        let gptModel = application.buttons["GPT-5"]
        XCTAssertTrue(gptModel.waitForExistence(timeout: 2))
        gptModel.tap()
        thinkingControl.tap()
        let extraHighThinking = application.buttons["Extra high"]
        XCTAssertTrue(extraHighThinking.waitForExistence(timeout: 2))
        extraHighThinking.tap()
        application.buttons["apply-agent-settings"].tap()
        XCTAssertTrue(application.staticTexts["GPT-5"].waitForExistence(timeout: 5))
        XCTAssertTrue(application.staticTexts["Extra high"].exists)

        application.buttons["Compact context"].tap()
        XCTAssertTrue(application.staticTexts["Pi will summarize older context while preserving recent work."].exists)
        application.buttons["Compact now"].tap()
        XCTAssertTrue(application.staticTexts["12K / 200K tokens"].waitForExistence(timeout: 5))
        let settingsScreenshot = XCTAttachment(screenshot: application.screenshot())
        settingsScreenshot.name = "Agent settings after compaction"
        settingsScreenshot.lifetime = .keepAlways
        add(settingsScreenshot)
        application.buttons["Close agent settings"].tap()

        application.buttons["rename-conversation"].tap()
        let titleField = application.alerts["Conversation title"].textFields.firstMatch
        XCTAssertTrue(titleField.waitForExistence(timeout: 2))
        XCTAssertEqual(titleField.value as? String, "Notification service")
        titleField.tap()
        titleField.typeKey("a", modifierFlags: .command)
        titleField.typeText("APNs service")
        XCTAssertEqual(titleField.value as? String, "APNs service")
        application.buttons["Rename"].tap()
        let titleButton = application.buttons["rename-conversation"]
        let renamedTitle = NSPredicate(format: "label == %@", "Rename APNs service")
        expectation(for: renamedTitle, evaluatedWith: titleButton)
        waitForExpectations(timeout: 5)

        application.buttons["Archive conversation"].tap()
        XCTAssertTrue(application.alerts["Archive “APNs service”?"].waitForExistence(timeout: 2))
        application.buttons["Archive"].tap()
        XCTAssertTrue(application.textFields["Search conversations"].waitForExistence(timeout: 5))
    }

    @MainActor
    func testBusyConversationDisablesAgentMutations() {
        let application = XCUIApplication()
        application.launchArguments = [
            "-ui-testing-ready", "-ui-testing-list", "-luna-theme", "latte",
        ]
        application.launch()

        let conversation = application.buttons.matching(
            NSPredicate(format: "label CONTAINS 'Launch Luna'")
        ).firstMatch
        XCTAssertTrue(conversation.waitForExistence(timeout: 5))
        conversation.tap()
        application.buttons["Agent settings"].tap()

        let model = application.buttons["agent-model"]
        XCTAssertTrue(model.waitForExistence(timeout: 5))
        XCTAssertFalse(model.isEnabled)
        XCTAssertFalse(application.buttons["thinking-level"].isEnabled)
        XCTAssertFalse(application.buttons["apply-agent-settings"].isEnabled)
        XCTAssertFalse(application.buttons["compact-context"].isEnabled)
    }

    @MainActor
    func testReadyFixtureNavigatesFromListToConversation() {
        let application = XCUIApplication()
        application.launchArguments = [
            "-ui-testing-ready", "-ui-testing-list", "-luna-theme", "latte",
        ]
        application.launch()

        XCTAssertTrue(application.staticTexts["Luna"].waitForExistence(timeout: 5))
        XCTAssertTrue(application.textFields["Search conversations"].exists)
        XCTAssertTrue(application.buttons["New conversation"].exists)
        let conversation = application.buttons.matching(
            NSPredicate(format: "label CONTAINS 'Launch Luna'")
        ).firstMatch
        XCTAssertTrue(conversation.exists)
        conversation.tap()
        XCTAssertTrue(application.buttons["Back"].waitForExistence(timeout: 5))
    }
}
