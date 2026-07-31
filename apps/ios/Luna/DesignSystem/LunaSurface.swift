import SwiftUI

struct LunaBackground: View {
    @Environment(\.lunaPalette) private var palette

    var body: some View {
        palette.background
            .overlay(alignment: .topLeading) {
                RadialGradient(
                    colors: [palette.accent.opacity(0.15), .clear],
                    center: .topLeading,
                    startRadius: 0,
                    endRadius: 430
                )
            }
            .ignoresSafeArea()
    }
}

struct LunaCardModifier: ViewModifier {
    @Environment(\.lunaPalette) private var palette
    let cornerRadius: CGFloat

    func body(content: Content) -> some View {
        content
            .background(palette.surface.opacity(0.94))
            .clipShape(RoundedRectangle(cornerRadius: cornerRadius, style: .continuous))
            .overlay {
                RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
                    .stroke(palette.border.opacity(0.82), lineWidth: 1)
            }
            .shadow(color: palette.foreground.opacity(0.10), radius: 35, y: 22)
    }
}

extension View {
    func lunaCard(cornerRadius: CGFloat = 28) -> some View {
        modifier(LunaCardModifier(cornerRadius: cornerRadius))
    }
}
