import Foundation
import Testing
@testable import Luna

@MainActor
struct LunaRouteTests {
    @Test
    func parsesStableHomeAndConversationLinks() throws {
        let id = UUID(uuidString: "00000000-0000-0000-0000-000000000003")!

        #expect(LunaRoute(url: LunaRoute.home.url) == .home)
        #expect(LunaRoute(url: LunaRoute.conversation(id).url) == .conversation(id))
        #expect(LunaRoute(url: URL(string: "https://example.com/conversation/\(id)")!) == nil)
        #expect(LunaRoute(url: URL(string: "luna://conversation/not-a-uuid")!) == nil)
        #expect(LunaRoute(url: URL(string: "luna://conversation/\(id)?source=push")!) == nil)
    }

    @Test
    func routesAReadyAppToAConversationAndHome() async {
        let id = UUID(uuidString: "00000000-0000-0000-0000-000000000003")!
        let model = PreviewFixtures.appModel(showConversationList: true)

        await model.open(LunaRoute.conversation(id).url)
        #expect(model.conversationStore?.selectedConversationId == id)

        await model.open(LunaRoute.home.url)
        #expect(model.conversationStore?.selectedConversationId == nil)
    }
}
