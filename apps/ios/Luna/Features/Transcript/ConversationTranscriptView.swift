import SwiftUI

struct ConversationTranscriptView: View {
    let conversation: Conversation
    let messages: [Message]
    let imageLoader: AuthenticatedImageLoader
    let canLoadEarlier: Bool
    let onLoadEarlier: () async -> Void

    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.lunaPalette) private var palette
    @State private var positionedInitialMessages = false
    @State private var contentHeight: CGFloat = 0
    @State private var viewportHeight: CGFloat = 0

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    if canLoadEarlier {
                        Button("Load earlier messages") {
                            Task { await onLoadEarlier() }
                        }
                        .buttonStyle(LoadEarlierButtonStyle())
                        .frame(maxWidth: .infinity)
                        .padding(.bottom, 22)
                    }
                    if messages.isEmpty {
                        emptyState
                    } else {
                        ForEach(messages) { message in
                            MessageBubbleView(message: message, imageLoader: imageLoader)
                        }
                    }
                    if let taskList = conversation.taskList {
                        TaskListProgressView(taskList: taskList)
                    }
                    if conversation.state.isBusy {
                        TypingIndicatorView(activities: conversation.activities)
                    }
                    Color.clear
                        .frame(height: 1)
                        .id("transcript-bottom")
                }
                .padding(.horizontal, 24)
                .padding(.top, 30)
                .padding(.bottom, 24)
                .onGeometryChange(for: CGFloat.self) { geometry in
                    geometry.size.height
                } action: { height in
                    contentHeight = height
                }
            }
            .onGeometryChange(for: CGFloat.self) { geometry in
                geometry.size.height
            } action: { height in
                viewportHeight = height
            }
            .onAppear { scrollToBottom(proxy) }
            .onChange(of: contentHeight) { _, _ in scrollToBottom(proxy) }
            .onChange(of: messages) { _, _ in scrollToBottom(proxy) }
            .onChange(of: conversation.activities) { _, _ in scrollToBottom(proxy) }
            .onChange(of: conversation.taskList) { _, _ in scrollToBottom(proxy) }
        }
        .accessibilityIdentifier("message-transcript")
    }

    private var emptyState: some View {
        VStack(spacing: 0) {
            LunaMoonMark()
            Text("What should we work on?")
                .font(LunaFont.display(36, weight: .bold))
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
        .frame(maxWidth: .infinity, minHeight: 400)
    }

    private func scrollToBottom(_ proxy: ScrollViewProxy) {
        guard contentHeight > viewportHeight, viewportHeight > 0 else { return }
        let animated = positionedInitialMessages
            && !reduceMotion
            && !ProcessInfo.processInfo.arguments.contains { $0.hasPrefix("-ui-testing") }
        positionedInitialMessages = true
        if animated {
            withAnimation(.easeOut(duration: 0.22)) {
                proxy.scrollTo("transcript-bottom", anchor: .bottom)
            }
        } else {
            proxy.scrollTo("transcript-bottom", anchor: .bottom)
        }
    }
}

private struct MessageBubbleView: View {
    let message: Message
    let imageLoader: AuthenticatedImageLoader

    @Environment(\.lunaPalette) private var palette
    @State private var showsTimestamp = false

