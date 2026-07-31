import Foundation
import Testing
@testable import Luna

private struct TestRequest: Codable, Sendable {
    let value: String
}

private struct TestResponse: Codable, Equatable, Sendable {
    let accepted: Bool
}

private actor RecordingTransport: HTTPTransport {
    private let response: HTTPResponse
    private(set) var requests: [URLRequest] = []

    init(status: Int = 200, data: Data) {
        let url = URL(string: "https://mac.example.ts.net:8447")!
        let response = HTTPURLResponse(
            url: url,
            statusCode: status,
            httpVersion: "HTTP/1.1",
            headerFields: ["Content-Type": "application/json"]
        )!
        self.response = HTTPResponse(data: data, response: response)
    }

    func data(for request: URLRequest) -> HTTPResponse {
        requests.append(request)
        return response
    }

    func lastRequest() -> URLRequest? {
        requests.last
    }

    func requestCount() -> Int {
        requests.count
    }
}

struct NetworkingTests {
    private let server = URL(string: "https://mac.example.ts.net:8447")!

    @Test
    func attachesBearerCredentialsAndJSONBody() async throws {
        let credentials = MemoryCredentialStore()
        await credentials.setToken("native-token", for: server)
        let transport = RecordingTransport(data: Data(#"{"accepted":true}"#.utf8))
        let client = APIClient(baseURL: server, credentials: credentials, transport: transport)

        let response: TestResponse = try await client.post(
            "/v1/example",
            body: TestRequest(value: "Luna")
        )
        let request = try #require(await transport.lastRequest())

        #expect(response.accepted)
        #expect(request.url?.absoluteString == "https://mac.example.ts.net:8447/v1/example")
        #expect(request.value(forHTTPHeaderField: "Authorization") == "Bearer native-token")
        #expect(request.value(forHTTPHeaderField: "Content-Type") == "application/json")
        let requestBody = try #require(request.httpBody)
        let object = try #require(
            JSONSerialization.jsonObject(with: requestBody) as? [String: String]
        )
        #expect(object == ["value": "Luna"])
    }

    @Test
    func leavesPairingRequestsUnauthenticated() async throws {
        let transport = RecordingTransport(
            status: 202,
            data: Data(#"{"expiresAt":"2026-03-20T12:15:00Z"}"#.utf8)
        )
        let client = APIClient(
            baseURL: server,
            credentials: MemoryCredentialStore(),
            transport: transport
        )

        let response: PairingCodeRequestResponse = try await client.post(
            "/v1/pairing/request",
            authenticated: false
        )
        let request = try #require(await transport.lastRequest())

        #expect(response.expiresAt == "2026-03-20T12:15:00Z")
        #expect(request.value(forHTTPHeaderField: "Authorization") == nil)
    }

    @Test
    func surfacesTypedServerErrors() async throws {
        let credentials = MemoryCredentialStore()
        await credentials.setToken("native-token", for: server)
        let transport = RecordingTransport(
            status: 409,
            data: Data(
                #"{"code":"conflict","message":"Pi is busy.","retryable":false}"#.utf8
            )
        )
        let client = APIClient(baseURL: server, credentials: credentials, transport: transport)

        await #expect(throws: APIClientError.self) {
            try await client.postVoid("/v1/conversations/id/abort")
        }
        do {
            try await client.postVoid("/v1/conversations/id/abort")
            Issue.record("Expected conflict")
        } catch let APIClientError.server(status, error) {
            #expect(status == 409)
            #expect(error.code == .conflict)
            #expect(error.message == "Pi is busy.")
        }
    }

    @Test
    func neverSendsCredentialsToAnotherOrigin() async throws {
        let credentials = MemoryCredentialStore()
        await credentials.setToken("native-token", for: server)
        let transport = RecordingTransport(data: Data())
        let client = APIClient(baseURL: server, credentials: credentials, transport: transport)

        await #expect(throws: APIClientError.self) {
            _ = try await client.data(at: "https://attacker.example/v1/media")
        }
        #expect(await transport.requestCount() == 0)
    }

    @Test
    func buildsServerCompatibleMultipartBodies() throws {
        var form = MultipartForm(boundary: "LunaBoundary")
        form.addField(name: "conversationId", value: "conversation-id")
        form.addFile(
            name: "file",
            fileName: "screen.png",
            mimeType: "image/png",
            data: Data([0x01, 0x02])
        )
        let body = String(decoding: form.data, as: UTF8.self)

        #expect(form.contentType == "multipart/form-data; boundary=LunaBoundary")
        #expect(body.contains("name=\"conversationId\"\r\n\r\nconversation-id"))
        #expect(body.contains("filename=\"screen.png\""))
        #expect(body.hasSuffix("--LunaBoundary--\r\n"))
    }

    @Test
    func cachesAuthenticatedImages() async throws {
        let credentials = MemoryCredentialStore()
        await credentials.setToken("native-token", for: server)
        let pixel = Data(
            base64Encoded: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="
        )!
        let transport = RecordingTransport(data: pixel)
        let client = APIClient(baseURL: server, credentials: credentials, transport: transport)
        let loader = AuthenticatedImageLoader(client: client)

        _ = try await loader.image(at: "/v1/attachments/id/content")
        _ = try await loader.image(at: "/v1/attachments/id/content")

        #expect(await transport.requestCount() == 1)
    }

    @Test
    func storesCredentialsInAnIsolatedKeychainService() async throws {
        let store = KeychainCredentialStore(service: "com.yannickherrero.luna.tests.\(UUID())")

        try await store.setToken("secret", for: server)
        #expect(try await store.token(for: server) == "secret")
        try await store.removeToken(for: server)
        #expect(try await store.token(for: server) == nil)
    }
}
