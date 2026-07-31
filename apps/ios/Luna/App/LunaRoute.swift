import Foundation

enum LunaRoute: Equatable, Sendable {
    static let scheme = "luna"

    case home
    case conversation(UUID)

    init?(url: URL) {
        guard url.scheme?.lowercased() == Self.scheme,
              url.query == nil,
              url.fragment == nil
        else {
            return nil
        }

        switch url.host?.lowercased() {
        case "home":
            guard url.path.isEmpty || url.path == "/" else { return nil }
            self = .home
        case "conversation":
            let components = url.pathComponents.filter { $0 != "/" }
            guard components.count == 1,
                  let id = UUID(uuidString: components[0])
            else {
                return nil
            }
            self = .conversation(id)
        default:
            return nil
        }
    }

    var url: URL {
        switch self {
        case .home:
            URL(string: "\(Self.scheme)://home")!
        case let .conversation(id):
            URL(string: "\(Self.scheme)://conversation/\(id.uuidString)")!
        }
    }
}
