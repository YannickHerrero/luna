# Apple platform delivery and expansion seams

Luna’s Apple targets carry device-targeted notifications and sanitized glanceable conversation state while preserving stable bundle identifiers and strict credential boundaries.

## Shipping targets

| Product         | Bundle identifier                             | Initial behavior                                        |
| --------------- | --------------------------------------------- | ------------------------------------------------------- |
| iPhone/iPad app | `com.yannickherrero.luna`                     | Complete authenticated Luna client                      |
| iOS widgets     | `com.yannickherrero.luna.widgets`             | A2 Active Agents and B2 account weekly usage             |
| Watch companion | `com.yannickherrero.luna.watchkitapp`         | Validates and displays WatchConnectivity snapshots       |
| Watch widget    | `com.yannickherrero.luna.watchkitapp.widgets` | C3 Work pulse Smart Stack status                         |

The main app recognizes `luna://home` and `luna://conversation/<UUID>`. Widgets and future APNs payload handling must reuse these routes rather than creating parallel navigation state.

## Notification implementation boundary

The notification flow is coordinated across protocol, storage, server runtime, and the authenticated iOS app:

1. Authenticated registration and deletion routes tie one active APNs environment/topic/token to the paired native device without returning or logging the token.
2. Every accepted non-shell agent cycle durably records its causal device owner. Accepted steering transfers ownership atomically; web ownership clears the target instead of falling back to another device.
3. Completion, failure, and crash paths create at most one durable delivery per cycle. Voluntary interruption suppresses delivery. Bounded retries survive pending-delivery restart recovery, and invalid-token feedback disables the affected registration.
4. The app requests explicit system authorization after bootstrap, registers on authorization and token rotation, disables registration after denial, and retries transient registration failures on foreground activation.
5. Payloads contain a bounded conversation title, generic status copy, conversation UUID, and stable `luna://conversation/<UUID>` route—never complete messages, credentials, account identifiers, tokens, or repository paths.
6. Notification taps flow through `LunaRoute`; foreground presentation is suppressed only when that same conversation is already selected.

Provider credentials remain outside the repository at the restrictive location documented in the deployment runbook. Production APNs delivery and provisioning changes require an explicitly approved release stage and signed physical-device verification.

## Widget and Watch snapshot boundary

Extensions and the Watch must never receive the bearer credential stored in the iOS Keychain. Luna's shared snapshot layer uses a strict, versioned allowlist: conversation ID, bounded title, session state, bounded summarized activity, and update time. It caps the number of agents, normalizes control characters and whitespace, rejects unknown schema versions, and has no fields for messages, credentials, tokens, or repository paths.

The iOS Active Agents publisher includes only starting, working, compacting, restoring, and retrying conversations. It classifies raw activity into a fixed safe vocabulary, publishes only when display content changes, and requests a timeline reload after an atomic write. The A2 Activity field widget displays current, empty, stale, and unavailable states; small widgets feature one conversation while medium widgets deep-link up to three activity cards.

The iOS app also fetches the authenticated server's sanitized OpenAI weekly snapshot when it becomes active. It validates and atomically publishes only availability, used percentage, localizable reset date, and collection date. The B2 Capacity line widget shows percent used (never remaining), formats reset time in the device time zone, retains stale values with explicit age, and renders unavailable instead of inventing a value.

The same encoded active-agent snapshot is sent as WatchConnectivity application context. The Watch rejects malformed or unsupported versions, atomically persists the latest valid value in its App Group, updates companion connection/freshness status, and reloads the C3 Work pulse widget. The widget is a classic user-pinnable accessory-rectangular Smart Stack surface: four static segments encode agent count, the exact total remains textual, and stale or unreachable state never presents old data as live.

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
