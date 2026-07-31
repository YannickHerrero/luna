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
                if let store = model.conversationStore {
                    ReadyPlaceholderView(store: store)
                } else {
                    LoadingView()
                }
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
    @Bindable var store: ConversationStore
    @Environment(\.lunaPalette) private var palette
    @Environment(\.scenePhase) private var scenePhase

    var body: some View {
        ZStack {
            LunaBackground()
            Text("Luna")
                .font(LunaFont.display(40, weight: .bold))
                .foregroundStyle(palette.foreground)
        }
        .task {
            store.startRealtime()
            await store.loadSelectedMessages()
        }
        .onChange(of: scenePhase) { _, next in
            if next == .active {
                store.resumeRealtime()
            } else if next == .background {
                store.stopRealtime()
            }
        }
    }
}
