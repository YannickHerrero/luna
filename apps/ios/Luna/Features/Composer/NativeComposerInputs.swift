import SwiftUI
import UIKit

struct ComposerTextView: UIViewRepresentable {
    @Binding var text: String
    @Binding var height: CGFloat
    let accessibilityLabel: String
    let onSubmit: () -> Void
    let onPasteImages: ([UIImage]) -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(parent: self)
    }

    func makeUIView(context: Context) -> PasteAwareTextView {
        let view = PasteAwareTextView()
        view.delegate = context.coordinator
        view.backgroundColor = .clear
        view.font = .systemFont(ofSize: 16, weight: .regular)
        view.textColor = UIColor.label
        view.textContainerInset = UIEdgeInsets(top: 8, left: 4, bottom: 8, right: 4)
        view.textContainer.lineFragmentPadding = 0
        view.keyboardDismissMode = .interactive
        view.returnKeyType = .send
        view.isScrollEnabled = false
        view.accessibilityLabel = accessibilityLabel
        view.onSubmit = onSubmit
        view.onPasteImages = onPasteImages
        view.onLayoutWidthChange = { [weak view, weak coordinator = context.coordinator] in
            guard let view else { return }
            coordinator?.resize(view)
        }
        return view
    }

    func updateUIView(_ view: PasteAwareTextView, context: Context) {
        context.coordinator.parent = self
        view.onSubmit = onSubmit
        view.onPasteImages = onPasteImages
        view.accessibilityLabel = accessibilityLabel
        if view.text != text {
            view.text = text
            if view.bounds.width >= 80 {
                context.coordinator.resize(view)
            }
        }
    }

    @MainActor
    final class Coordinator: NSObject, UITextViewDelegate {
        var parent: ComposerTextView

        init(parent: ComposerTextView) {
            self.parent = parent
        }

        func textViewDidChange(_ textView: UITextView) {
            parent.text = textView.text
            resize(textView)
        }

        func resize(_ textView: UITextView) {
            let width = max(textView.bounds.width, 80)
            let fitting = textView.sizeThatFits(
                CGSize(width: width, height: .greatestFiniteMagnitude)
            ).height
            let nextHeight = min(max(ceil(fitting), 36), 176)
            if abs(parent.height - nextHeight) > 0.5 {
                parent.height = nextHeight
            }
            let shouldScroll = fitting > 176
            if textView.isScrollEnabled != shouldScroll {
                textView.isScrollEnabled = shouldScroll
            }
        }

        func textView(
            _ textView: UITextView,
            shouldChangeTextIn range: NSRange,
            replacementText value: String
        ) -> Bool {
            guard value == "\n",
                  let view = textView as? PasteAwareTextView,
                  !view.allowsNewline
            else {
                return true
            }
            parent.onSubmit()
            return false
        }
    }
}

@MainActor
final class PasteAwareTextView: UITextView {
    var onSubmit: () -> Void = {}
    var onPasteImages: ([UIImage]) -> Void = { _ in }
    var onLayoutWidthChange: () -> Void = {}
    var allowsNewline = false
    private var reportedLayoutWidth: CGFloat = 0

    override func layoutSubviews() {
        super.layoutSubviews()
        guard abs(bounds.width - reportedLayoutWidth) > 0.5 else { return }
        reportedLayoutWidth = bounds.width
        onLayoutWidthChange()
    }

    override func paste(_ sender: Any?) {
        let images = UIPasteboard.general.images ?? []
        if images.isEmpty {
            super.paste(sender)
        } else {
            onPasteImages(images)
        }
    }

    override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        if let key = presses.first?.key,
           key.keyCode == .keyboardReturnOrEnter
        {
            if key.modifierFlags.contains(.shift) {
                allowsNewline = true
                insertText("\n")
                allowsNewline = false
            } else {
                onSubmit()
            }
            return
        }
        super.pressesBegan(presses, with: event)
    }
}

struct CameraPicker: UIViewControllerRepresentable {
    let onImage: (UIImage) -> Void
    @Environment(\.dismiss) private var dismiss

    func makeCoordinator() -> Coordinator {
        Coordinator(parent: self)
    }

    func makeUIViewController(context: Context) -> UIImagePickerController {
        let controller = UIImagePickerController()
        controller.sourceType = .camera
        controller.cameraDevice = .rear
        controller.delegate = context.coordinator
        return controller
    }

    func updateUIViewController(_ controller: UIImagePickerController, context: Context) {}

    @MainActor
    final class Coordinator: NSObject, UINavigationControllerDelegate, UIImagePickerControllerDelegate {
        let parent: CameraPicker

        init(parent: CameraPicker) {
            self.parent = parent
        }

        func imagePickerController(
            _ picker: UIImagePickerController,
            didFinishPickingMediaWithInfo info: [UIImagePickerController.InfoKey: Any]
        ) {
            if let image = info[.originalImage] as? UIImage {
                parent.onImage(image)
            }
            parent.dismiss()
        }

        func imagePickerControllerDidCancel(_ picker: UIImagePickerController) {
            parent.dismiss()
        }
    }
}
