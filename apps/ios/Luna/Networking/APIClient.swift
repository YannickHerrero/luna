import Foundation

struct HTTPResponse: @unchecked Sendable {
    let data: Data
    let response: HTTPURLResponse
}

protocol HTTPTransport: Sendable {
    func data(for request: URLRequest) async throws -> HTTPResponse
}

struct URLSessionHTTPTransport: HTTPTransport, @unchecked Sendable {
    let session: URLSession

    init(session: URLSession = .shared) {
        self.session = session
    }

    func data(for request: URLRequest) async throws -> HTTPResponse {
        let (data, response) = try await session.data(for: request)
        guard let response = response as? HTTPURLResponse else {
            throw APIClientError.invalidResponse
        }
        return HTTPResponse(data: data, response: response)
    }
}

enum HTTPMethod: String, Sendable {
    case get = "GET"
    case post = "POST"
    case patch = "PATCH"
}

enum APIClientError: Error, Equatable, Sendable {
    case invalidURL(String)
    case untrustedURL(URL)
    case authenticationRequired
    case invalidResponse
    case server(status: Int, error: LunaAPIError)
    case decoding(String)
}

extension APIClientError: LocalizedError {
    var errorDescription: String? {
        switch self {
        case let .invalidURL(value):
            "Luna’s server URL is invalid: \(value)"
        case .untrustedURL:
            "Luna refused to send credentials to another server."
        case .authenticationRequired:
            "Pair this device with Luna before continuing."
        case .invalidResponse:
            "Luna received an invalid server response."
        case let .server(_, error):
            error.message
        case .decoding:
            "Luna could not understand the server response."
        }
    }
}

extension LunaAPIError: LocalizedError {
    var errorDescription: String? { message }
}

struct APIClient: Sendable {
    let baseURL: URL
    private let credentials: any CredentialStore
    private let transport: any HTTPTransport

    init(
        baseURL: URL,
        credentials: any CredentialStore = KeychainCredentialStore.shared,
        transport: any HTTPTransport = URLSessionHTTPTransport()
    ) {
        self.baseURL = baseURL
        self.credentials = credentials
        self.transport = transport
    }

    func get<Response: Decodable & Sendable>(
        _ path: String,
        authenticated: Bool = true,
        as responseType: Response.Type = Response.self
    ) async throws -> Response {
        let request = try await makeRequest(path: path, method: .get, authenticated: authenticated)
        return try await execute(request, as: responseType)
    }

    func post<Response: Decodable & Sendable>(
        _ path: String,
        authenticated: Bool = true,
        as responseType: Response.Type = Response.self
    ) async throws -> Response {
        let request = try await makeRequest(path: path, method: .post, authenticated: authenticated)
        return try await execute(request, as: responseType)
    }

    func post<Body: Encodable & Sendable, Response: Decodable & Sendable>(
        _ path: String,
        body: Body,
        authenticated: Bool = true,
        as responseType: Response.Type = Response.self
    ) async throws -> Response {
        let data = try JSONEncoder().encode(body)
        let request = try await makeRequest(
            path: path,
            method: .post,
            body: data,
            contentType: "application/json",
            authenticated: authenticated
        )
        return try await execute(request, as: responseType)
    }

    func patch<Body: Encodable & Sendable, Response: Decodable & Sendable>(
        _ path: String,
        body: Body,
        as responseType: Response.Type = Response.self
    ) async throws -> Response {
        let data = try JSONEncoder().encode(body)
        let request = try await makeRequest(
            path: path,
            method: .patch,
            body: data,
            contentType: "application/json",
            authenticated: true
        )
        return try await execute(request, as: responseType)
    }

    func postVoid(_ path: String) async throws {
        let request = try await makeRequest(path: path, method: .post, authenticated: true)
        _ = try await executeData(request)
    }

    func upload<Response: Decodable & Sendable>(
        _ path: String,
        form: MultipartForm,
        as responseType: Response.Type = Response.self
    ) async throws -> Response {
        let request = try await makeRequest(
            path: path,
            method: .post,
            body: form.data,
            contentType: form.contentType,
            authenticated: true
        )
        return try await execute(request, as: responseType)
    }

    func data(at path: String) async throws -> Data {
        let request = try await makeRequest(path: path, method: .get, authenticated: true)
        return try await executeData(request).data
    }

    func makeRequest(
        path: String,
        method: HTTPMethod,
        body: Data? = nil,
        contentType: String? = nil,
        authenticated: Bool
    ) async throws -> URLRequest {
        let url = try resolve(path)
        var request = URLRequest(url: url)
        request.httpMethod = method.rawValue
        request.httpBody = body
        request.timeoutInterval = 60
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        if let contentType {
            request.setValue(contentType, forHTTPHeaderField: "Content-Type")
        }
        if authenticated {
            guard let token = try await credentials.token(for: baseURL) else {
                throw APIClientError.authenticationRequired
            }
            request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        }
        return request
    }

    private func execute<Response: Decodable & Sendable>(
        _ request: URLRequest,
        as responseType: Response.Type
    ) async throws -> Response {
        let result = try await executeData(request)
        do {
            return try JSONDecoder().decode(responseType, from: result.data)
        } catch {
            throw APIClientError.decoding(String(describing: error))
        }
    }

    private func executeData(_ request: URLRequest) async throws -> HTTPResponse {
        let result = try await transport.data(for: request)
        guard (200 ... 299).contains(result.response.statusCode) else {
            let serverError = (try? JSONDecoder().decode(LunaAPIError.self, from: result.data))
                ?? LunaAPIError(
                    code: .internalError,
                    message: "Luna could not complete the request.",
                    retryable: result.response.statusCode >= 500,
                    requestId: nil
                )
            throw APIClientError.server(status: result.response.statusCode, error: serverError)
        }
        return result
    }

    private func resolve(_ value: String) throws -> URL {
        guard let url = URL(string: value, relativeTo: baseURL)?.absoluteURL else {
            throw APIClientError.invalidURL(value)
        }
        guard sameOrigin(url, baseURL) else {
            throw APIClientError.untrustedURL(url)
        }
        return url
    }

    private func sameOrigin(_ left: URL, _ right: URL) -> Bool {
        left.scheme?.lowercased() == right.scheme?.lowercased()
            && left.host?.lowercased() == right.host?.lowercased()
            && effectivePort(left) == effectivePort(right)
    }

    private func effectivePort(_ url: URL) -> Int? {
        url.port ?? (url.scheme?.lowercased() == "https" ? 443 : 80)
    }
}
