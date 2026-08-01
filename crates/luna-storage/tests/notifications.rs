use luna_protocol::{ApnsEnvironment, DevicePlatform, MessageDelivery};
use luna_storage::{
    AgentCycleOutcome, Database, NewApnsRegistration, NewDevice, NewPairingCode, NewUserMessage,
};
use uuid::Uuid;

const NOW: &str = "2026-03-20T12:00:00Z";
const LATER: &str = "2026-03-20T12:01:00Z";

async fn pair_device(
    database: &Database,
    name: &str,
    platform: DevicePlatform,
    suffix: &str,
) -> Uuid {
    let id = Uuid::now_v7();
    let hash = format!("pairing-{suffix}");
    let credential = format!("credential-{suffix}");
    database
        .create_pairing_code(NewPairingCode {
            id: Uuid::now_v7(),
            code_hash: &hash,
            created_at: NOW,
            expires_at: "2026-03-20T13:00:00Z",
        })
        .await
        .expect("pairing code");
    database
        .redeem_pairing_code(
            &hash,
            NewDevice {
                id,
                name,
                platform,
                credential_hash: &credential,
                created_at: NOW,
            },
        )
        .await
        .expect("pair device")
        .expect("paired device");
    id
}

async fn register(database: &Database, device_id: Uuid, token_character: char) {
    let token = token_character.to_string().repeat(64);
    database
        .upsert_apns_registration(NewApnsRegistration {
            device_id,
            token: &token,
            environment: ApnsEnvironment::Sandbox,
            topic: "com.yannickherrero.luna",
            app_version: Some("1.0"),
            registered_at: NOW,
        })
        .await
        .expect("APNs registration");
}

#[tokio::test]
async fn steering_atomically_transfers_cycle_ownership_and_delivery_is_idempotent() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let database = Database::connect(&directory.path().join("notifications.sqlite"))
        .await
        .expect("database");
    let iphone = pair_device(&database, "iPhone", DevicePlatform::Ios, "iphone").await;
    let ipad = pair_device(&database, "iPad", DevicePlatform::Ipados, "ipad").await;
    register(&database, iphone, 'a').await;
    register(&database, ipad, 'b').await;
    let conversation = database
        .create_conversation(Uuid::now_v7(), "/tmp", NOW)
        .await
        .expect("conversation");

    let initial = database
        .accept_user_message(NewUserMessage {
            conversation_id: conversation.id,
            device_id: iphone,
            client_message_id: Uuid::now_v7(),
            text: "Start on iPhone",
            attachment_ids: &[],
            delivery: MessageDelivery::Initial,
            accepted_at: NOW,
        })
        .await
        .expect("initial message");
    let steering = database
        .accept_user_message(NewUserMessage {
            conversation_id: conversation.id,
            device_id: ipad,
            client_message_id: Uuid::now_v7(),
            text: "Steer from iPad",
            attachment_ids: &[],
            delivery: MessageDelivery::Steer,
            accepted_at: LATER,
        })
        .await
        .expect("steering message");
    assert_eq!(
        database
            .orphaned_active_agent_cycle_conversations()
            .await
            .expect("active cycles"),
        vec![conversation.id]
    );
    database
        .set_dispatch_state(initial.dispatch_id, "completed", None, LATER)
        .await
        .expect("complete initial dispatch");
    database
        .set_dispatch_state(steering.dispatch_id, "completed", None, LATER)
        .await
        .expect("complete steering dispatch");
    assert_eq!(
        database
            .orphaned_active_agent_cycle_conversations()
            .await
            .expect("orphaned cycles"),
        vec![conversation.id]
    );

    let delivery = database
        .complete_active_agent_cycle(
            conversation.id,
            None,
            AgentCycleOutcome::Ready,
            "2026-03-20T12:02:00Z",
        )
        .await
        .expect("complete cycle")
        .expect("delivery");
    assert_eq!(delivery.target_device_id, ipad);
    assert_eq!(delivery.registration.device_id, ipad);
    assert_eq!(delivery.registration.token, "b".repeat(64));
    assert!(
        database
            .complete_active_agent_cycle(
                conversation.id,
                None,
                AgentCycleOutcome::Ready,
                "2026-03-20T12:02:01Z",
            )
            .await
            .expect("replayed completion")
            .is_none()
    );
}

#[tokio::test]
async fn never_falls_back_to_another_device_or_notifies_for_web_and_interruption() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let database = Database::connect(&directory.path().join("notifications.sqlite"))
        .await
        .expect("database");
    let iphone = pair_device(&database, "iPhone", DevicePlatform::Ios, "iphone").await;
    let ipad = pair_device(&database, "iPad", DevicePlatform::Ipados, "ipad").await;
    let web = pair_device(&database, "Web", DevicePlatform::Web, "web").await;
    register(&database, iphone, 'a').await;
    register(&database, ipad, 'b').await;
    database
        .disable_apns_for_device(ipad, LATER)
        .await
        .expect("disable iPad");
    let conversation = database
        .create_conversation(Uuid::now_v7(), "/tmp", NOW)
        .await
        .expect("conversation");

    database
        .accept_user_message(NewUserMessage {
            conversation_id: conversation.id,
            device_id: ipad,
            client_message_id: Uuid::now_v7(),
            text: "iPad owns this cycle",
            attachment_ids: &[],
            delivery: MessageDelivery::Initial,
            accepted_at: NOW,
        })
        .await
        .expect("iPad message");
    assert!(
        database
            .complete_active_agent_cycle(conversation.id, None, AgentCycleOutcome::Ready, LATER,)
            .await
            .expect("complete disabled cycle")
            .is_none()
    );

    database
        .accept_user_message(NewUserMessage {
            conversation_id: conversation.id,
            device_id: web,
            client_message_id: Uuid::now_v7(),
            text: "Web owns this cycle",
            attachment_ids: &[],
            delivery: MessageDelivery::Initial,
            accepted_at: "2026-03-20T12:03:00Z",
        })
        .await
        .expect("web message");
    assert!(
        database
            .complete_active_agent_cycle(
                conversation.id,
                None,
                AgentCycleOutcome::Attention,
                "2026-03-20T12:04:00Z",
            )
            .await
            .expect("complete web cycle")
            .is_none()
    );

    database
        .accept_user_message(NewUserMessage {
            conversation_id: conversation.id,
            device_id: iphone,
            client_message_id: Uuid::now_v7(),
            text: "Interrupt this",
            attachment_ids: &[],
            delivery: MessageDelivery::Initial,
            accepted_at: "2026-03-20T12:05:00Z",
        })
        .await
        .expect("iPhone message");
    assert!(
        database
            .complete_active_agent_cycle(
                conversation.id,
                None,
                AgentCycleOutcome::Interrupted,
                "2026-03-20T12:06:00Z",
            )
            .await
            .expect("interrupt cycle")
            .is_none()
    );
}
