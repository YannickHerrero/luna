import SwiftUI

@main
struct LunaApp: App {
    @AppStorage("luna-theme") private var storedTheme = LunaTheme.latte.rawValue

    init() {
#if DEBUG
        let arguments = ProcessInfo.processInfo.arguments
        if arguments.contains("-ui-testing-reset-grouping") {
            UserDefaults.standard.set(false, forKey: "luna-group-conversations")
        } else if arguments.contains("-ui-testing-grouped") {
            UserDefaults.standard.set(true, forKey: "luna-group-conversations")
        }
#endif
    }

    private var theme: LunaTheme {
        LunaTheme(rawValue: storedTheme) ?? .latte
    }

    var body: some Scene {
        WindowGroup {
            RootView()
                .lunaTheme(theme)
        }
    }
}
