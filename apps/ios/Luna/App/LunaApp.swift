import SwiftUI

@main
struct LunaApp: App {
    @AppStorage("luna-theme") private var storedTheme = LunaTheme.latte.rawValue

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
