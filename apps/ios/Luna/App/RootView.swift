import SwiftUI

struct RootView: View {
    @Environment(\.lunaPalette) private var palette

    var body: some View {
        ZStack {
            LunaBackground()
            VStack(spacing: 14) {
                Text("☾")
                    .font(LunaFont.display(44))
                    .foregroundStyle(palette.accent)
                Text("Luna")
                    .font(LunaFont.display(40, weight: .bold))
                    .foregroundStyle(palette.foreground)
            }
        }
    }
}

#Preview {
    RootView()
}
