import Testing
@testable import LunaWatch

struct LunaWatchTests {
    @Test
    func companionCopySetsAccurateExpectations() {
        #expect(LunaWatchCopy.status == "Companion ready")
        #expect(LunaWatchCopy.detail.contains("coming soon"))
        #expect(LunaWatchCopy.detail.contains("iPhone"))
    }
}
