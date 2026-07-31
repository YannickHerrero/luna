import PhotosUI
import SwiftUI
import UniformTypeIdentifiers
import UIKit

struct ConversationComposerView: View {
    @Bindable var store: ConversationStore
    let conversation: Conversation
    var onShowAgentControls: () -> Void = {}

    @Environment(\.horizontalSizeClass) private var horizontalSizeClass
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    @Environment(\.lunaPalette) private var palette
    @State private var sending = false
    @State private var transcribing = false
    @State private var attachmentOptionsPresented = false
    @State private var photoPickerPresented = false
    @State private var fileImporterPresented = false
    @State private var cameraPresented = false
    @State private var photoItems: [PhotosPickerItem] = []
    @State private var editorHeight: CGFloat = 36
    @State private var voiceRecorder = VoiceRecorder()

    private var draft: ComposerDraft {
        store.composerDraft(for: conversation.id)
    }

    private var text: Binding<String> {
        Binding(
            get: { store.composerDraft(for: conversation.id).text },
            set: { store.setDraftText($0, for: conversation.id) }
        )
    }

    private var busy: Bool { conversation.state.isBusy }

    var body: some View {
        VStack(spacing: 0) {
            attachmentPreviews
            composerBox
            Text(abbreviatedWorkingDirectory(conversation.activeWorkingDirectory))
                .lunaMonoFont(9)
                .foregroundStyle(palette.muted)
                .lineLimit(1)
                .padding(.horizontal, 14)
                .padding(.top, 7)
        }
        .padding(.horizontal, horizontalSizeClass == .compact ? 16 : 64)
        .padding(.top, 8)
        .padding(.bottom, 12)
        .confirmationDialog(
            "Attach image",
            isPresented: $attachmentOptionsPresented,
            titleVisibility: .visible
        ) {
            Button("Photo Library") { photoPickerPresented = true }
            Button("Choose from Files") { fileImporterPresented = true }
            Button("Cancel", role: .cancel) {}
        }
        .photosPicker(
            isPresented: $photoPickerPresented,
            selection: $photoItems,
            maxSelectionCount: max(6 - draft.attachments.count, 1),
            matching: .images
        )
        .onChange(of: photoItems) { _, items in
            guard !items.isEmpty else { return }
            Task { await importPhotoItems(items) }
        }
        .fileImporter(
            isPresented: $fileImporterPresented,
            allowedContentTypes: [.png, .jpeg, .gif, .webP, .heic, .image],
            allowsMultipleSelection: true,
            onCompletion: importFiles
        )
        .fullScreenCover(isPresented: $cameraPresented) {
            CameraPicker { image in
                addImages([image], prefix: "camera")
            }
            .ignoresSafeArea()
        }
        .onDisappear { voiceRecorder.cancel() }
    }

    private var composerBox: some View {
        Group {
            if horizontalSizeClass == .compact || dynamicTypeSize.isAccessibilitySize {
                VStack(spacing: 2) {
                    editor
                    HStack(spacing: 2) {
                        leadingActions
                        Spacer(minLength: 8)
                        trailingActions
                    }
                }
            } else {
                HStack(alignment: .bottom, spacing: 2) {
                    leadingActions
                    editor
                    trailingActions
                }
            }
        }
        .padding(8)
        .frame(minHeight: 54)
        .background(palette.surface)
        .clipShape(RoundedRectangle(cornerRadius: 22, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: 22, style: .continuous)
                .stroke(palette.border, lineWidth: 1)
        }
        .shadow(color: palette.foreground.opacity(0.08), radius: 20, y: 16)
    }

    private var editor: some View {
        ZStack(alignment: .topLeading) {
            Text(busy ? "Steer Pi…" : "Message Luna…")
                .font(LunaFont.body(horizontalSizeClass == .compact ? 16 : 14))
                .foregroundStyle(palette.muted)
                .padding(.leading, 5)
                .padding(.top, 8)
                .opacity(draft.text.isEmpty ? 1 : 0)
                .accessibilityHidden(true)
            ComposerTextView(
                text: text,
                height: $editorHeight,
                accessibilityLabel: busy ? "Steer Pi" : "Message Luna",
                onSubmit: submit,
                onPasteImages: { addImages($0, prefix: "pasted") }
            )
            .frame(minWidth: 80, maxWidth: .infinity)
            .frame(height: editorHeight)
        }
    }

