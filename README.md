<p align="center">
  <img src="apps/web/public/icon-192.png" width="96" height="96" alt="Luna icon">
</p>

<h1 align="center">Luna</h1>

<p align="center">
  Persistent Pi conversations from every device.<br>
  A private Rust server with web, terminal, iPhone, and iPad clients for continuing agent work without losing context.
</p>

<p align="center">
  <strong>Rust</strong> · <strong>Axum</strong> · <strong>SQLite</strong> · <strong>Next.js</strong> · <strong>SwiftUI</strong> · <strong>Pi RPC</strong> · <strong>Tailscale</strong>
</p>

![Luna conversation view on desktop](docs/images/luna-desktop.png)

<p align="center">
  <img src="docs/images/luna-mobile.png" width="360" alt="Luna conversation view on mobile">
  <img src="docs/images/luna-mobile-list.png" width="360" alt="Luna conversation list on mobile">
</p>

## What Luna does

- Keeps one supervised Pi RPC session per active conversation.
- Streams normalized assistant messages, tool activity, state, workspace, and repository updates.
- Restores durable Pi context after server or process restarts.
- Supports steering, interruption, explicit `!` shell commands, Markdown, syntax highlighting, image attachments, and voice transcription.
- Generates concise contextual conversation titles with an isolated Pi model request.
- Tracks multiple repositories and discovers project icons automatically.
- Syncs reconnecting devices through retained, cursor-based events.
- Runs as a responsive, offline-capable PWA, a terminal-palette-aware Rust TUI for disposable SSH sessions, and a universal native iPhone/iPad app with Catppuccin Latte and Mocha themes.
- Ships reserved iOS/watchOS widget surfaces and an embedded Apple Watch companion placeholder for staged native expansion.
- Serves everything from one loopback-bound Rust process.

Pi process stdout never reaches clients. Output from an authenticated user's explicit `!` shell command is the sole exception: Luna bounds and persists it as a conversation message. SQLite is authoritative for client state, while Pi's session JSONL remains authoritative for agent context.

## Architecture

```mermaid
flowchart LR
    Web[Installable PWA] -->|HTTPS / WebSocket| TS[Tailscale Serve]
    Apple[Native iPhone and iPad app] -->|HTTPS / WebSocket| TS
    SSH[Remote SSH terminal] --> TUI[luna-tui]
    TUI -->|HTTPS / WebSocket| TS
    TS -->|loopback HTTP| Server[luna-server\nAxum + Tokio]
    Server --> DB[(SQLite)]
    Server --> Media[Private media storage]
    Server --> Pi[Supervised Pi RPC sessions]
    Pi --> Bridge[Luna Pi bridge]
    Bridge --> Server
```

The server owns authentication, normalized event persistence, session supervision, dispatch reconciliation, retention, and recovery. Citadel supervises only the Luna server process; Luna supervises its own Pi children.

## Repository layout

| Path                     | Purpose                                                         |
| ------------------------ | --------------------------------------------------------------- |
| `crates/luna-server`     | Axum HTTP/WebSocket server and Pi orchestration                 |
| `crates/luna-storage`    | SQLx/SQLite persistence and migrations                          |
| `crates/luna-pi`         | Strict JSONL RPC client, process supervision, and normalization |
| `crates/luna-protocol`   | Canonical Serde/Schemars/Utoipa protocol types                  |
| `apps/web`               | Statically exported Next.js PWA                                 |
| `apps/tui`               | Rust/Ratatui terminal client for disposable SSH sessions        |
| `apps/ios`               | iPhone/iPad app, widgets, Watch companion, tests, and XcodeGen  |
| `integrations/pi`        | Pi bridge extension                                             |
| `integrations/citadel`   | Production service manifest                                     |
| `fastlane`               | Apple ID bootstrap, signing, archive, and TestFlight upload     |
| `packages/protocol`      | Generated OpenAPI and TypeScript bindings                       |
| `packages/design-tokens` | Shared Catppuccin design tokens                                 |

## Development

Requirements: Rust 1.95+, Node.js 24+, and pnpm 11.3+. Native Apple development additionally requires Xcode with iOS 18+ and watchOS 11+ SDKs plus XcodeGen 2.45+.

```sh
pnpm install
pnpm generate
pnpm test
pnpm typecheck
pnpm lint
pnpm build
```

For faster agent and local-development feedback, run the same generated-code and static/unit checks through the resource-aware scheduler:

