import Foundation

protocol EventSource: Sendable {
    func events(for request: URLRequest) -> AsyncThrowingStream<ServerEventEnvelope, Error>
}

enum EventSourceError: Error, LocalizedError, Sendable {
    case unsupportedMessage

    var errorDescription: String? {
        "Luna received an unsupported realtime message."
    }
}

struct URLSessionEventSource: EventSource, @unchecked Sendable {
    private let session: URLSession

    init(session: URLSession? = nil) {
        if let session {
            self.session = session
        } else {
            let configuration = URLSessionConfiguration.default
            configuration.waitsForConnectivity = true
            configuration.timeoutIntervalForRequest = 60
            self.session = URLSession(configuration: configuration)
        }
    }

    func events(for request: URLRequest) -> AsyncThrowingStream<ServerEventEnvelope, Error> {
        let socket = session.webSocketTask(with: request)
        return AsyncThrowingStream { continuation in
            let receiver = Task {
                socket.resume()
                do {
                    while !Task.isCancelled {
                        let message = try await socket.receive()
                        let data: Data
                        switch message {
                        case let .string(value):
                            data = Data(value.utf8)
                        case let .data(value):
                            data = value
                        @unknown default:
                            throw EventSourceError.unsupportedMessage
                        }
                        continuation.yield(
                            try JSONDecoder().decode(ServerEventEnvelope.self, from: data)
                        )
                    }
                    continuation.finish()
                } catch is CancellationError {
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }
            }
            continuation.onTermination = { @Sendable _ in
                receiver.cancel()
                socket.cancel(with: .goingAway, reason: nil)
            }
        }
    }
}

struct EmptyEventSource: EventSource {
    func events(for request: URLRequest) -> AsyncThrowingStream<ServerEventEnvelope, Error> {
        AsyncThrowingStream { continuation in
            continuation.finish()
        }
    }
}
