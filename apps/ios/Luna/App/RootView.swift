import SwiftUI
#if DEBUG
import WidgetKit
#endif

struct RootView: View {
    @State private var model: AppModel
    @Environment(\.scenePhase) private var scenePhase

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
#if DEBUG
            if ProcessInfo.processInfo.arguments.contains("-ui-testing-usage-widget-preview") {
                OpenAIUsageWidgetPreviewGallery()
            } else if ProcessInfo.processInfo.arguments.contains("-ui-testing-widget-preview") {
                ActiveAgentsWidgetPreviewGallery()
            } else {
                rootContent
            }
#else
            rootContent
#endif
        }
        .task {
            if model.phase == .loading {
                await model.start()
            }
        }
        .task(id: model.phase) {
            if model.phase == .ready {
                await model.resolvePendingRoute()
                await model.refreshOpenAIWeeklyUsage()
            }
        }
        .onChange(of: scenePhase) { _, next in
            if next == .active {
                Task { await model.refreshOpenAIWeeklyUsage() }
            }
        }
        .onOpenURL { url in
            Task { await model.open(url) }
        }
    }

    @ViewBuilder
    private var rootContent: some View {
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
}

#if DEBUG
private struct ActiveAgentsWidgetPreviewGallery: View {
    private let entry = LunaWidgetEntry.placeholder()
    @Environment(\.colorScheme) private var colorScheme

    private var background: Color {
        colorScheme == .dark ? LunaColors.Mocha.background : LunaColors.Latte.background
    }

    private var foreground: Color {
        colorScheme == .dark ? LunaColors.Mocha.foreground : LunaColors.Latte.foreground
    }

    private var border: Color {
        colorScheme == .dark ? LunaColors.Mocha.border : LunaColors.Latte.border
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 22) {
                Text("A2 · Activity field")
                    .font(.system(.largeTitle, design: .serif, weight: .semibold))
                    .foregroundStyle(foreground)
                widgetCard(family: .systemSmall, width: 170, height: 170)
                widgetCard(family: .systemMedium, width: 345, height: 170)
            }
            .padding(24)
        }
        .background(background)
        .accessibilityIdentifier("active-agents-widget-preview")
    }

    private func widgetCard(
        family: WidgetFamily,
        width: CGFloat,
        height: CGFloat
    ) -> some View {
        LunaActiveAgentsWidgetView(entry: entry, family: family)
            .padding(16)
            .frame(width: width, height: height)
            .background(LunaWidgetBackground())
            .clipShape(RoundedRectangle(cornerRadius: 26, style: .continuous))
            .overlay {
                RoundedRectangle(cornerRadius: 26, style: .continuous)
                    .stroke(border, lineWidth: 1)
            }
            .shadow(color: foreground.opacity(0.08), radius: 18, y: 10)
    }
}

private struct OpenAIUsageWidgetPreviewGallery: View {
    private let entry = OpenAIUsageWidgetEntry.placeholder()
    @Environment(\.colorScheme) private var colorScheme

    private var background: Color {
        colorScheme == .dark ? LunaColors.Mocha.background : LunaColors.Latte.background
    }

    private var foreground: Color {
        colorScheme == .dark ? LunaColors.Mocha.foreground : LunaColors.Latte.foreground
    }

    private var border: Color {
        colorScheme == .dark ? LunaColors.Mocha.border : LunaColors.Latte.border
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 22) {
                Text("B2 · Capacity line")
                    .font(.system(.largeTitle, design: .serif, weight: .semibold))
                    .foregroundStyle(foreground)
                widgetCard(family: .systemSmall, width: 170, height: 170)
                widgetCard(family: .systemMedium, width: 345, height: 170)
            }
            .padding(24)
        }
        .background(background)
        .accessibilityIdentifier("openai-usage-widget-preview")
    }

    private func widgetCard(
        family: WidgetFamily,
        width: CGFloat,
        height: CGFloat
    ) -> some View {
        OpenAIWeeklyUsageWidgetView(entry: entry, family: family)
            .padding(16)
            .frame(width: width, height: height)
            .background(OpenAIUsageWidgetBackground())
            .clipShape(RoundedRectangle(cornerRadius: 26, style: .continuous))
            .overlay {
                RoundedRectangle(cornerRadius: 26, style: .continuous)
                    .stroke(border, lineWidth: 1)
            }
            .shadow(color: foreground.opacity(0.08), radius: 18, y: 10)
    }
}
#endif

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
