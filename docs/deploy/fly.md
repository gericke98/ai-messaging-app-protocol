# Fly.io deployment runbook (M2)

> **STATUS: prepared 2026-06-10, UNVERIFIED — no Fly account exists yet.**
> Everything below is written against the committed `fly.toml`,
> `docker/fly-entrypoint.sh`, and workspace `Dockerfile`. The first real
> `flyctl deploy` will be babysat and this doc corrected against reality.

## Decisions baked in

- **Region `scl` (Santiago)** — founding users are in Chile; ~$10/mo for a
  shared-cpu-1x/512MB always-on machine.
- **No Fly Managed Postgres** (it is the $38/mo cost problem). Default:
  [Neon](https://neon.tech) free tier, region `aws-sa-east-1` (São Paulo,
  closest to scl). Any Postgres 16+ URL works — migrations are embedded and
  run at boot.
- **Single machine, no autostop** — WebSockets must stay up
  (`auto_stop_machines = "off"`, `min_machines_running = 1`).
- **Fly edge terminates public TLS** — the app speaks plain HTTP on :8080
  internally; clients use `wss://<app>.fly.dev/ws`. The federation mTLS
  listener (:8443) binds but is not publicly exposed (single-home-server v1).

## One-time setup

```sh
brew install flyctl
flyctl auth signup            # or: flyctl auth login
flyctl apps create ainm-server          # name must match fly.toml `app`
flyctl volumes create ainm_data --region scl --size 1
```

### Database (Neon)

1. Create a Neon project (Postgres 16, region `aws-sa-east-1`).
2. Copy the pooled connection string.

### Federation certs (required flags, unused listener)

`run_federation` requires `--cert/--key/--ca-cert` even though v1 never
federates. Generate a throwaway CA + server cert and ship them as secrets so
no private key is baked into the image:

```sh
cd server/certs && ./generate.sh    # or reuse the committed dev outputs' SCRIPT, not the committed keys
flyctl secrets set \
  AINM_FED_CERT_B64="$(base64 -i server-a.pem)" \
  AINM_FED_KEY_B64="$(base64 -i server-a-key.pem)" \
  AINM_FED_CA_B64="$(base64 -i ca.pem)"
```

> Do NOT reuse the committed dev certs for anything trust-bearing; they are
> fine here only because the federation listener has no peers in v1. Rotate
> before enabling real federation.

### Application secrets

`AINM_ENV=production` is set in `fly.toml`; `prod_guard` then refuses to boot
unless the real secrets exist:

```sh
flyctl secrets set \
  AINM_DATABASE__URL='postgres://...neon.../ainm?sslmode=require' \
  AINM_TOKEN_SECRET="$(openssl rand -hex 32)" \
  AINM_LLM_ENCRYPTION_KEY="$(openssl rand -hex 32)" \
  AINM_LLM__API_KEY='sk-ant-...' \
  AINM_EMAIL__API_KEY='...'            # email provider key (verification codes)
```

Optional:

```sh
flyctl secrets set AINM_OTEL__ENDPOINT='https://<collector>:4318'   # span export
flyctl secrets set AINM_APNS__KEY_PATH=... AINM_APNS__KEY_ID=... \
  AINM_APNS__TEAM_ID=... AINM_APNS__TOPIC=com.ainm.messenger        # push (M4, needs Apple acct)
flyctl secrets set AINM_INSTANT_REGISTRATION=0                      # kill switch if abused
```

`AINM_IDENTITY` (env, not secret) defaults to `<app>.fly.dev`; set it when a
custom domain exists (`flyctl certs add chat.example.com` first).

## Deploy

```sh
flyctl deploy
flyctl logs            # watch boot: prod_guard OK → migrations → listeners up
curl -sf https://ainm-server.fly.dev/health/ready && echo OK
```

## Post-deploy verification checklist

1. `/health/ready` returns 200 (DB-aware check).
2. iOS app onboarding against `wss://ainm-server.fly.dev/ws` (signup → email
   code → key bind → connect).
3. **M5 exit criterion re-run**: `npx @ainm/connector init --server
   wss://ainm-server.fly.dev` completes in ≤60s over real TLS (the local run
   was 0.16s).
4. Rate limiting visible: burst >100 rapid requests → 429s.
5. `flyctl logs` shows no `[push] APNs delivery ... failed` spam (Noop until
   APNs secrets are set — expected).
