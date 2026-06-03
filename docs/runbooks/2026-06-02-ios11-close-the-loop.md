# IOS-11 Runbook — Close the Loop (iOS app ↔ hosted assistant)

**Goal:** Drive a real conversation from the iOS simulator to the hosted `assistant@localhost`
agent over a DB-backed local server, end to end: signup → handshake → "Chat with Assistant"
→ streamed reply.

**Key invariant:** the server's domain must equal the host the app connects to. We run the
server with `--identity localhost`, which sets `server_domain = localhost`. Therefore:

- the hosted agent is provisioned as **`assistant@localhost`**,
- signup binds the human key as **`username@localhost`**,
- the app connects to **`ws://localhost:8080`**, so the handshake identity is
  `username@localhost` — the binding gate matches.

The server's `--mode human` also opens a federation mTLS listener on `:8443` and starts an
interactive REPL (connected as `alice@localhost`). **Neither is used by the app** — the
simulator talks to the plain-TCP WebSocket listener on `:8080`. The cert flags
(`--cert/--key/--ca-cert`) are required by `--mode human` but the local WS listener is plain
TCP and does not validate the cert CN, so the repo's dev certs are fine for a localhost run.

---

## Steps

### 1. Start Postgres

```bash
docker compose -f server/docker-compose.yml up -d postgres
```

This brings up Postgres with DB `ainm`, user `ainm`, password `dev_password` on
`localhost:5432` — matching the default `AINM_DATABASE__URL`
(`postgres://ainm:dev_password@localhost:5432/ainm`). A local Postgres works too, as long as
the DB referenced by `AINM_DATABASE__URL` exists. Migrations apply automatically on connect.

### 2. Start the server

```bash
AINM_LLM__API_KEY=sk-... make dogfood
```

> **No OpenAI key?** Launch with `AINM_LLM__MOCK=1 make dogfood` (or set `AINM_LLM__MOCK=1`
> in the environment before any dogfood command). The hosted agent then replies with a fixed
> canned message instead of calling a real LLM — enough to see the full handshake → agent →
> streamed delivery loop render in the app. `AINM_LLM__MOCK` takes precedence over
> `AINM_LLM__API_KEY`; unset it to use the real model. Dev/test only.

`make dogfood` runs `scripts/dogfood.sh`, which execs:

```
cargo run --bin ainm-server -- \
  --mode human --port 8080 --identity localhost \
  --cert certs/server-a.pem --key certs/server-a-key.pem --ca-cert certs/ca.pem
```

(from inside the `server/` directory).

Confirm in the log:

- `[db] PostgreSQL connected and migrations applied`
- `[hosted-agent] provisioned assistant@localhost (enabled=true)`
- `[email] no AINM_EMAIL__RESEND_API_KEY; using mock email sender (codes logged)` —
  `scripts/dogfood.sh` unsets `AINM_EMAIL__RESEND_API_KEY`, so OTPs are logged, not emailed.
- `[llm] Using OpenAI-compatible client` (if `AINM_LLM__API_KEY` is set). Without the key you
  get the mock LLM client and the loop runs but produces no substantive reply.
- The federation banner: `Local:  ws://0.0.0.0:8080/ws`.

> The REPL prompt that appears belongs to the in-process `alice@localhost` client — leave it
> idle. The WebSocket server runs in parallel; that is what the app connects to. Type `/quit`
> in the REPL to shut the whole server down when you're done.

### 3. Configure the app

In the iOS app onboarding, set the server URL to:

```
ws://localhost:8080
```

Plain `ws://` (not `wss://`): the local server's TLS is on the separate federation port
(`:8443`), and the WS listener on `:8080` is plain TCP. The iOS simulator reaches the host
machine via `localhost`.

### 4. Sign up

Enter email + username and submit. Because the mock email sender is active, **the OTP is
printed to the server log** (the terminal running `make dogfood`) rather than emailed. Read
the code from the log, enter it in the app, then set a PIN. This binds the device key as
`username@localhost`.

### 5. Connect (handshake)

On connect, the app sends `ainm.initialize` signed by the bound key. The server verifies the
signature against the registered `username@localhost` binding and checks the timestamp
(±300s skew). The simulator's clock equals the host clock, so skew is not a concern.

### 6. Open the assistant thread

Tap **"Chat with Assistant"**. A thread opens addressed to `assistant@localhost` (the
provisioned hosted agent). Sent messages carry `to: assistant@localhost`.

### 7. Send a message and watch the reply

Send: `What's 2+2?`

Expected on-screen and in the log:

- `ainm.stream.start` then a series of `ainm.stream.delta` events render the reply live,
- a final `ainm.message.deliver` with the collapsed, signed message.

### 8. Record the result

Capture: success / failure, end-to-end latency, and any errors (with a tail of the server
log). Note whether a memory briefing or any tool calls appeared in the reply.

---

## Troubleshooting

**Handshake rejected (binding / identity error).**
The bound address must equal `username@<server_domain>` and the handshake identity must equal
`username@<host-from-serverURL>`. If they differ, the server rejects the connect. Ensure the
server ran with `--identity localhost` (→ `server_domain = localhost`) AND the app's server
URL is `ws://localhost:8080`. If you signed up against a server started with a different
`--identity`, the old binding won't match — re-provision (fresh DB or new signup) so the key
binds to `localhost`.

**No reply from the assistant.**
`AINM_LLM__API_KEY` is unset → the server uses the mock LLM client. The loop runs and you may
see stream events, but no substantive answer. Restart with `AINM_LLM__API_KEY=sk-... make dogfood`.

**Connection refused / cannot connect.**
The server isn't up, or the app is pointed at `wss://` instead of `ws://`. The `:8080`
listener is plain `ws://`. Confirm the banner shows `Local: ws://0.0.0.0:8080/ws` and nothing
else holds port 8080 (`lsof -i :8080`).

**Database errors / server exits at startup.**
Postgres isn't running or `AINM_DATABASE__URL` points at a DB that doesn't exist. Start it
with `docker compose -f server/docker-compose.yml up -d postgres`, or export a corrected
`AINM_DATABASE__URL` before `make dogfood`. The default is
`postgres://ainm:dev_password@localhost:5432/ainm`. Watch for
`[db] PostgreSQL connected and migrations applied` in the log; a `[db] ... init failed`
warning means the server fell back to in-memory storage and the hosted agent will NOT be
provisioned (no `assistant@localhost`).

**No OTP in the log.**
The mock email sender only activates when `AINM_EMAIL__RESEND_API_KEY` is unset. The script
unsets it; if you exported a Resend key in this shell some other way, the real sender is used
and the code is emailed instead of logged.
