import SwiftUI

struct ConversationShellView: View {
    @Bindable var store: ConversationStore

    @Environment(\.horizontalSizeClass) private var horizontalSizeClass
    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.lunaPalette) private var palette
    @State private var search = ""

    var body: some View {
        GeometryReader { geometry in
            ZStack {
                LunaBackground()
                if horizontalSizeClass == .compact {
                    compactLayout
                } else {
                    regularLayout(width: geometry.size.width)
                }
            }
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
        .overlay(alignment: .bottomTrailing) {
            if let error = store.errorMessage {
                errorToast(error)
            }
        }
    }

    private var compactLayout: some View {
        Group {
            if let conversation = store.selectedConversation {
                ConversationPanel(
                    store: store,
                    conversation: conversation,
                    messages: store.selectedMessages,
                    imageLoader: store.imageLoader,
                    canLoadEarlier: store.canLoadEarlier,
                    compact: true,
                    onLoadEarlier: { await store.loadEarlierMessages() },
                    onBack: store.showConversationList
                )
            } else {
                sidebar
            }
        }
        .background(palette.surface)
    }

    private func regularLayout(width: CGFloat) -> some View {
        let sidebarWidth = min(340, max(290, width * 0.24))
        return HStack(spacing: 14) {
            sidebar
                .frame(width: sidebarWidth)
                .lunaCard(cornerRadius: 28)
            Group {
                if let conversation = store.selectedConversation {
                    ConversationPanel(
                        store: store,
                        conversation: conversation,
                        messages: store.selectedMessages,
                        imageLoader: store.imageLoader,
                        canLoadEarlier: store.canLoadEarlier,
                        compact: false,
                        onLoadEarlier: { await store.loadEarlierMessages() },
                        onBack: store.showConversationList
                    )
                } else {
                    LunaWelcomeView {
                        Task { await store.createConversation() }
                    }
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(palette.surface.opacity(0.94))
            .lunaCard(cornerRadius: 28)
        }
        .padding(14)
    }

    private var sidebar: some View {
        ConversationSidebar(
            conversations: store.conversations,
            selectedConversationId: store.selectedConversationId,
            imageLoader: store.imageLoader,
            onSelect: { id in
                Task { await store.selectConversation(id) }
            },
            onCreate: {
                Task { await store.createConversation() }
            },
            search: $search
        )
    }

    private func errorToast(_ message: String) -> some View {
        Button {
            store.errorMessage = nil
        } label: {
            HStack(spacing: 13) {
                Text(message)
                    .multilineTextAlignment(.leading)
                LunaIconView(icon: .x, size: 15)
            }
            .font(LunaFont.body(12))
            .foregroundStyle(palette.foreground)
            .padding(.horizontal, 14)
            .padding(.vertical, 12)
            .background(palette.surface)
            .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
            .overlay {
                RoundedRectangle(cornerRadius: 14, style: .continuous)
                    .stroke(palette.red.opacity(0.4), lineWidth: 1)
            }
            .shadow(color: palette.red.opacity(0.18), radius: 28, y: 18)
        }
        .buttonStyle(.plain)
        .padding(24)
        .accessibilityLabel("Dismiss error: \(message)")
    }
}

private struct ConversationPanel: View {
    @Bindable var store: ConversationStore
    let conversation: Conversation
    let messages: [Message]
    let imageLoader: AuthenticatedImageLoader
    let canLoadEarlier: Bool
    let compact: Bool
    let onLoadEarlier: () async -> Void
    let onBack: () -> Void

    @Environment(\.lunaPalette) private var palette
    @State private var agentSettingsPresented = false
    @State private var renamePresented = false
    @State private var archivePresented = false
    @State private var renameTitle = ""
    @State private var renaming = false
    @State private var archiving = false

    var body: some View {
        VStack(spacing: 0) {
            header
            ConversationTranscriptView(
                conversation: conversation,
                messages: messages,
                imageLoader: imageLoader,
                canLoadEarlier: canLoadEarlier,
                onLoadEarlier: onLoadEarlier
            )
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            ConversationComposerView(
                store: store,
                conversation: conversation,
                onShowAgentControls: { agentSettingsPresented = true }
            )
            .id(conversation.id)
        }
        .background(palette.surface.opacity(0.94))
        .sheet(isPresented: $agentSettingsPresented) {
            AgentControlsView(
                store: store,
                conversationId: conversation.id,
                busy: conversation.state.isBusy
            )
            .presentationDetents(compact ? [.fraction(0.86)] : [.height(720)])
            .presentationDragIndicator(.hidden)
            .presentationCornerRadius(compact ? 25 : 24)
        }
        .alert("Conversation title", isPresented: $renamePresented) {
            TextField("Conversation title", text: $renameTitle)
                .accessibilityIdentifier("conversation-title-field")
            Button("Cancel", role: .cancel) {}
            Button("Rename") { rename() }
                .disabled(
                    renaming
                        || renameTitle.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                )
        }
        .alert("Archive “\(conversation.title)”?", isPresented: $archivePresented) {
            Button("Cancel", role: .cancel) {}
            Button("Archive", role: .destructive) { archive() }
                .disabled(archiving)
        } message: {
            Text("This conversation will be removed from Luna’s active list.")
        }
    }

    private var header: some View {
        HStack(spacing: 11) {
            if compact {
                LunaIconButton(
                    icon: .arrowLeft,
                    accessibilityLabel: "Back",
                    action: onBack
                )
            }
            Button {
                renameTitle = conversation.title
                renamePresented = true
            } label: {
                VStack(alignment: .leading, spacing: 3) {
                    Text(conversation.title)
                        .font(LunaFont.display(18, weight: .bold))
                        .foregroundStyle(palette.foreground)
                        .lineLimit(1)
                    Text(
                        conversation.repositories.map(\.displayName).joined(separator: " · ").isEmpty
                            ? "Home"
                            : conversation.repositories.map(\.displayName).joined(separator: " · ")
                    )
                    .font(LunaFont.mono(10))
                    .foregroundStyle(palette.muted)
                    .lineLimit(1)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Rename \(conversation.title)")
            .accessibilityHint("Opens a field to change the conversation title")
            .accessibilityIdentifier("rename-conversation")
            statusPill
            LunaIconButton(
                icon: .archive,
                accessibilityLabel: "Archive conversation",
                action: { archivePresented = true }
            )
        }
        .frame(minHeight: compact ? 62 : 72)
        .padding(.horizontal, compact ? 10 : 20)
        .overlay(alignment: .bottom) {
            Rectangle()
                .fill(palette.border)
                .frame(height: 1)
        }
    }

    private func rename() {
        let title = renameTitle.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !title.isEmpty, title != conversation.title, !renaming else { return }
        renaming = true
        store.errorMessage = nil
        Task {
            defer { renaming = false }
            do {
                try await store.renameConversation(conversation.id, title: title)
            } catch {
                store.errorMessage = message(from: error)
            }
        }
    }

    private func archive() {
        guard !archiving else { return }
        archiving = true
        store.errorMessage = nil
        Task {
            defer { archiving = false }
            do {
                try await store.archiveConversation(conversation.id)
            } catch {
                store.errorMessage = message(from: error)
            }
        }
    }

    private func message(from error: Error) -> String {
        (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
    }

    private var statusPill: some View {
        HStack(spacing: 7) {
            Circle()
                .fill(conversation.state.isBusy ? palette.green : palette.overlay)
                .frame(width: 6, height: 6)
            if !compact {
                Text(stateLabel(conversation.state))
            }
        }
        .font(LunaFont.mono(10))
        .foregroundStyle(palette.muted)
        .padding(.horizontal, compact ? 7 : 10)
        .frame(height: 30)
        .overlay {
            Capsule().stroke(palette.border, lineWidth: 1)
        }
        .accessibilityLabel(stateLabel(conversation.state))
    }

}

private struct LunaWelcomeView: View {
    let onCreate: () -> Void
    @Environment(\.lunaPalette) private var palette

    var body: some View {
        VStack(spacing: 0) {
            LunaMoonMark()
            Text("YOUR WORK, IN CONVERSATION")
                .font(LunaFont.mono(10, weight: .bold))
                .tracking(1.3)
                .foregroundStyle(palette.accent)
                .padding(.top, 18)
            Text("Powerful agents.\nFamiliar conversations.")
                .font(LunaFont.display(50, weight: .bold))
                .tracking(-2.2)
                .multilineTextAlignment(.center)
                .foregroundStyle(palette.foreground)
                .padding(.top, 13)
            Text("Continue a Pi session from iPhone, iPad, or the web without losing context.")
                .font(LunaFont.body(16))
                .foregroundStyle(palette.muted)
                .multilineTextAlignment(.center)
                .lineSpacing(5)
                .frame(maxWidth: 520)
                .padding(.top, 12)
            Button(action: onCreate) {
                HStack(spacing: 7) {
                    LunaIconView(icon: .plus, size: 17)
                    Text("New conversation")
                }
            }
            .buttonStyle(LunaPrimaryButtonStyle())
            .padding(.top, 18)
        }
        .padding(30)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}
