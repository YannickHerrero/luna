import SwiftUI

struct AgentControlsView: View {
    @Bindable var store: ConversationStore
    let conversationId: UUID
    let busy: Bool

    @Environment(\.dismiss) private var dismiss
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    @Environment(\.lunaPalette) private var palette
    @State private var agent: ConversationAgentState?
    @State private var selectedModelKey = ""
    @State private var selectedThinking = ThinkingLevel.off
    @State private var loading = false
    @State private var saving = false
    @State private var compacting = false
    @State private var confirmingCompaction = false
    @State private var estimatedTokens: UInt64?

    private var selectedModel: AgentModel? {
        agent?.availableModels.first { modelKey($0) == selectedModelKey }
    }

    private var supportedThinking: [ThinkingLevel] {
        selectedModel?.supportedThinkingLevels ?? [.off]
    }

    private var controlsDisabled: Bool {
        busy || loading || saving || compacting
    }

    private var contextWindow: UInt64? {
        agent?.contextUsage?.contextWindow ?? selectedModel?.contextWindow
    }

    private var contextTokens: UInt64? {
        estimatedTokens ?? agent?.contextUsage?.tokens
    }

    private var contextPercent: Double? {
        agentContextPercent(
            tokens: contextTokens,
            contextWindow: contextWindow,
            fallback: agent?.contextUsage?.percent
        )
    }

