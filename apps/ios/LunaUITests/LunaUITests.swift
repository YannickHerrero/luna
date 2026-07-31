import XCTest

final class LunaUITests: XCTestCase {
    @MainActor
    func testApplicationLaunches() {
        let application = XCUIApplication()
        application.launch()
        XCTAssertTrue(application.staticTexts["Pair with Luna"].waitForExistence(timeout: 5))
        XCTAssertTrue(application.textFields["pairing-code"].exists)
        XCTAssertTrue(application.textFields["device-name"].exists)
    }

    @MainActor
    func testTranscriptShowsProgressAndWorkingActivity() {
        let application = XCUIApplication()
        application.launchArguments = ["-ui-testing-ready"]
        application.launch()

        XCTAssertTrue(application.otherElements["conversation-panel"].waitForExistence(timeout: 5))
        XCTAssertTrue(application.staticTexts["Native client parity"].waitForExistence(timeout: 5))
        XCTAssertTrue(application.staticTexts["Verifying Markdown and task progress"].exists)
        XCTAssertTrue(
            application.staticTexts.matching(
                NSPredicate(format: "label CONTAINS 'await luna.connect()'")
            ).firstMatch.exists
        )
    }

    @MainActor
    func testReadyFixtureNavigatesFromListToConversation() {
        let application = XCUIApplication()
        application.launchArguments = ["-ui-testing-ready", "-ui-testing-list"]
        application.launch()

        XCTAssertTrue(application.staticTexts["Luna"].waitForExistence(timeout: 5))
        XCTAssertTrue(application.textFields["Search conversations"].exists)
        XCTAssertTrue(application.buttons["New conversation"].exists)
        let conversation = application.buttons.matching(
            NSPredicate(format: "label CONTAINS 'Launch Luna'")
        ).firstMatch
        XCTAssertTrue(conversation.exists)
        conversation.tap()
        XCTAssertTrue(application.otherElements["conversation-panel"].waitForExistence(timeout: 5))
        XCTAssertTrue(application.buttons["Back"].exists)
    }
}
