import SwiftUI

struct LunaMoonMark: View {
    @Environment(\.lunaPalette) private var palette

    var body: some View {
        Text("☾")
            .font(LunaFont.display(44))
            .frame(width: 58, height: 58)
            .foregroundStyle(palette.accent)
            .background(palette.accent.opacity(0.10))
            .clipShape(RoundedRectangle(cornerRadius: 20, style: .continuous))
            .overlay {
                RoundedRectangle(cornerRadius: 20, style: .continuous)
                    .stroke(palette.accent.opacity(0.40), lineWidth: 1)
            }
            .shadow(color: palette.accent.opacity(0.18), radius: 23, y: 14)
            .accessibilityHidden(true)
    }
}

struct LunaPrimaryButtonStyle: ButtonStyle {
    @Environment(\.lunaPalette) private var palette
    @Environment(\.isEnabled) private var isEnabled

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(LunaFont.body(14, weight: .bold))
            .foregroundStyle(palette.onAccent)
            .frame(minHeight: LunaShape.minimumTarget)
            .padding(.horizontal, 18)
            .background(palette.accent.opacity(isEnabled ? 1 : 0.65))
            .clipShape(RoundedRectangle(cornerRadius: 13, style: .continuous))
            .shadow(color: palette.accent.opacity(0.24), radius: 14, y: 10)
            .scaleEffect(configuration.isPressed ? 0.98 : 1)
            .animation(.easeOut(duration: LunaMotion.standardDuration), value: configuration.isPressed)
    }
}

struct LunaSecondaryButtonStyle: ButtonStyle {
    @Environment(\.lunaPalette) private var palette
    @Environment(\.isEnabled) private var isEnabled

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(LunaFont.body(14, weight: .bold))
            .foregroundStyle(palette.foreground)
            .frame(minHeight: LunaShape.minimumTarget)
            .padding(.horizontal, 16)
            .background(palette.raised)
            .clipShape(RoundedRectangle(cornerRadius: 13, style: .continuous))
            .overlay {
                RoundedRectangle(cornerRadius: 13, style: .continuous)
                    .stroke(configuration.isPressed ? palette.accent : palette.border, lineWidth: 1)
            }
            .opacity(isEnabled ? 1 : 0.65)
    }
}

struct LunaFieldModifier: ViewModifier {
    @Environment(\.lunaPalette) private var palette

    func body(content: Content) -> some View {
        content
            .font(LunaFont.body(16))
            .foregroundStyle(palette.foreground)
            .padding(.horizontal, 13)
            .frame(minHeight: LunaShape.minimumTarget)
            .background(palette.background)
            .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
            .overlay {
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .stroke(palette.border, lineWidth: 1)
            }
    }
}

extension View {
    func lunaField() -> some View {
        modifier(LunaFieldModifier())
    }
}
