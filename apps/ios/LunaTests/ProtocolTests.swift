import Foundation
import Testing
@testable import Luna

struct ProtocolTests {
    @Test
    func decodesPairingBootstrapUsingTheServerContract() throws {
        let data = Data(
            #"""
            {
              "deviceId": "00000000-0000-0000-0000-000000000001",
              "token": "secret-token",
              "bootstrap": {
                "protocolVersion": 1,
                "cursor": 14,
                "device": {
                  "id": "00000000-0000-0000-0000-000000000001",
                  "name": "iPhone",
                  "platform": "ios",
                  "notificationsEnabled": false,
                  "createdAt": "2026-03-20T12:00:00Z",
                  "lastSeenAt": "2026-03-20T12:00:00Z"
                },
                "conversations": [{
                  "id": "00000000-0000-0000-0000-000000000002",
                  "title": "Luna iOS",
                  "titleMode": "automatic",
                  "state": "working",
                  "preview": "Building the native client",
                  "activeWorkingDirectory": "/Users/example/dev/luna",
                  "repositories": [],
                  "activities": [],
                  "unreadCount": 0,
                  "createdAt": "2026-03-20T12:00:00Z",
                  "updatedAt": "2026-03-20T12:01:00Z",
                  "version": 2
                }]
              }
            }
            """#.utf8
        )

        let response = try JSONDecoder().decode(PairingExchangeResponse.self, from: data)

        #expect(response.token == "secret-token")
        #expect(response.bootstrap.protocolVersion == 1)
        #expect(response.bootstrap.conversations.first?.state == .working)
    }

    @Test
    func decodesStreamedEventEnvelope() throws {
        let data = Data(
            #"""
            {
              "version": 1,
              "eventId": 15,
              "conversationId": "00000000-0000-0000-0000-000000000002",
              "emittedAt": "2026-03-20T12:01:01Z",
              "type": "message.delta",
              "payload": {
                "messageId": "00000000-0000-0000-0000-000000000003",
                "chunkIndex": 1,
                "delta": "Hello"
              }
            }
            """#.utf8
        )

        let envelope = try JSONDecoder().decode(ServerEventEnvelope.self, from: data)
        guard case let .messageDelta(delta) = envelope.event else {
            Issue.record("Expected message.delta")
            return
        }
        #expect(envelope.eventId == 15)
        #expect(delta.chunkIndex == 1)
        #expect(delta.delta == "Hello")
    }

    @Test
    func preservesForwardCompatibilityForUnknownEvents() throws {
        let data = Data(
            #"""
            {
              "version": 1,
              "eventId": 16,
              "emittedAt": "2026-03-20T12:01:02Z",
              "type": "future.event",
              "payload": {"value": true}
            }
            """#.utf8
        )

        let envelope = try JSONDecoder().decode(ServerEventEnvelope.self, from: data)
        #expect(envelope.event == .unknown(type: "future.event"))
    }

    @Test
    func recognizesTUIPlatformFromTheSharedProtocol() throws {
        let platform = try JSONDecoder().decode(
            DevicePlatform.self,
            from: Data(#""tui""#.utf8)
        )

        #expect(platform == .tui)
    }

    @Test
    func encodesNativePairingPlatform() throws {
        let request = PairingExchangeRequest(code: "123456", deviceName: "Yannick’s iPhone", platform: .ios)
        let object = try #require(
            JSONSerialization.jsonObject(with: JSONEncoder().encode(request)) as? [String: Any]
        )

        #expect(object["code"] as? String == "123456")
        #expect(object["platform"] as? String == "ios")
        #expect(object["deviceName"] as? String == "Yannick’s iPhone")
    }
}
