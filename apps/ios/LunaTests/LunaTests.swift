import Testing
@testable import Luna

struct LunaTests {
    @Test
    func themesMatchThePWA() {
        #expect(LunaTheme.allCases == [.latte, .mocha])
        #expect(LunaTheme.latte.displayName == "Catppuccin Latte")
        #expect(LunaTheme.mocha.displayName == "Catppuccin Mocha")
        #expect(LunaShape.minimumTarget == 44)
        #expect(LunaMotion.standardDuration == 0.2)
    }

    @Test
    func generatedIconsCoverPWAControls() {
        #expect(LunaIcon.allCases.contains(.send))
        #expect(LunaIcon.allCases.contains(.settings))
        #expect(LunaIcon.allCases.contains(.triangleAlert))
    }
}
