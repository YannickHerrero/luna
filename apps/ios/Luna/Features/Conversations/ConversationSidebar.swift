import SwiftUI

struct ConversationSidebar: View {
    let conversations: [Conversation]
    let selectedConversationId: UUID?
    let imageLoader: AuthenticatedImageLoader
    let listScope: ConversationListScope
    let onSelect: (UUID) -> Void
    let onCreate: () -> Void
    let onShowActive: () -> Void
    let onShowArchived: () -> Void

    @Binding var search: String
    @Binding var groupByProject: Bool
    @AppStorage("luna-theme") private var storedTheme = LunaTheme.latte.rawValue
    @Environment(\.horizontalSizeClass) private var horizontalSizeClass
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    @Environment(\.lunaPalette) private var palette

    private var filteredConversations: [Conversation] {
        guard !search.isEmpty else { return conversations }
        return conversations.filter {
            $0.title.localizedCaseInsensitiveContains(search)
        }
    }

    private var projectSections: [ConversationProjectSection] {
        conversationProjectSections(filteredConversations)
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            searchField
            listControls
            conversationList
            footer
        }
        .background(palette.surface.opacity(0.94))
        .accessibilityIdentifier("conversation-sidebar")
    }

    private var header: some View {
        HStack(alignment: .center) {
            VStack(alignment: .leading, spacing: 1) {
                Text("PERSISTENT PI")
                    .lunaMonoFont(10, weight: .bold)
                    .tracking(1.3)
                    .foregroundStyle(palette.accent)
                Text("Luna")
                    .font(LunaFont.display(30, weight: .bold))
                    .tracking(-1.2)
                    .foregroundStyle(palette.foreground)
            }
            Spacer()
            LunaIconButton(
                icon: .plus,
                accessibilityLabel: "New conversation",
                isAccent: true,
                action: onCreate
            )
            .accessibilityIdentifier("new-conversation")
        }
        .padding(.horizontal, 22)
        .padding(.top, horizontalSizeClass == .compact ? 20 : 25)
        .padding(.bottom, 16)
    }

    private var searchField: some View {
        HStack(spacing: 8) {
            LunaIconView(icon: .search, size: 15)
            TextField(
                dynamicTypeSize.isAccessibilitySize ? "Search" : "Search conversations",
                text: $search
            )
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .font(LunaFont.body(horizontalSizeClass == .compact ? 16 : 13))
                .foregroundStyle(palette.foreground)
                .accessibilityLabel("Search conversations")
                .accessibilityIdentifier("conversation-search")
        }
        .foregroundStyle(palette.muted)
        .padding(.horizontal, 13)
        .frame(minHeight: 38)
        .background(palette.background)
        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .stroke(palette.border, lineWidth: 1)
        }
        .padding(.horizontal, 15)
        .padding(.bottom, 12)
    }

    private var listControls: some View {
        HStack(spacing: 6) {
            Button(groupByProject ? "Grouped by project" : "Group by project") {
                groupByProject.toggle()
            }
            .accessibilityValue(groupByProject ? "On" : "Off")
            .accessibilityIdentifier("group-conversations")
            Spacer(minLength: 4)
            Button(listScope == .active ? "Archived" : "Active") {
                if listScope == .active { onShowArchived() } else { onShowActive() }
            }
            .accessibilityIdentifier("conversation-list-scope")
        }
        .buttonStyle(ConversationListControlStyle())
        .padding(.horizontal, 15)
        .padding(.bottom, 8)
    }

    private var conversationList: some View {
        ScrollView {
            LazyVStack(spacing: 0) {
                if groupByProject {
                    ForEach(projectSections) { section in
                        ProjectHeader(
                            repository: section.repository,
                            count: section.conversations.count,
                            imageLoader: imageLoader
                        )
                        ForEach(section.conversations) { conversation in
                            conversationCell(conversation)
                        }
                    }
                } else {
                    ForEach(filteredConversations) { conversation in
                        conversationCell(conversation)
                    }
                }
                if filteredConversations.isEmpty {
                    VStack(spacing: 8) {
                        Text(emptyListMessage)
                            .foregroundStyle(palette.muted)
                        Button(listScope == .active ? "Start one" : "Show active") {
                            if listScope == .active { onCreate() } else { onShowActive() }
                        }
                        .fontWeight(.bold)
                        .foregroundStyle(palette.accent)
                    }
                    .font(LunaFont.body(13))
                    .padding(.vertical, 60)
                }
            }
            .padding(.horizontal, 9)
            .padding(.bottom, 14)
        }
        .frame(maxHeight: .infinity)
    }

    private var emptyListMessage: String {
        if !search.isEmpty { return "No matching conversations." }
        return listScope == .active ? "No conversations yet." : "No archived conversations."
    }

    private func conversationCell(_ conversation: Conversation) -> some View {
        ConversationCell(
            conversation: conversation,
            isSelected: conversation.id == selectedConversationId,
            imageLoader: imageLoader,
            onSelect: { onSelect(conversation.id) }
        )
    }

    private var footer: some View {
        HStack {
            Text(currentTheme.displayName)
                .lunaMonoFont(10)
            Spacer()
            LunaIconButton(
                icon: currentTheme == .latte ? .moon : .sun,
                accessibilityLabel: "Toggle theme",
                action: toggleTheme
            )
            .frame(width: 38, height: 38)
        }
        .foregroundStyle(palette.muted)
        .padding(.leading, 16)
        .padding(.trailing, 8)
        .padding(.vertical, 6)
        .overlay(alignment: .top) {
            Rectangle()
                .fill(palette.border)
                .frame(height: 1)
        }
    }

    private var currentTheme: LunaTheme {
        LunaTheme(rawValue: storedTheme) ?? .latte
    }

    private func toggleTheme() {
        storedTheme = (currentTheme == .latte ? LunaTheme.mocha : .latte).rawValue
    }
}

