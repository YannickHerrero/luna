import Foundation

struct DraftAttachment: Identifiable, Equatable, Sendable {
    let id: UUID
    let data: Data
    let fileName: String
    let mimeType: String

    init(
        id: UUID = UUID(),
        data: Data,
        fileName: String,
        mimeType: String
    ) {
        self.id = id
        self.data = data
        self.fileName = fileName
        self.mimeType = mimeType
    }
}

struct ComposerDraft: Equatable, Sendable {
    var text: String
    var attachments: [DraftAttachment]

    static let empty = ComposerDraft(text: "", attachments: [])
}

struct ComposerDraftPersistence: @unchecked Sendable {
    private let defaults: UserDefaults
    private let prefix: String

    init(defaults: UserDefaults = .standard, prefix: String = "luna-draft:") {
        self.defaults = defaults
        self.prefix = prefix
    }

    func text(for conversationId: UUID) -> String {
        defaults.string(forKey: key(conversationId)) ?? ""
    }

    func setText(_ text: String, for conversationId: UUID) {
        if text.isEmpty {
            defaults.removeObject(forKey: key(conversationId))
        } else {
            defaults.set(text, forKey: key(conversationId))
        }
    }

    private func key(_ conversationId: UUID) -> String {
        "\(prefix)\(conversationId.uuidString)"
    }
}

enum ComposerSubmission: Equatable, Sendable {
    case empty
    case stop
    case message(text: String)
    case invalidShellAttachments

    static func parse(text: String, attachmentCount: Int) -> ComposerSubmission {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty, attachmentCount == 0 { return .empty }
        if trimmed == "/stop" { return .stop }
        if trimmed.hasPrefix("!"), attachmentCount > 0 { return .invalidShellAttachments }
        return .message(
            text: trimmed.isEmpty ? "Please review the attached image." : trimmed
        )
    }
}

enum ComposerError: LocalizedError, Equatable, Sendable {
    case shellAttachments
    case invalidImage
    case imageTooLarge
    case cameraUnavailable
    case microphonePermissionDenied
    case recordingFailed

    var errorDescription: String? {
        switch self {
        case .shellAttachments:
            "Shell commands cannot include attachments."
        case .invalidImage:
            "Only PNG, JPEG, GIF, WebP, HEIC, and HEIF images are supported."
        case .imageTooLarge:
            "Images must be between 1 byte and 20 MB."
        case .cameraUnavailable:
            "The camera is unavailable on this device."
        case .microphonePermissionDenied:
            "Allow microphone access in Settings to transcribe voice."
        case .recordingFailed:
            "Luna could not record audio."
        }
    }
}

func abbreviatedWorkingDirectory(_ path: String) -> String {
    let components = path.split(separator: "/", omittingEmptySubsequences: true)
    guard components.count >= 2, components[0] == "Users" else { return path }
    if components.count == 2 { return "~" }
    return "~/" + components.dropFirst(2).joined(separator: "/")
}
