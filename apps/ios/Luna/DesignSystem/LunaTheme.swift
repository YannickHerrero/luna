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

private struct LunaMonoFontModifier: ViewModifier {
    @ScaledMetric(relativeTo: .body) private var scaledSize: CGFloat = 0
    let weight: Font.Weight

    init(size: CGFloat, weight: Font.Weight) {
        _scaledSize = ScaledMetric(wrappedValue: size, relativeTo: .body)
        self.weight = weight
    }

    func body(content: Content) -> some View {
        content.font(.system(size: scaledSize, weight: weight, design: .monospaced))
    }
}

extension View {
    func lunaMonoFont(
        _ size: CGFloat,
        weight: Font.Weight = .regular
    ) -> some View {
        modifier(LunaMonoFontModifier(size: size, weight: weight))
    }
}

enum LunaFont {
    static func display(_ size: CGFloat, weight: Font.Weight = .regular) -> Font {
        .custom("Iowan Old Style", size: size, relativeTo: textStyle(for: size))
            .weight(weight)
    }

    static func body(_ size: CGFloat, weight: Font.Weight = .regular) -> Font {
        .custom("SF Pro Rounded", size: size, relativeTo: textStyle(for: size))
            .weight(weight)
    }

    static func mono(_ size: CGFloat, weight: Font.Weight = .regular) -> Font {
        .custom("SF Pro Rounded", size: size, relativeTo: textStyle(for: size))
            .monospaced()
            .weight(weight)
    }

    private static func textStyle(for size: CGFloat) -> Font.TextStyle {
        switch size {
        case ..<13: .caption
        case ..<15: .footnote
        case ..<18: .body
        case ..<21: .headline
        case ..<24: .title3
        case ..<28: .title2
        case ..<34: .title
        default: .largeTitle
        }
    }
}
