# Single-Node Production Deployment Runbook

## Configure

1. `cp .env.production.example .env.production`
2. Fill in every value:
   - `AINM_TOKEN_SECRET` — `openssl rand -base64 48` (>= 32 bytes; keep STABLE across deploys —
     rotating it invalidates in-flight enrollment/recovery tokens).
   - `AINM_LLM_ENCRYPTION_KEY` — `openssl rand -base64 32`.
   - `AINM_LLM__API_KEY`, `AINM_LLM__MODEL` — the hosted agent's LLM.
   - `AINM_EMAIL__RESEND_API_KEY`, `AINM_EMAIL__FROM` — OTP delivery.
   - `AINM_DATABASE__URL` — Postgres (defaults to the compose `postgres` service).
3. Keep `.env.production` out of git (already in `.gitignore`).

The server **refuses to boot** in production (`AINM_ENV=production`) if any required value is
missing/unsafe — read the `[config]` lines on stderr and fix them. `AINM_LLM__MOCK` must NOT be set.

## Build & run

```bash
docker compose -f docker-compose.prod.yml up --build -d
docker compose -f docker-compose.prod.yml logs -f app
```

Health: `curl -sf http://<host>:8080/health/ready` returns 200 once Postgres is up and migrations ran.

## Stop / restart (graceful)

```bash
docker compose -f docker-compose.prod.yml stop      # or: restart
```

On SIGTERM the server stops accepting new connections and drains in-flight agent turns up to
`AINM_SHUTDOWN_DRAIN_SECS` (default 25s). `stop_grace_period` (30s) exceeds that so Docker does not
SIGKILL mid-drain. The logs show `received SIGTERM; beginning graceful shutdown` → `drained cleanly`.

## Sandbox caveat (bubblewrap)

The image installs `bubblewrap` so MCP servers configured with `sandbox = "bubblewrap"` are isolated.
bubblewrap needs unprivileged user namespaces; some container hosts require enabling host userns or
adding a capability. If sandboxed MCP servers fail to launch, that is the cause — enable host userns on
the host or grant the needed capability. (Hardening-only MCP servers, `sandbox = "none"`, are unaffected.)

## Federation demo (separate, unchanged)

`server/docker-compose.yml` is the two-server federation demo, not a production deploy. Its build
context now points at the repo root (`context: ..`, `dockerfile: Dockerfile`) so it builds against the
same Dockerfile.