    var body: some View {
        ZStack {
            palette.surface.ignoresSafeArea()
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    header
                    content
                }
                .padding(.horizontal, 22)
                .padding(.top, 22)
                .padding(.bottom, 28)
                .frame(maxWidth: 540)
                .frame(maxWidth: .infinity)
            }
        }
        .task { await load() }
        .accessibilityIdentifier("agent-settings")
    }

    private var header: some View {
        Group {
            if dynamicTypeSize.isAccessibilitySize {
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        Spacer()
                        closeButton
                    }
                    headerLabels
                }
            } else {
                HStack(alignment: .center, spacing: 14) {
                    headerLabels
                    Spacer(minLength: 14)
                    closeButton
                }
            }
        }
    }

    private var headerLabels: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("CONVERSATION CONTROLS")
                .lunaMonoFont(10, weight: .bold)
                .tracking(1.3)
                .foregroundStyle(palette.accent.mix(with: palette.foreground, by: 0.70))
            Text("Agent settings")
                .font(LunaFont.display(27, weight: .bold))
                .tracking(-0.8)
                .foregroundStyle(palette.foreground)
        }
        .fixedSize(horizontal: false, vertical: true)
    }

    private var closeButton: some View {
        LunaIconButton(
            icon: .x,
            accessibilityLabel: "Close agent settings",
            action: { dismiss() }
        )
    }

    @ViewBuilder
    private var content: some View {
        if loading, agent == nil {
            ProgressView()
                .tint(palette.accent)
                .frame(maxWidth: .infinity, minHeight: 280)
                .accessibilityLabel("Loading agent settings")
        } else if let agent {
            VStack(spacing: 17) {
                modelSection(agent)
                thinkingSection
                applyButton
                Text("Like Pi’s model selector, this also becomes the default for new Pi sessions.")
                    .font(LunaFont.body(11))
                    .foregroundStyle(palette.foreground.opacity(0.86))
                    .multilineTextAlignment(.center)
                    .fixedSize(horizontal: false, vertical: true)
                    .frame(maxWidth: .infinity)
                    .padding(.top, -10)
                contextCard(agent)
            }
            .padding(.top, 24)
            .accessibilityElement(children: .contain)
        } else {
            Text("Agent settings are unavailable.")
                .font(LunaFont.body(11))
                .foregroundStyle(palette.foreground.opacity(0.86))
                .frame(maxWidth: .infinity, minHeight: 140)
                .padding(.top, 24)
        }
    }

    private func modelSection(_ agent: ConversationAgentState) -> some View {
        VStack(alignment: .leading, spacing: 7) {
            settingLabel("Model")
            Menu {
                ForEach(groupModels(agent.availableModels)) { group in
                    Section(group.provider) {
                        ForEach(group.models) { model in
                            Button {
                                selectModel(model)
                            } label: {
                                if modelKey(model) == selectedModelKey {
                                    Label(model.name, systemImage: "checkmark")
                                } else {
                                    Text(model.name)
                                }
                            }
                        }
                    }
                }
            } label: {
                selectionLabel(selectedModel?.name ?? "Choose a model")
            }
            .disabled(controlsDisabled)
            .accessibilityLabel("Model")
            .accessibilityIdentifier("agent-model")
        }
    }

    private var thinkingSection: some View {
        VStack(alignment: .leading, spacing: 7) {
            settingLabel("Thinking effort")
            Menu {
                ForEach(supportedThinking, id: \.self) { level in
                    Button {
                        selectedThinking = level
                    } label: {
                        if level == selectedThinking {
                            Label(level.displayName, systemImage: "checkmark")
                        } else {
                            Text(level.displayName)
                        }
                    }
                }
            } label: {
                selectionLabel(selectedThinking.displayName)
            }
            .disabled(controlsDisabled || supportedThinking.count == 1)
            .accessibilityLabel("Thinking effort")
            .accessibilityIdentifier("thinking-level")

            if supportedThinking.count == 1 {
                Text("This model does not support configurable reasoning.")
                    .font(LunaFont.body(11))
                    .foregroundStyle(palette.foreground.opacity(0.86))
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }

    private var applyButton: some View {
        Button {
            Task { await save() }
        } label: {
            Group {
                if saving {
                    ProgressView().tint(palette.onAccent)
                } else {
                    Text("Apply model settings")
                }
            }
            .frame(maxWidth: .infinity)
        }
        .buttonStyle(LunaPrimaryButtonStyle())
        .disabled(controlsDisabled || selectedModel == nil)
        .accessibilityIdentifier("apply-agent-settings")
    }

    private func contextCard(_ agent: ConversationAgentState) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Group {
                if dynamicTypeSize.isAccessibilitySize {
                    VStack(alignment: .leading, spacing: 8) {
                        contextLabels
                        contextPercentLabel
                    }
                } else {
                    HStack(alignment: .center, spacing: 14) {
                        contextLabels
                        Spacer(minLength: 10)
                        contextPercentLabel
                    }
                }
            }

            contextProgress

            Text(
                "Automatic compaction is \(agent.autoCompactionEnabled ? "enabled" : "disabled")."
                    + (estimatedTokens == nil ? "" : " Size shown is the post-compaction estimate.")
            )
            .font(LunaFont.body(11))
            .foregroundStyle(palette.foreground.opacity(0.86))
            .lineSpacing(3)
            .fixedSize(horizontal: false, vertical: true)

            if confirmingCompaction {
                VStack(alignment: .leading, spacing: 10) {
                    Text("Pi will summarize older context while preserving recent work.")
                        .font(LunaFont.body(11))
                        .foregroundStyle(palette.foreground.opacity(0.86))
                        .fixedSize(horizontal: false, vertical: true)
                    HStack(spacing: 10) {
                        Spacer()
                        Button("Cancel") { confirmingCompaction = false }
                            .buttonStyle(LunaSecondaryButtonStyle())
                            .disabled(compacting)
                        Button("Compact now") {
                            Task { await compact() }
                        }
                        .buttonStyle(LunaPrimaryButtonStyle())
                        .disabled(compacting)
                    }
                }
                .accessibilityElement(children: .contain)
                .accessibilityLabel("Confirm context compaction")
            } else {
                Button {
                    confirmingCompaction = true
                } label: {
                    Group {
                        if compacting {
                            ProgressView().tint(palette.foreground)
                        } else {
                            Text("Compact context")
                        }
                    }
                    .frame(maxWidth: .infinity)
                }
                .buttonStyle(LunaSecondaryButtonStyle())
                .disabled(controlsDisabled || contextTokens == nil)
                .accessibilityIdentifier("compact-context")
            }
        }
        .padding(17)
        .background(palette.raised)
        .clipShape(RoundedRectangle(cornerRadius: 17, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: 17, style: .continuous)
                .stroke(palette.border, lineWidth: 1)
        }
        .padding(.top, 5)
    }

    private var contextLabels: some View {
        VStack(alignment: .leading, spacing: 4) {
            settingLabel("Conversation context")
            Text(contextDescription)
                .font(LunaFont.body(14, weight: .bold))
                .foregroundStyle(palette.foreground)
        }
        .fixedSize(horizontal: false, vertical: true)
    }

    @ViewBuilder
    private var contextPercentLabel: some View {
        if let contextPercent {
            Text("\(Int(contextPercent.rounded()))%")
                .lunaMonoFont(14, weight: .bold)
                .foregroundStyle(palette.accent.mix(with: palette.foreground, by: 0.70))
                .accessibilityLabel("Conversation context used")
                .accessibilityValue("\(Int(contextPercent.rounded())) percent")
        }
    }

    private var contextDescription: String {
        guard let contextTokens else { return "Not measured yet" }
        return "\(formatAgentTokens(contextTokens)) / \(formatAgentTokens(contextWindow ?? 0)) tokens"
    }

    private var contextProgress: some View {
        GeometryReader { geometry in
            ZStack(alignment: .leading) {
                Capsule().fill(palette.muted.opacity(0.18))
                Capsule()
                    .fill(palette.accent)
                    .frame(width: geometry.size.width * (contextPercent ?? 0) / 100)
            }
        }
        .frame(height: 7)
        .accessibilityHidden(true)
    }

    private func settingLabel(_ text: String) -> some View {
        Text(text)
            .font(LunaFont.body(11, weight: .bold))
            .foregroundStyle(palette.foreground)
            .fixedSize(horizontal: false, vertical: true)
    }

    private func selectionLabel(_ text: String) -> some View {
        HStack(spacing: 10) {
            Text(text)
                .font(LunaFont.body(13, weight: .bold))
                .foregroundStyle(palette.foreground)
                .lineLimit(dynamicTypeSize.isAccessibilitySize ? nil : 1)
                .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 10)
            LunaIconView(icon: .chevronDown, size: 15)
                .foregroundStyle(palette.muted)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 9)
        .frame(maxWidth: .infinity, minHeight: 46)
        .background(palette.background)
        .clipShape(RoundedRectangle(cornerRadius: 13, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: 13, style: .continuous)
                .stroke(palette.border, lineWidth: 1)
        }
        .contentShape(Rectangle())
    }

    private func selectModel(_ model: AgentModel) {
        selectedModelKey = modelKey(model)
        selectedThinking = preferredThinkingLevel(
            current: selectedThinking,
            supported: model.supportedThinkingLevels
        )
    }

    private func load() async {
        loading = true
        store.errorMessage = nil
        defer { loading = false }
        do {
            install(try await store.loadAgentState(for: conversationId))
        } catch {
            store.errorMessage = message(from: error)
        }
    }

    private func save() async {
        guard let selectedModel else { return }
        saving = true
        store.errorMessage = nil
        defer { saving = false }
        do {
            install(
                try await store.updateAgentState(
                    for: conversationId,
                    request: UpdateConversationAgentRequest(
                        model: AgentModelSelection(
                            provider: selectedModel.provider,
                            modelId: selectedModel.id
                        ),
                        thinkingLevel: selectedThinking
                    )
                )
            )
        } catch {
            store.errorMessage = message(from: error)
        }
    }

    private func compact() async {
        compacting = true
        confirmingCompaction = false
        store.errorMessage = nil
        defer { compacting = false }
        do {
            estimatedTokens = try await store.compactConversation(conversationId)
                .estimatedTokensAfter
        } catch {
            store.errorMessage = message(from: error)
        }
    }

    private func install(_ state: ConversationAgentState) {
        agent = state
        selectedModelKey = state.model.map(modelKey) ?? ""
        selectedThinking = state.thinkingLevel
    }

    private func message(from error: Error) -> String {
        (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
    }
}

