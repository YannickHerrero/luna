# Luna

Luna turns persistent [Pi](https://github.com/badlogic/pi-mono) sessions into private,
iMessage-like conversations available from iPhone, iPad, and the web.

The V1 architecture is deliberately small:

- **`luna-server`** — Rust, Axum, Tokio, and SQLx/SQLite
- **Pi runtime** — one supervised `pi --mode rpc` process per active conversation
- **Pi bridge** — a TypeScript extension for dispatch markers, logical working directories,
  and structured repository observations
- **PWA** — a statically exported Next.js app served by the Rust binary
- **Protocol** — Rust DTOs and OpenAPI are canonical; TypeScript artifacts are generated

Raw terminal output is never sent to clients. The server persists normalized conversation,
message, activity, workspace, repository, and attachment events.

## Development

Requirements: Rust 1.95+, Node 24+, and pnpm 11.3+.

```sh
pnpm install
pnpm generate
pnpm test
pnpm typecheck
pnpm lint
pnpm build
```

Run the complete service at `http://127.0.0.1:9870`:

```sh
pnpm build:web
cargo run -p luna-server
```

The server prints a single-use pairing code at startup. The PWA prompts for this code and
stores its device credential in an HttpOnly, SameSite cookie.

## Privacy and networking

Luna binds to loopback by default. Production access is expected to use **Tailscale Serve**, not
Funnel. Device tokens are hashed at rest, pairing codes are single-use, attachment files use
private permissions, and uploaded audio is proxied in memory without being persisted.

See [`docs/deployment.md`](docs/deployment.md) for the Citadel and Tailscale runbook.
