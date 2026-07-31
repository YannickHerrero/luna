import SwiftUI

struct AuthenticatedImageView<Placeholder: View>: View {
    let path: String
    let loader: AuthenticatedImageLoader
    @ViewBuilder let placeholder: () -> Placeholder

    @State private var image: UIImage?

    var body: some View {
        Group {
            if let image {
                Image(uiImage: image)
                    .resizable()
            } else {
                placeholder()
            }
        }
        .task(id: path) {
            image = try? await loader.image(at: path)
        }
    }
}
