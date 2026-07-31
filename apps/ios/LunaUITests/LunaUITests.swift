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
}
