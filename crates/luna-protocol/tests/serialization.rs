use luna_protocol::{ClientCommand, ClientHello, ServerEvent, ServerEventEnvelope};
use uuid::Uuid;

#[test]
fn client_commands_use_top_level_discriminators() {
    let command = ClientCommand::ClientHello {
        version: 1,
        command: ClientHello {
            request_id: Uuid::nil(),
            device_id: Uuid::nil(),
            last_cursor: 42,
        },
    };
    let value = serde_json::to_value(command).expect("serialize command");
    assert_eq!(value["type"], "client.hello");
    assert_eq!(value["lastCursor"], 42);
}

#[test]
fn event_envelopes_flatten_normalized_events() {
    let event = ServerEventEnvelope {
        version: 1,
        event_id: Some(7),
        conversation_id: None,
        emitted_at: "2026-01-01T00:00:00Z".into(),
        event: ServerEvent::ServerWelcome {
            cursor: 7,
            resumed: true,
        },
    };
    let value = serde_json::to_value(event).expect("serialize event");
    assert_eq!(value["type"], "server.welcome");
    assert_eq!(value["payload"]["cursor"], 7);
}
