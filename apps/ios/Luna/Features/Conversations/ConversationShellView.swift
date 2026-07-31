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
                    conversation: conversation,
                    messages: store.selectedMessages,
                    compact: true,
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
                        conversation: conversation,
                        messages: store.selectedMessages,
                        compact: false,
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
    let conversation: Conversation
    let messages: [Message]
    let compact: Bool
    let onBack: () -> Void

    @Environment(\.lunaPalette) private var palette

    var body: some View {
        VStack(spacing: 0) {
            header
            Group {
                if messages.isEmpty {
                    conversationEmpty
                } else {
                    ScrollView {
                        LazyVStack(alignment: .leading, spacing: 12) {
                            ForEach(messages) { message in
                                Text(message.text)
                                    .font(LunaFont.body(14))
                                    .foregroundStyle(palette.foreground)
                                    .frame(maxWidth: .infinity, alignment: message.role == .user ? .trailing : .leading)
                            }
                        }
                        .padding(24)
                    }
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            composerPlaceholder
        }
        .background(palette.surface.opacity(0.94))
        .accessibilityIdentifier("conversation-panel")
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
            statusPill
            LunaIconButton(
                icon: .archive,
                accessibilityLabel: "Archive conversation",
                action: {}
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

    private var conversationEmpty: some View {
        VStack(spacing: 0) {
            LunaMoonMark()
            Text("What should we work on?")
                .font(LunaFont.display(compact ? 28 : 42, weight: .bold))
                .tracking(-1.2)
                .foregroundStyle(palette.foreground)
                .padding(.top, 13)
            Text("Luna starts every conversation at your home directory and follows Pi across repositories.")
                .font(LunaFont.body(14))
                .foregroundStyle(palette.muted)
                .multilineTextAlignment(.center)
                .lineSpacing(5)
                .frame(maxWidth: 520)
                .padding(.top, 8)
        }
        .padding(30)
    }

    private var composerPlaceholder: some View {
        HStack(spacing: 2) {
            LunaIconView(icon: .settings, size: 18)
                .frame(width: 38, height: 38)
            LunaIconView(icon: .paperclip, size: 18)
                .frame(width: 38, height: 38)
            Text("Message Luna…")
                .font(LunaFont.body(compact ? 16 : 14))
                .foregroundStyle(palette.muted)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 9)
            LunaIconView(icon: .mic, size: 18)
                .frame(width: 38, height: 38)
            LunaIconView(icon: .send, size: 17)
                .foregroundStyle(.white)
                .frame(width: 38, height: 38)
                .background(palette.accent)
                .clipShape(Circle())
        }
        .foregroundStyle(palette.muted)
        .padding(8)
        .frame(minHeight: 54)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: 22, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: 22, style: .continuous)
                .stroke(palette.border, lineWidth: 1)
        }
        .shadow(color: palette.foreground.opacity(0.08), radius: 20, y: 16)
        .padding(.horizontal, compact ? 16 : 64)
        .padding(.top, 8)
        .padding(.bottom, 12)
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