private struct AgentModelGroup: Identifiable {
    let provider: String
    var models: [AgentModel]
    var id: String { provider }
}

private func modelKey(_ model: AgentModel) -> String {
    "\(model.provider)\u{0}\(model.id)"
}

private func groupModels(_ models: [AgentModel]) -> [AgentModelGroup] {
    models.reduce(into: []) { groups, model in
        if let index = groups.firstIndex(where: { $0.provider == model.provider }) {
            groups[index].models.append(model)
        } else {
            groups.append(AgentModelGroup(provider: model.provider, models: [model]))
        }
    }
}

func preferredThinkingLevel(
    current: ThinkingLevel,
    supported: [ThinkingLevel]
) -> ThinkingLevel {
    if supported.contains(current) { return current }
    if supported.contains(.high) { return .high }
    return supported.last ?? .off
}

func agentContextPercent(
    tokens: UInt64?,
    contextWindow: UInt64?,
    fallback: Double?
) -> Double? {
    if let tokens, let contextWindow, contextWindow > 0 {
        return min(100, max(0, Double(tokens) / Double(contextWindow) * 100))
    }
    return fallback.map { min(100, max(0, $0)) }
}

func formatAgentTokens(_ value: UInt64) -> String {
    let units: [(threshold: Double, suffix: String)] = [
        (1_000_000_000, "B"),
        (1_000_000, "M"),
        (1_000, "K"),
    ]
    let number = Double(value)
    guard let unit = units.first(where: { number >= $0.threshold }) else {
        return String(value)
    }
    let scaled = number / unit.threshold
    let rounded = scaled >= 100
        ? scaled.rounded(.toNearestOrAwayFromZero)
        : (scaled * 10).rounded(.toNearestOrAwayFromZero) / 10
    let formatted = rounded >= 100
        ? String(format: "%.0f", rounded)
        : String(format: "%.1f", rounded).replacingOccurrences(of: ".0", with: "")
    return formatted + unit.suffix
}