    private var leadingActions: some View {
        HStack(spacing: 2) {
            LunaIconButton(
                icon: .settings,
                accessibilityLabel: "Agent settings",
                action: onShowAgentControls
            )
            LunaIconButton(
                icon: .paperclip,
                accessibilityLabel: "Attach image",
                action: { attachmentOptionsPresented = true }
            )
            LunaIconButton(
                icon: .camera,
                accessibilityLabel: "Take photo",
                action: showCamera
            )
        }
    }

    private var trailingActions: some View {
        HStack(spacing: 2) {
            if busy {
                LunaIconButton(
                    icon: .circleStop,
                    accessibilityLabel: "Interrupt Pi",
                    foreground: palette.red,
                    action: interrupt
                )
            } else {
                Button(action: toggleRecording) {
                    Group {
                        if transcribing {
                            ProgressView().controlSize(.small)
                        } else {
                            LunaIconView(icon: .mic, size: 18)
                        }
                    }
                    .frame(width: LunaShape.minimumTarget, height: LunaShape.minimumTarget)
                    .foregroundStyle(voiceRecorder.isRecording ? Color.white : palette.muted)
                    .accessibilityHidden(true)
                    .background(voiceRecorder.isRecording ? palette.red : .clear)
                    .clipShape(Circle())
                }
                .buttonStyle(.plain)
                .frame(
                    minWidth: LunaShape.minimumTarget,
                    minHeight: LunaShape.minimumTarget
                )
                .contentShape(Circle())
                .disabled(transcribing)
                .accessibilityLabel(
                    voiceRecorder.isRecording ? "Stop recording" : "Transcribe voice"
                )
            }
            Button(action: submit) {
                Group {
                    if sending {
                        ProgressView()
                            .tint(.white)
                            .controlSize(.small)
                    } else {
                        LunaIconView(icon: .send, size: 17)
                    }
                }
                .foregroundStyle(.white)
                .frame(width: LunaShape.minimumTarget, height: LunaShape.minimumTarget)
                .background(palette.accent)
                .accessibilityHidden(true)
                .clipShape(Circle())
                .shadow(color: palette.accent.opacity(0.24), radius: 14, y: 8)
            }
            .buttonStyle(.plain)
            .frame(
                minWidth: LunaShape.minimumTarget,
                minHeight: LunaShape.minimumTarget
            )
            .contentShape(Circle())
            .disabled(sending)
            .opacity(sending ? 0.55 : 1)
            .accessibilityLabel("Send")
            .accessibilityIdentifier("send-message")
        }
    }

    @ViewBuilder
    private var attachmentPreviews: some View {
        if !draft.attachments.isEmpty {
            ScrollView(.horizontal) {
                HStack(spacing: 8) {
                    ForEach(draft.attachments) { attachment in
                        if let image = UIImage(data: attachment.data) {
                            ZStack(alignment: .topTrailing) {
                                Image(uiImage: image)
                                    .resizable()
                                    .scaledToFill()
                                    .frame(width: 68, height: 68)
                                    .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
                                Button {
                                    store.removeDraftAttachment(attachment.id, for: conversation.id)
                                } label: {
                                    LunaIconView(icon: .x, size: 11)
                                        .foregroundStyle(.white)
                                        .frame(width: 20, height: 20)
                                        .background(palette.foreground)
                                        .clipShape(Circle())
                                        .overlay { Circle().stroke(palette.surface, lineWidth: 2) }
                                }
                                .buttonStyle(.plain)
                                .offset(x: 5, y: -5)
                                .accessibilityLabel("Remove \(attachment.fileName)")
                            }
                        }
                    }
                }
                .padding(.horizontal, 8)
                .padding(.top, 5)
                .padding(.bottom, 8)
            }
            .scrollIndicators(.hidden)
        }
    }

