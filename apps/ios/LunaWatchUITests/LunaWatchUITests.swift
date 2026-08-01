import XCTest

final class LunaWatchUITests: XCTestCase {
    @MainActor
    func testCompanionLaunches() {
        let application = XCUIApplication()
        application.launch()

        XCTAssertTrue(application.staticTexts["Luna"].waitForExistence(timeout: 5))
        XCTAssertTrue(application.descendants(matching: .any)["watch-companion-status"].exists)
    }

    @MainActor
    func testSmartStackWidgetLayoutRenders() {
        let application = XCUIApplication()
        application.launchArguments = ["-ui-testing-widget-preview"]
        application.launch()

        XCTAssertTrue(
            application.descendants(matching: .any)["watch-widget-preview"]
                .waitForExistence(timeout: 5)
        )
        XCTAssertTrue(application.staticTexts["C3 · Work pulse"].exists)
        XCTAssertTrue(
            application.descendants(matching: .any).matching(
                NSPredicate(
                    format: "label == %@",
                    "Luna. 2 agents active. Reviewing files."
                )
            ).firstMatch.exists
        )
    }
}