    var body: some View {
        HStack {
            if message.role == .user { Spacer(minLength: 45) }
            VStack(alignment: message.role == .user ? .trailing : .leading, spacing: 3) {
                bubble
                if showsTimestamp {
                    Text(LunaTime.messageTimestamp(message.createdAt))
                        .font(LunaFont.mono(9))
                        .foregroundStyle(palette.muted)
                        .padding(.horizontal, 6)
                        .transition(.opacity.combined(with: .move(edge: .top)))
                }
            }
            .frame(maxWidth: 760, alignment: message.role == .user ? .trailing : .leading)
            if message.role == .assistant { Spacer(minLength: 45) }
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 9)
        .contentShape(Rectangle())
        .onTapGesture {
            withAnimation(.easeOut(duration: 0.14)) {
                showsTimestamp.toggle()
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel(
            "\(message.role == .user ? "You" : "Luna"): \(message.text)"
        )
        .accessibilityHint(
            showsTimestamp
                ? "Sent \(LunaTime.messageTimestamp(message.createdAt)). Double tap to hide the timestamp."
                : "Double tap to show the sent date and time."
        )
        .accessibilityAddTraits(.isButton)
        .accessibilityAction { showsTimestamp.toggle() }
    }

    private var bubble: some View {
        VStack(alignment: .leading, spacing: 8) {
            if !message.attachments.isEmpty {
                attachmentGrid
            }
            if message.role == .assistant {
                HStack(alignment: .lastTextBaseline, spacing: 3) {
                    MarkdownView(message.text)
                    if message.status == .streaming {
                        StreamCaret()
                    }
                }
            } else {
                Text(message.text)
                    .font(LunaFont.body(14))
                    .foregroundStyle(palette.onAccent)
                    .lineSpacing(5)
                    .textSelection(.enabled)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(message.role == .user ? EdgeInsets(top: 10, leading: 15, bottom: 10, trailing: 15) : EdgeInsets(top: 5, leading: 0, bottom: 5, trailing: 0))
        .background(message.role == .user ? palette.accent : .clear)
        .clipShape(
            UnevenRoundedRectangle(
                topLeadingRadius: 19,
                bottomLeadingRadius: 19,
                bottomTrailingRadius: message.role == .user ? 6 : 19,
                topTrailingRadius: 19,
                style: .continuous
            )
        )
        .shadow(
            color: message.role == .user ? palette.accent.opacity(0.16) : .clear,
            radius: 25,
            y: 8
        )
    }

    private var attachmentGrid: some View {
        LazyVGrid(
            columns: [GridItem(.flexible(), spacing: 6), GridItem(.flexible(), spacing: 6)],
            spacing: 6
        ) {
            ForEach(message.attachments) { attachment in
                AuthenticatedImageView(path: attachment.contentUrl, loader: imageLoader) {
                    attachmentPlaceholder(attachment)
                }
                .scaledToFill()
                .frame(minWidth: 110, maxWidth: 240, minHeight: 110, maxHeight: 220)
                .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
                .accessibilityLabel(attachment.fileName)
            }
        }
        .frame(maxWidth: 486)
    }

    private func attachmentPlaceholder(_ attachment: Attachment) -> some View {
        VStack(spacing: 7) {
            LunaIconView(icon: .paperclip, size: 22)
            Text(attachment.fileName)
                .font(LunaFont.body(10))
                .lineLimit(1)
        }
        .foregroundStyle(message.role == .user ? palette.onAccent : palette.muted)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(palette.raised.opacity(message.role == .user ? 0.18 : 1))
    }
}

private struct StreamCaret: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.lunaPalette) private var palette
    @State private var visible = true

    private var animateCaret: Bool {
        !reduceMotion && !ProcessInfo.processInfo.arguments.contains { $0.hasPrefix("-ui-testing") }
    }

    var body: some View {
        Rectangle()
            .fill(palette.accent)
            .frame(width: 7, height: 16)
            .opacity(visible ? 1 : 0.12)
            .onAppear {
                guard animateCaret else { return }
                withAnimation(.linear(duration: 0.4).repeatForever(autoreverses: true)) {
                    visible = false
                }
            }
            .accessibilityHidden(true)
    }
}

private struct TypingIndicatorView: View {
    let activities: [AgentActivity]

    @Environment(\.lunaPalette) private var palette
    @State private var expanded = false

    private var latest: AgentActivity? { activities.last }

    var body: some View {
        HStack(alignment: .top, spacing: 9) {
            TypingDots()
            if let latest {
                if activities.count > 1 {
                    VStack(alignment: .leading, spacing: 8) {
                        Button {
                            withAnimation(.easeOut(duration: 0.16)) { expanded.toggle() }
                        } label: {
                            HStack(spacing: 6) {
                                Text(latest.summary)
                                    .lineLimit(1)
                                LunaIconView(icon: .chevronDown, size: 14)
                                    .rotationEffect(.degrees(expanded ? 180 : 0))
                            }
                            .foregroundStyle(palette.foreground)
                        }
                        .buttonStyle(.plain)
                        if expanded {
                            VStack(alignment: .leading, spacing: 5) {
                                ForEach(Array(activities.enumerated()), id: \.element.id) { index, activity in
                                    Text("\(index + 1). \(activity.summary)")
                                }
                            }
                            .foregroundStyle(palette.muted)
                            .padding(.leading, 2)
                        }
                    }
                } else {
                    Text(latest.summary)
                        .foregroundStyle(palette.foreground)
                        .lineLimit(1)
                }
            }
        }
        .font(LunaFont.body(12))
        .lineSpacing(4)
        .padding(.horizontal, 14)
        .padding(.vertical, 11)
        .background(palette.raised)
        .clipShape(
            UnevenRoundedRectangle(
                topLeadingRadius: 18,
                bottomLeadingRadius: 6,
                bottomTrailingRadius: 18,
                topTrailingRadius: 18,
                style: .continuous
            )
        )
        .overlay {
            UnevenRoundedRectangle(
                topLeadingRadius: 18,
                bottomLeadingRadius: 6,
                bottomTrailingRadius: 18,
                topTrailingRadius: 18,
                style: .continuous
            )
            .stroke(palette.border, lineWidth: 1)
        }
        .padding(.vertical, 9)
        .accessibilityElement(children: .combine)
        .accessibilityLabel(
            latest.map { "Pi is working. \($0.summary)" } ?? "Pi is working"
        )
    }
}

private struct TypingDots: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.lunaPalette) private var palette
    @State private var animate = false