```sh
pnpm verify:code
```

It runs the Rust suite once after generation, then parallelizes the independent web tests, type checking, and linting. This avoids Cargo contention while shortening the remaining critical path. `pnpm verify` adds the serialized browser E2E and release-build gates and writes timing metadata under ignored `.data/verification/`.

For broad codebase analysis, Luna's Pi bridge exposes a `luna_scout` tool that can run up to two isolated read-only investigations concurrently. Scouts are restricted to the current Git repository, receive only read/search tools, load no project extensions or context files, inherit only bounded runtime/provider environment variables, and return capped evidence for the coordinating agent to verify and synthesize. They cannot implement, build, test, use Git, push, deploy, or recursively delegate.

Run the complete application locally:

```sh
pnpm build:web
LUNA_ENV_FILE=/tmp/luna-development.env cargo run -p luna-server
```

On the pairing screen, choose **Ask for a pairing code**. Luna writes a fresh six-digit, single-use, 15-minute code to its Citadel stdout log and invalidates older unused codes. Enter that newest code with a device name; the web client exchanges it for a credential stored in an HttpOnly, SameSite cookie.

### Terminal client over SSH

Build and pair the on-demand terminal client once:

```sh
pnpm build:tui
install -d "$HOME/.local/bin"
install -m 755 target/release/luna-tui "$HOME/.local/bin/luna-tui"
luna-tui --server https://your-mac.example.ts.net:8447
```

Subsequent sessions are disposable and can be opened directly through SSH:

```sh
ssh -t luna-host luna-tui
```

Quitting the TUI does not interrupt Pi. Reopening reloads durable messages and catches up from retained events. See [`apps/tui/README.md`](apps/tui/README.md) for profiles, credential storage, keys, and MVP limitations.

Run deterministic browser acceptance coverage with locally installed Chrome:

```sh
pnpm --filter @luna/web test:e2e
```

The suite covers pairing, persistent streaming, steering, interruption, image upload, rename/archive flows, reload recovery, themes, service workers, keyboard behavior, and serious/critical accessibility checks.

### Native iPhone and iPad development

The checked-in Xcode project is generated from `apps/ios/project.yml`. After running `pnpm generate`, regenerate it with XcodeGen and run the native unit/UI suite on an available simulator:

```sh
cd apps/ios
xcodegen generate
cd ../..
xcrun simctl list devices available
xcodebuild test \
  -project apps/ios/Luna.xcodeproj \
  -scheme Luna \
  -destination 'platform=iOS Simulator,id=<SIMULATOR_UDID>'
```

The native pairing screen can select a private HTTPS server, while `LUNA_SERVER_URL` provides a temporary Xcode-scheme override for development. Device credentials remain in Keychain. See [`apps/ios/README.md`](apps/ios/README.md) for setup, architecture, fixture arguments, iPad/watchOS verification, signing, widgets, and notification-readiness details. TestFlight automation lives in [`fastlane`](fastlane/README.md).

## Production

Luna is designed to bind only to `127.0.0.1`, run under [Citadel](https://github.com/YannickHerrero/citadel), and be exposed privately through **Tailscale Serve**—never Funnel.

Production configuration lives outside Git in `~/.config/luna/server.env` with mode `600`. State defaults to `~/Library/Application Support/Luna Server` and includes SQLite, Pi sessions, attachments, and repository icons.

See [`docs/deployment.md`](docs/deployment.md) for build, backup, Citadel, Tailscale, verification, and rollback procedures.

## Security and privacy

- Loopback binding by default
- Tailnet identity allowlisting
- One-time pairing codes and hashed device credentials
- Origin validation and strict device cookies
- Authenticated private attachment and icon routes
- Bounded request, image decode, and event-retention limits
- Ephemeral transcription proxying without audio persistence
- Sanitized account-level weekly usage without copying Codex credentials to clients
- Device-targeted, idempotent APNs delivery without logging tokens or message content
- Security headers, immutable hashed assets, and no-cache app entry points
- No Funnel configuration or raw terminal transport

## Current scope

Luna V1 provides the server, PWA, SSH-friendly Rust TUI, universal native iPhone/iPad client, A2/B2 iOS widgets, Watch companion, and C3 Smart Stack widget. Device-targeted APNs registration and provider delivery preserve initiating-device ownership across steering and route notification taps through stable conversation deep links. Signed physical-device delivery and distribution provisioning remain release-stage verification.
