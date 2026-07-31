import SwiftUI

struct LunaIconButton: View {
    let icon: LunaIcon
    let accessibilityLabel: String
    var isAccent = false
    var foreground: Color?
    var action: () -> Void

    @Environment(\.lunaPalette) private var palette

    var body: some View {
        Button(action: action) {
            LunaIconView(icon: icon, size: 18)
                .foregroundStyle(foreground ?? (isAccent ? Color.white : palette.muted))
                .frame(width: LunaShape.minimumTarget, height: LunaShape.minimumTarget)
                .background(isAccent ? palette.accent : .clear)
                .clipShape(Circle())
                .shadow(
                    color: isAccent ? palette.accent.opacity(0.28) : .clear,
                    radius: 10,
                    y: 8
                )
                .contentShape(Circle())
        }
        .buttonStyle(.plain)
        .frame(
            minWidth: LunaShape.minimumTarget,
            minHeight: LunaShape.minimumTarget
        )
        .accessibilityLabel(accessibilityLabel)
    }
}
