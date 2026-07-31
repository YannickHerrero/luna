import SwiftUI

struct RootView: View {
    @State private var model: AppModel

    init(model: AppModel) {
        _model = State(initialValue: model)
    }

    init() {
#if DEBUG
        let arguments = ProcessInfo.processInfo.arguments
        if arguments.contains("-ui-testing-ready") {
            _model = State(
                initialValue: PreviewFixtures.appModel(
                    showConversationList: arguments.contains("-ui-testing-list")
                )
            )
            return
        }
#endif
        _model = State(initialValue: AppModel())
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
                    ConversationShellView(store: store)
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
