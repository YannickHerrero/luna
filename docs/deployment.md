# Luna deployment runbook

Production on this Mac is supervised by Citadel and exposed only to the tailnet through
Tailscale Serve. Never enable Tailscale Funnel for Luna.

## Build and verify

From the Luna repository:

```sh
pnpm install --frozen-lockfile
pnpm generate
pnpm test
pnpm --filter @luna/web test:e2e
pnpm typecheck
pnpm lint
pnpm build
```

`pnpm build` exports the PWA to `apps/web/out` and builds
`target/release/luna-server`. Citadel starts only that release binary; Node is used by the Pi
subprocess and at build time, not as a second web service. The server invokes the locally
authenticated `codex app-server` on demand to collect the general account weekly limit.

## Private configuration

Luna automatically reads `~/.config/luna/server.env`. Create it from the committed template:

```sh
mkdir -p ~/.config/luna
install -m 600 config/server.env.example ~/.config/luna/server.env
$EDITOR ~/.config/luna/server.env
```

Keep API keys only in this external file. Luna refuses to load it if group or other permissions
are present. Codex account usage does not require another key: Luna reads only the sanitized
weekly window from the local Codex login, caches it for five minutes by default, and never returns
account identifiers or Codex credentials. The non-secret local development file `.luna.local.json` may be copied from
`.luna.local.example.json`; it is ignored by Git.

Default state is under `~/Library/Application Support/Luna Server`:

- `luna.sqlite` — normalized client state and sync events
- `pi-sessions/` — Pi JSONL session history, authoritative for agent context
- `attachments/` — private originals and thumbnails
- `repository-icons/` — private repository icon copies

Back up SQLite with its online backup command and copy Pi sessions and attachments together:

```sh
sqlite3 "$HOME/Library/Application Support/Luna Server/luna.sqlite" \
  ".backup '$HOME/luna-backup.sqlite'"
```

## Citadel

Luna includes [`integrations/citadel/citadel.service.json`](../integrations/citadel/citadel.service.json).
Add this entry to Citadel's ignored `config/services.local.json`:

```json
{
  "manifest": "/absolute/path/to/luna/integrations/citadel/citadel.service.json",
  "publicUrl": "https://your-mac.example.ts.net:8447",
  "environmentFile": "~/.config/luna/server.env"
}
```

Render and validate Citadel configuration before activating the Luna service. Do not stop or
restart the Citadel supervisor or unrelated services; Luna supports SIGTERM for a service-scoped,
graceful shutdown. Luna's readiness check is `http://127.0.0.1:9870/v1/health/ready`. Preserve the previous binary and state
until another tailnet device has completed pairing, conversation creation, streaming, image
upload, and reconnection checks.

## Tailscale Serve

Create a dedicated HTTPS listener that proxies to Luna's loopback port:

```sh
tailscale serve --bg --https=8447 http://127.0.0.1:9870
```

Then verify:

```sh
tailscale serve status
curl --fail http://127.0.0.1:9870/v1/health/ready
curl --fail https://your-mac.example.ts.net:8447/v1/health/ready
```

Set `LUNA_PUBLIC_ORIGIN` to the exact HTTPS origin and
`LUNA_ALLOWED_TAILNET_LOGINS` to a comma-separated allowlist. Tailscale identity headers are
accepted only when they match that allowlist.

## Rollback

1. Leave the Tailscale Serve route intact.
2. Stop Luna through Citadel.
3. Restore the previous release binary and, only if a migration requires it, the coordinated
   SQLite/Pi-session/attachment backup.
4. Start Luna through Citadel and verify loopback health before testing the tailnet URL.
5. Inspect Citadel logs and retain failed state for diagnosis.
