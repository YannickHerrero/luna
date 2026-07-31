import Foundation
import Testing
@testable import Luna

struct LunaTimeTests {
    @Test
    func formatsConversationRecencyLikeThePWA() throws {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(secondsFromGMT: 0)!
        let locale = Locale(identifier: "en_US")
        let now = try #require(LunaTime.parse("2026-03-20T15:00:00Z"))

        #expect(
            LunaTime.conversationTimestamp(
                "2026-03-20T09:05:00Z",
                now: now,
                calendar: calendar,
                locale: locale
            ) == "9:05 AM"
        )
        #expect(
            LunaTime.conversationTimestamp(
                "2026-03-18T09:05:00Z",
                now: now,
                calendar: calendar,
                locale: locale
            ) == "Wed"
        )
        #expect(
            LunaTime.conversationTimestamp(
                "2026-02-01T09:05:00Z",
                now: now,
                calendar: calendar,
                locale: locale
            ) == "Feb 1"
        )
    }

    @Test
    func parsesFractionalServerTimestamps() {
        #expect(LunaTime.parse("2026-03-20T09:05:00.123Z") != nil)
        #expect(LunaTime.parse("2026-03-20T09:05:00Z") != nil)
    }
}
