import Foundation

struct MultipartForm: Sendable {
    private enum Part: Sendable {
        case field(name: String, value: String)
        case file(name: String, fileName: String, mimeType: String, data: Data)
    }

    let boundary: String
    private var parts: [Part] = []

    init(boundary: String = "Luna-\(UUID().uuidString)") {
        self.boundary = boundary
    }

    var contentType: String {
        "multipart/form-data; boundary=\(boundary)"
    }

    var data: Data {
        var result = Data()
        for part in parts {
            result.append("--\(boundary)\r\n")
            switch part {
            case let .field(name, value):
                result.append("Content-Disposition: form-data; name=\"\(escaped(name))\"\r\n\r\n")
                result.append("\(value)\r\n")
            case let .file(name, fileName, mimeType, data):
                result.append(
                    "Content-Disposition: form-data; name=\"\(escaped(name))\"; filename=\"\(escaped(fileName))\"\r\n"
                )
                result.append("Content-Type: \(mimeType)\r\n\r\n")
                result.append(data)
                result.append("\r\n")
            }
        }
        result.append("--\(boundary)--\r\n")
        return result
    }

    mutating func addField(name: String, value: String) {
        parts.append(.field(name: name, value: value))
    }

    mutating func addFile(name: String, fileName: String, mimeType: String, data: Data) {
        parts.append(.file(name: name, fileName: fileName, mimeType: mimeType, data: data))
    }

    private func escaped(_ value: String) -> String {
        value.replacingOccurrences(of: "\"", with: "_")
            .replacingOccurrences(of: "\r", with: "_")
            .replacingOccurrences(of: "\n", with: "_")
    }
}

private extension Data {
    mutating func append(_ string: String) {
        append(contentsOf: string.utf8)
    }
}
