# Apple platform delivery and expansion seams

Luna’s first TestFlight archive intentionally reserves the native products that will later carry notifications and glanceable conversation state. The placeholders establish stable bundle identifiers and packaging without copying credentials or presenting unfinished permission prompts.

## Shipping targets

| Product         | Bundle identifier                             | Initial behavior                                        |
| --------------- | --------------------------------------------- | ------------------------------------------------------- |
| iPhone/iPad app | `com.yannickherrero.luna`                     | Complete authenticated Luna client                      |
| iOS widget      | `com.yannickherrero.luna.widgets`             | Opens `luna://home`; live status is explicitly deferred |
| Watch companion | `com.yannickherrero.luna.watchkitapp`         | Embedded companion placeholder                          |
| Watch widget    | `com.yannickherrero.luna.watchkitapp.widgets` | Accessory placeholder for the future companion snapshot |

The main app recognizes `luna://home` and `luna://conversation/<UUID>`. Widgets and future APNs payload handling must reuse these routes rather than creating parallel navigation state.

## Notification implementation boundary

The main App ID reserves Push Notifications, but the app does not request authorization or register for remote notifications yet. The next notification phase must be a coordinated server, storage, protocol, and client change:

1. Add authenticated APNs registration and deletion routes tied to the paired device.
2. Store environment, topic, token metadata, last registration time, and delivery disablement without logging raw tokens.
3. Register only after explicit user authorization and update `notificationsEnabled` from authoritative server state.
4. Deliver conversation-completion notifications only to the conversation’s notification target.
5. Put the conversation UUID in the payload and route taps through `LunaRoute`.
6. Handle APNs feedback, token rotation, retries, idempotency, and provider-key failure without affecting conversation execution.

Do not add or reuse an APNs provider `.p8` key before that design is implemented and approved. Provider credentials belong outside the repository at a documented restrictive path.

## Widget and Watch snapshot boundary

Extensions and the Watch must never receive the bearer credential stored in the iOS Keychain. Luna's shared snapshot layer uses a strict, versioned allowlist: conversation ID, bounded title, session state, bounded summarized activity, and update time. It caps the number of agents, normalizes control characters and whitespace, rejects unknown schema versions, and has no fields for messages, credentials, tokens, or repository paths.

The intended flow is:

```text
Authenticated iOS state
  → sanitized snapshot
  → iOS App Group container
  → iOS WidgetKit timeline

Authenticated iOS state
  → WatchConnectivity application context
  → Watch App Group container
  → Watch app and Watch WidgetKit timeline
```

All four Apple targets now declare `group.com.yannickherrero.luna`; the iOS app/widget and Watch app/widget use the same filename and schema in their device-local group containers. The source entitlement does not perform Apple Developer portal changes: physical-device and distribution signing still require associating the group with every participating App ID and regenerating profiles during an explicitly approved release stage.

## Release boundary

`fastlane ios beta` bootstraps identifiers and Push capability, provisions every embedded target, verifies generated files, archives one signed IPA, uploads it to internal TestFlight, and waits for processing. External TestFlight distribution and App Store submission remain separate release decisions because they require review metadata, a reviewer-accessible Luna environment, support and privacy URLs, and final non-placeholder extension behavior.
