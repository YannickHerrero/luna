import SwiftUI
import WidgetKit

@main
struct LunaWidgetBundle: WidgetBundle {
    var body: some Widget {
        LunaActiveAgentsWidget()
        LunaOpenAIWeeklyUsageWidget()
    }
}
