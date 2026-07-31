import SwiftUI

struct RootView: View {
    @State private var model: AppModel

    init(model: AppModel = AppModel()) {
        _model = State(initialValue: model)
    }

    var body: some View {
        Group {
            switch model.phase {
            case .loading:
                LoadingView()
            case .pairing:
                PairingView(model: model)
            case .ready:
                ReadyPlaceholderView()
            }
        }
        .task {
            if model.phase == .loading {
                await model.start()
            }
        }
    }
}

private struct LoadingView: View {
    @Environment(\.lunaPalette) private var palette

    var body: some View {
        ZStack {
            LunaBackground()
            VStack(spacing: 20) {
                LunaMoonMark()
                ProgressView()
                    .tint(palette.accent)
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Loading Luna")
    }
}

private struct ReadyPlaceholderView: View {
    @Environment(\.lunaPalette) private var palette

    var body: some View {
        ZStack {
            LunaBackground()
            Text("Luna")
                .font(LunaFont.display(40, weight: .bold))
                .foregroundStyle(palette.foreground)
        }
    }
}
