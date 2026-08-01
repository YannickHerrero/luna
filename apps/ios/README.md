# Luna for iPhone and iPad

Luna is a universal native SwiftUI client for iOS and iPadOS 18 or later, with a watchOS 11 companion and WidgetKit placeholders. It uses the same authenticated HTTP and WebSocket protocol as the PWA, stores the device credential in Keychain, and adapts the PWA’s Catppuccin interface to compact and regular-width Apple devices.

## Requirements

- Xcode with iOS 18 and watchOS 11 or later SDKs
- XcodeGen 2.45 or later
- Node.js and pnpm versions from the repository root `package.json`

## Generate the project

`project.yml` is the source of truth for targets and build settings. Generated design tokens, app icons, and Lucide assets are checked in so clean builds do not require a script phase.

From the repository root:

```sh
pnpm install
pnpm generate
cd apps/ios
xcodegen generate
```

Regenerate `Luna.xcodeproj` after changing `project.yml`, adding source files, or changing generated resources. Review generated changes before committing them.

## Configure a server

The pairing screen displays the active server and provides a native server editor. Remote servers must use HTTPS; HTTP is accepted only for loopback development addresses.

For a temporary development override, set `LUNA_SERVER_URL` in the Xcode scheme environment. A server selected in the app is persisted in `UserDefaults`. The paired bearer credential is stored separately in Keychain and must never be added to source control.

On the server, request a fresh pairing code and enter the newest code in the native pairing screen. Native clients identify themselves as iOS devices and otherwise use the same pairing/bootstrap contract as the PWA.

## Build and test

List available simulator identifiers:

```sh
xcrun simctl list devices available
```

Run all unit and UI tests on an iPhone simulator:

```sh
xcodebuild test \
  -project apps/ios/Luna.xcodeproj \
  -scheme Luna \
  -destination 'platform=iOS Simulator,id=<SIMULATOR_UDID>'
```

Verify the regular-width build on an iPad simulator:

```sh
xcodebuild build \
  -project apps/ios/Luna.xcodeproj \
  -scheme Luna \
  -destination 'platform=iOS Simulator,id=<IPAD_SIMULATOR_UDID>'
```

Run the watchOS unit and UI suite on an available Apple Watch simulator:

```sh
xcodebuild test \
  -project apps/ios/Luna.xcodeproj \
  -scheme LunaWatch \
  -destination 'platform=watchOS Simulator,id=<WATCH_SIMULATOR_UDID>'
```

The iOS UI suite includes deterministic pairing and ready-state fixtures, populated transcript/composer coverage, conversation controls, Dynamic Type audits, and minimum hit-region audits. Debug launches support these fixture arguments:

- `-ui-testing-ready` installs the in-process ready-state fixture.
- `-ui-testing-list` opens that fixture on the conversation list.
- `-ui-testing-grouped` enables project grouping for deterministic snapshots.
- `-ui-testing-reset-grouping` disables project grouping before a persistence test.
- `-luna-theme latte` or `-luna-theme mocha` selects a deterministic theme.

## Project structure

- `Luna/App`: app lifecycle, server configuration, fixtures, and root navigation
- `Luna/Protocol`: native models matching the server protocol
- `Luna/Networking`: authenticated HTTP, multipart media, images, and WebSockets
- `Luna/Persistence`: Keychain-backed device credentials
- `Luna/State`: synchronized conversations, messages, reconnect, and recovery
- `Luna/Features`: pairing, shell, transcript, composer, and agent controls
- `Luna/DesignSystem`: shared colors, typography, surfaces, icons, and controls
- `LunaShared/Snapshots`: versioned, allowlisted App Group snapshot models and storage
- `LunaTests`: protocol, networking, state, Markdown, composer, pairing, and control tests
- `LunaUITests`: end-to-end native fixture and accessibility acceptance tests
- `LunaWidgets`: A2 Activity field widget for current iPhone/iPad agents
- `LunaWatch`: embedded watchOS companion placeholder and tests
- `LunaWatchWidgets`: watchOS accessory widget placeholders

## Physical devices, signing, and notifications

Simulator builds do not require signing. For physical-device signing, copy `Config/Local.xcconfig.example` to the ignored `Config/Local.xcconfig` and set the local development team. Do not commit provisioning profiles, certificates, private keys, or team-specific project changes.

The main app reserves the Push Notifications entitlement, stable `luna://home` and `luna://conversation/<UUID>` routes, and the server’s existing notification-target state. Notification permission, APNs token registration, and provider delivery remain intentionally deferred. Do not add or reuse an APNs provider `.p8` key until the server registration and delivery routes are designed and approved.

The app, widget, Watch app, and Watch widget declare `group.com.yannickherrero.luna` and compile the same versioned, allowlisted active-agent snapshot store. Widget publication and WatchConnectivity are separate layers; device credentials remain in the iOS Keychain and are never represented in snapshot models or copied to extensions or the Watch. Physical-device and distribution signing require associating the App Group with every participating App ID and regenerating profiles during an explicitly approved release stage.

## TestFlight

Fastlane provisions and exports the iPhone/iPad app, iOS widget, Watch companion, and Watch widget as one archive. Local API-key metadata belongs in ignored `../../fastlane/.env`; the private key remains outside the repository. See [`../../fastlane/README.md`](../../fastlane/README.md) for lanes and setup, and [`../../docs/apple-platform-roadmap.md`](../../docs/apple-platform-roadmap.md) for the notification and shared-snapshot seams.