private struct ProjectHeader: View {
    let repository: Repository?
    let count: Int
    let imageLoader: AuthenticatedImageLoader

    @Environment(\.lunaPalette) private var palette

    var body: some View {
        HStack(spacing: 7) {
            Group {
                if let path = repository?.icon.contentUrl {
                    AuthenticatedImageView(path: path, loader: imageLoader) {
                        fallbackIcon
                    }
                    .scaledToFill()
                } else {
                    fallbackIcon
                }
            }
            .frame(width: 22, height: 22)
            .background(palette.surface)
            .clipShape(RoundedRectangle(cornerRadius: 7, style: .continuous))
            .overlay {
                RoundedRectangle(cornerRadius: 7, style: .continuous)
                    .stroke(palette.border, lineWidth: 1)
            }
            Text(repository?.displayName ?? "No Project")
                .lineLimit(1)
            Spacer(minLength: 8)
            Text("\(count)")
        }
        .lunaMonoFont(10, weight: .bold)
        .foregroundStyle(palette.muted)
        .padding(.horizontal, 12)
        .padding(.top, 9)
        .padding(.bottom, 5)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(repository?.displayName ?? "No Project"), \(count) conversations")
        .accessibilityIdentifier("project-\(repository?.id.uuidString ?? "none")")
    }

    private var fallbackIcon: some View {
        Text(repository?.icon.fallbackText ?? "☾")
            .font(LunaFont.display(10, weight: .bold))
            .foregroundStyle(palette.accent)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

private struct ConversationListControlStyle: ButtonStyle {
    @Environment(\.lunaPalette) private var palette

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .lunaMonoFont(9)
            .foregroundStyle(palette.muted)
            .padding(.horizontal, 9)
            .frame(minHeight: 30)
            .background(configuration.isPressed ? palette.raised : palette.surface)
            .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
            .overlay {
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .stroke(configuration.isPressed ? palette.accent : palette.border, lineWidth: 1)
            }
    }
}

private struct ConversationCell: View {
    let conversation: Conversation
    let isSelected: Bool
    let imageLoader: AuthenticatedImageLoader
    let onSelect: () -> Void

    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    @Environment(\.lunaPalette) private var palette

    var body: some View {
        Button(action: onSelect) {
            Group {
                if dynamicTypeSize.isAccessibilitySize {
                    accessibleContent
                } else {
                    standardContent
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 11)
            .background(isSelected ? palette.raised : .clear)
            .clipShape(RoundedRectangle(cornerRadius: 17, style: .continuous))
            .overlay(alignment: .leading) {
                if isSelected {
                    Capsule()
                        .fill(palette.accent)
                        .frame(width: 3)
                }
            }
            .contentShape(RoundedRectangle(cornerRadius: 17, style: .continuous))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(
            "\(conversation.title), \(stateLabel(conversation.state)), \(conversation.preview)"
        )
        .accessibilityIdentifier("conversation-\(conversation.id.uuidString)")
    }

    private var standardContent: some View {
        HStack(spacing: 11) {
            avatar
            VStack(alignment: .leading, spacing: 4) {
                titleText.lineLimit(1)
                previewText.lineLimit(1)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            timestampText.lineLimit(1)
            stateDot
        }
    }

    private var accessibleContent: some View {
        HStack(alignment: .top, spacing: 11) {
            avatar
            VStack(alignment: .leading, spacing: 7) {
                titleText
                previewText
                HStack(spacing: 8) {
                    timestampText
                    Spacer(minLength: 8)
                    stateDot
                }
            }
            .fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private var titleText: some View {
        Text(conversation.title)
            .font(LunaFont.body(13, weight: .bold))
            .foregroundStyle(palette.foreground)
    }

    private var previewText: some View {
        Text(conversation.preview.isEmpty ? stateLabel(conversation.state) : conversation.preview)
            .font(LunaFont.body(11))
            .foregroundStyle(palette.muted)
    }

    private var timestampText: some View {
        Text(
            LunaTime.conversationTimestamp(
                conversation.lastMessageAt ?? conversation.createdAt
            )
        )
        .lunaMonoFont(9)
        .foregroundStyle(palette.muted)
    }

    private var avatar: some View {
        Group {
            if let path = conversation.repositories.first?.icon.contentUrl {
                AuthenticatedImageView(path: path, loader: imageLoader) {
                    fallbackAvatar
                }
                .scaledToFill()
            } else {
                fallbackAvatar
            }
        }
        .frame(width: 42, height: 42)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .stroke(palette.border, lineWidth: 1)
        }
    }

    private var fallbackAvatar: some View {
        Text(conversation.repositories.first?.icon.fallbackText ?? "☾")
            .font(LunaFont.display(15, weight: .bold))
            .foregroundStyle(palette.accent)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var stateDot: some View {
        Circle()
            .fill(stateColor)
            .frame(width: 7, height: 7)
            .shadow(
                color: conversation.state.isBusy ? stateColor.opacity(0.35) : .clear,
                radius: 4
            )
            .accessibilityHidden(true)
    }

    private var stateColor: Color {
        if conversation.state.isBusy { return palette.green }
        if conversation.state == .crashed || conversation.state == .error { return palette.red }
        return palette.overlay
    }
}

func stateLabel(_ state: SessionState) -> String {
    switch state {
    case .creating: "Creating"
    case .starting: "Starting"
    case .idle: "Ready"
    case .working: "Working"
    case .compacting: "Compacting"
    case .retrying: "Retrying"
    case .crashed: "Needs restore"
    case .restoring: "Restoring"
    case .interrupted: "Interrupted"
    case .stopped: "Stopped"
    case .error: "Needs attention"
    }
}