    private var animateDots: Bool {
        !reduceMotion && !ProcessInfo.processInfo.arguments.contains { $0.hasPrefix("-ui-testing") }
    }

    var body: some View {
        HStack(spacing: 4) {
            ForEach(0..<3) { index in
                Circle()
                    .fill(palette.muted)
                    .frame(width: 6, height: 6)
                    .offset(y: animate ? -2 : 2)
                    .animation(
                        !animateDots
                            ? nil
                            : .easeInOut(duration: 0.6)
                                .repeatForever(autoreverses: true)
                                .delay(Double(index) * 0.15),
                        value: animate
                    )
            }
        }
        .frame(minHeight: 17)
        .onAppear { animate = animateDots }
        .accessibilityHidden(true)
    }
}

private struct TaskListProgressView: View {
    let taskList: AgentTaskList

    @Environment(\.lunaPalette) private var palette
    @State private var expanded = false

    private var completed: Int { taskList.tasks.count { $0.status == .completed } }
    private var skipped: Int { taskList.tasks.count { $0.status == .skipped } }
    private var resolved: Int { completed + skipped }
    private var finished: Bool { resolved == taskList.tasks.count }
    private var current: AgentTask? {
        taskList.tasks.first { $0.status == .inProgress }
            ?? taskList.tasks.first { $0.status == .blocked }
            ?? taskList.tasks.first { $0.status == .pending }
    }

