import XCTest

final class LunaWatchUITests: XCTestCase {
    @MainActor
    func testCompanionLaunches() {
        let application = XCUIApplication()
        application.launch()

        XCTAssertTrue(application.staticTexts["Luna"].waitForExistence(timeout: 5))
        XCTAssertTrue(application.descendants(matching: .any)["watch-companion-status"].exists)
    }
}
