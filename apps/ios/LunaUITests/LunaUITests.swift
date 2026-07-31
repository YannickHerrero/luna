import XCTest

final class LunaUITests: XCTestCase {
    @MainActor
    func testApplicationLaunches() {
        let application = XCUIApplication()
        application.launch()
        XCTAssertTrue(application.staticTexts["Luna"].waitForExistence(timeout: 5))
    }
}