    var body: some View {
        VStack(spacing: 0) {
            Button {
                withAnimation(.easeOut(duration: 0.16)) { expanded.toggle() }
            } label: {
                HStack(spacing: 10) {
                    LunaIconView(icon: .listChecks, size: 17)
                        .foregroundStyle(finished ? palette.green : palette.accent)
                        .frame(width: 30, height: 30)
                        .background(
                            (finished ? palette.green : palette.accent).opacity(0.13)
                        )
                        .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                    VStack(alignment: .leading, spacing: 3) {
                        Text(taskList.title ?? "Plan progress")
                            .font(LunaFont.body(12, weight: .bold))
                            .foregroundStyle(palette.foreground)
                            .lineLimit(1)
                        Text(progressDescription)
                            .font(LunaFont.body(10))
                            .foregroundStyle(palette.muted)
                            .lineLimit(1)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    Text("\(resolved)/\(taskList.tasks.count)")
                        .font(LunaFont.mono(10))
                        .foregroundStyle(palette.muted)
                    LunaIconView(icon: .chevronDown, size: 15)
                        .foregroundStyle(palette.muted)
                        .rotationEffect(.degrees(expanded ? 180 : 0))
                }
                .padding(.horizontal, 14)
                .padding(.vertical, 12)
            }
            .buttonStyle(.plain)

            ProgressView(
                value: Double(resolved),
                total: Double(max(taskList.tasks.count, 1))
            )
            .progressViewStyle(.linear)
            .tint(finished ? palette.green : palette.accent)
            .padding(.horizontal, 14)
            .accessibilityLabel(progressAccessibilityLabel)

            if expanded {
                VStack(alignment: .leading, spacing: 8) {
                    ForEach(taskList.tasks) { task in
                        taskRow(task)
                    }
                }
                .padding(.horizontal, 14)
                .padding(.top, 10)
                .padding(.bottom, 13)
            } else {
                Color.clear.frame(height: 12)
            }
        }
        .frame(maxWidth: 760)
        .background(palette.raised)
        .clipShape(RoundedRectangle(cornerRadius: 18, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: 18, style: .continuous)
                .stroke(palette.border, lineWidth: 1)
        }
        .padding(.top, 18)
        .padding(.bottom, 9)
        .accessibilityIdentifier("task-progress")
    }

    private var progressDescription: String {
        if finished { return skipped > 0 ? "Plan finished" : "Plan complete" }
        return current?.text ?? "Reviewing remaining work"
    }

    private var progressAccessibilityLabel: String {
        "\(completed) of \(taskList.tasks.count) tasks completed"
            + (skipped > 0 ? ", \(skipped) skipped" : "")
    }

    private func taskRow(_ task: AgentTask) -> some View {
        HStack(alignment: .top, spacing: 7) {
            taskIcon(task.status)
                .frame(width: 18, height: 16)
            VStack(alignment: .leading, spacing: 2) {
                Text(task.text)
                    .font(LunaFont.body(11, weight: .semibold))
                    .foregroundStyle(taskColor(task.status))
                    .strikethrough(task.status == .completed || task.status == .skipped)
                if let note = task.note {
                    Text(note)
                        .font(LunaFont.body(9))
                        .foregroundStyle(palette.muted)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(task.text), \(taskStatusLabel(task.status))")
    }

    @ViewBuilder
    private func taskIcon(_ status: AgentTaskStatus) -> some View {
        switch status {
        case .completed:
            LunaIconView(icon: .check, size: 14).foregroundStyle(palette.green)
        case .blocked:
            LunaIconView(icon: .triangleAlert, size: 14).foregroundStyle(palette.red)
        case .skipped:
            LunaIconView(icon: .minus, size: 14).foregroundStyle(palette.muted)
        case .inProgress:
            LunaIconView(icon: .circle, size: 12).foregroundStyle(palette.accent)
        case .pending:
            LunaIconView(icon: .circle, size: 12).foregroundStyle(palette.muted)
        }
    }

    private func taskColor(_ status: AgentTaskStatus) -> Color {
        switch status {
        case .blocked: palette.red
        case .completed, .skipped: palette.muted
        default: palette.foreground
        }
    }

    private func taskStatusLabel(_ status: AgentTaskStatus) -> String {
        switch status {
        case .pending: "Pending"
        case .inProgress: "In progress"
        case .completed: "Completed"
        case .blocked: "Blocked"
        case .skipped: "Skipped"
        }
    }
}

private struct LoadEarlierButtonStyle: ButtonStyle {
    @Environment(\.lunaPalette) private var palette

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(LunaFont.body(11, weight: .bold))
            .foregroundStyle(palette.muted)
            .padding(.horizontal, 13)
            .padding(.vertical, 8)
            .background(palette.raised)
            .clipShape(Capsule())
            .overlay { Capsule().stroke(palette.border, lineWidth: 1) }
            .opacity(configuration.isPressed ? 0.72 : 1)
    }
}
