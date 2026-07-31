import UIKit

enum ImageLoaderError: Error, LocalizedError, Sendable {
    case invalidImage

    var errorDescription: String? {
        "Luna could not display this image."
    }
}

actor AuthenticatedImageLoader {
    private let client: APIClient
    private let cache = NSCache<NSString, UIImage>()

    init(client: APIClient) {
        self.client = client
        cache.countLimit = 120
        cache.totalCostLimit = 64 * 1024 * 1024
    }

    func image(at path: String) async throws -> UIImage {
        let key = path as NSString
        if let image = cache.object(forKey: key) {
            return image
        }
        let data = try await client.data(at: path)
        guard let image = UIImage(data: data) else {
            throw ImageLoaderError.invalidImage
        }
        cache.setObject(image, forKey: key, cost: data.count)
        return image
    }

    func removeAll() {
        cache.removeAllObjects()
    }
}
