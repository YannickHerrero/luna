import SwiftUI

struct LunaPalette: Sendable {
    let background: Color
    let surface: Color
    let foreground: Color
    let muted: Color
    let border: Color
    let accent: Color
    let onAccent: Color
    let raised: Color
    let overlay: Color
    let blue: Color
    let green: Color
    let peach: Color
    let red: Color
    let mauve: Color
}

enum LunaTheme: String, CaseIterable, Sendable {
    case latte
    case mocha

    var palette: LunaPalette {
        switch self {
        case .latte:
            LunaPalette(
                background: LunaColors.Latte.background,
                surface: LunaColors.Latte.surface,
                foreground: LunaColors.Latte.foreground,
                muted: LunaColors.Latte.muted,
                border: LunaColors.Latte.border,
                accent: LunaColors.Latte.accent,
                onAccent: LunaColors.Latte.onAccent,
                raised: LunaColors.Latte.raised,
                overlay: LunaColors.Latte.overlay,
                blue: LunaColors.Latte.blue,
                green: LunaColors.Latte.green,
                peach: LunaColors.Latte.peach,
                red: LunaColors.Latte.red,
                mauve: LunaColors.Latte.mauve
            )
        case .mocha:
            LunaPalette(
                background: LunaColors.Mocha.background,
                surface: LunaColors.Mocha.surface,
                foreground: LunaColors.Mocha.foreground,
                muted: LunaColors.Mocha.muted,
                border: LunaColors.Mocha.border,
                accent: LunaColors.Mocha.accent,
                onAccent: LunaColors.Mocha.onAccent,
                raised: LunaColors.Mocha.raised,
                overlay: LunaColors.Mocha.overlay,
                blue: LunaColors.Mocha.blue,
                green: LunaColors.Mocha.green,
                peach: LunaColors.Mocha.peach,
                red: LunaColors.Mocha.red,
                mauve: LunaColors.Mocha.mauve
            )
        }
    }

    var colorScheme: ColorScheme {
        self == .latte ? .light : .dark
    }

    var displayName: String {
        self == .latte ? "Catppuccin Latte" : "Catppuccin Mocha"
    }
}

private struct LunaThemeKey: EnvironmentKey {
    static let defaultValue = LunaTheme.latte
}

extension EnvironmentValues {
    var lunaTheme: LunaTheme {
        get { self[LunaThemeKey.self] }
        set { self[LunaThemeKey.self] = newValue }
    }

    var lunaPalette: LunaPalette {
        lunaTheme.palette
    }
}

extension View {
    func lunaTheme(_ theme: LunaTheme) -> some View {
        environment(\.lunaTheme, theme)
            .preferredColorScheme(theme.colorScheme)
            .tint(theme.palette.accent)
    }
}

enum LunaFont {
    static func display(_ size: CGFloat, weight: Font.Weight = .regular) -> Font {
        .custom("Iowan Old Style", fixedSize: size).weight(weight)
    }

    static func body(_ size: CGFloat, weight: Font.Weight = .regular) -> Font {
        .system(size: size, weight: weight, design: .rounded)
    }

    static func mono(_ size: CGFloat, weight: Font.Weight = .regular) -> Font {
        .system(size: size, weight: weight, design: .monospaced)
    }
}