    private func submit() {
        guard !sending else { return }
        let current = draft
        sending = true
        store.errorMessage = nil
        Task {
            defer { sending = false }
            do {
                let accepted = try await store.submitMessage(
                    in: conversation.id,
                    text: current.text,
                    attachments: current.attachments
                )
                if accepted { store.clearDraft(for: conversation.id) }
            } catch {
                store.errorMessage = message(from: error)
            }
        }
    }

    private func interrupt() {
        Task {
            do {
                try await store.abortConversation(conversation.id)
            } catch {
                store.errorMessage = message(from: error)
            }
        }
    }

    private func toggleRecording() {
        guard !transcribing else { return }
        Task {
            do {
                if voiceRecorder.isRecording {
                    let data = try voiceRecorder.stop()
                    transcribing = true
                    defer { transcribing = false }
                    let transcription = try await store.transcribe(
                        data,
                        fileName: "recording.m4a",
                        mimeType: "audio/mp4"
                    )
                    let current = draft.text
                    store.setDraftText(
                        current + (current.isEmpty ? "" : " ") + transcription,
                        for: conversation.id
                    )
                } else {
                    try await voiceRecorder.start()
                }
            } catch {
                voiceRecorder.cancel()
                store.errorMessage = message(from: error)
            }
        }
    }

    private func showCamera() {
        guard UIImagePickerController.isSourceTypeAvailable(.camera) else {
            store.errorMessage = ComposerError.cameraUnavailable.localizedDescription
            return
        }
        cameraPresented = true
    }

    private func importPhotoItems(_ items: [PhotosPickerItem]) async {
        defer { photoItems = [] }
        for item in items {
            guard draft.attachments.count < 6 else { break }
            do {
                guard let data = try await item.loadTransferable(type: Data.self),
                      let image = UIImage(data: data)
                else {
                    throw ComposerError.invalidImage
                }
                addImages([image], prefix: "photo")
            } catch {
                store.errorMessage = message(from: error)
            }
        }
    }

    private func importFiles(_ result: Result<[URL], Error>) {
        do {
            let urls = try result.get()
            var attachments: [DraftAttachment] = []
            for url in urls.prefix(max(6 - draft.attachments.count, 0)) {
                let accessed = url.startAccessingSecurityScopedResource()
                defer { if accessed { url.stopAccessingSecurityScopedResource() } }
                let values = try url.resourceValues(forKeys: [.contentTypeKey, .fileSizeKey])
                guard let type = values.contentType, type.conforms(to: .image),
                      let mimeType = type.preferredMIMEType
                else {
                    throw ComposerError.invalidImage
                }
                let data = try Data(contentsOf: url)
                try validateImageData(data)
                guard UIImage(data: data) != nil else { throw ComposerError.invalidImage }
                attachments.append(
                    DraftAttachment(
                        data: data,
                        fileName: url.lastPathComponent,
                        mimeType: mimeType
                    )
                )
            }
            store.addDraftAttachments(attachments, for: conversation.id)
        } catch {
            store.errorMessage = message(from: error)
        }
    }

    private func addImages(_ images: [UIImage], prefix: String) {
        do {
            var attachments: [DraftAttachment] = []
            for image in images.prefix(max(6 - draft.attachments.count, 0)) {
                guard let data = compressedJPEG(image) else { throw ComposerError.imageTooLarge }
                attachments.append(
                    DraftAttachment(
                        data: data,
                        fileName: "\(prefix)-\(UUID().uuidString).jpg",
                        mimeType: "image/jpeg"
                    )
                )
            }
            store.addDraftAttachments(attachments, for: conversation.id)
        } catch {
            store.errorMessage = message(from: error)
        }
    }

    private func compressedJPEG(_ image: UIImage) -> Data? {
        for quality in [0.9, 0.75, 0.6, 0.45] {
            if let data = image.jpegData(compressionQuality: quality),
               !data.isEmpty,
               data.count <= 20 * 1024 * 1024
            {
                return data
            }
        }
        return nil
    }

    private func validateImageData(_ data: Data) throws {
        guard !data.isEmpty, data.count <= 20 * 1024 * 1024 else {
            throw ComposerError.imageTooLarge
        }
    }

    private func message(from error: Error) -> String {
        (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
    }
}
