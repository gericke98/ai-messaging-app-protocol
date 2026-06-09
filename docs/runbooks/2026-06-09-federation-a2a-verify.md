# Runbook — Federation A2A Integration (P3a) Live Verify

**What this proves:** a message originating on server **A** is relayed over mTLS to server **B** and delivered, on B, **through `deliver_message`** — i.e. it passes B's P2 consent gate + A2A hop guard and reaches a **connected connector agent** registered in B's `AgentRegistry`. This is the integration substrate P3a built; cross-server agent→agent *addressing* semantics are P3b.

**Automated coverage (runs in CI, no Postgres):** `cargo test -p ainm-server federation::tests` — the in-process `relay_handler → deliver_message → connected-agent` test (`server/src/federation.rs`, added in P3a Task 4). It constructs a `FederationState` with the shared `AgentRegistry`, a signed `ainm.relay` payload, and asserts a connected agent's channel receives the delivery. This runbook is the **transport-level** (real mTLS, two processes) complement, which CI does not automate.

---

## Prerequisites

- Postgres reachable; export `AINM_DATABASE__URL` (each server provisions its own accounts).
- Cert fixtures at `server/certs/` (`ca.pem`, `server-a.pem`, `server-a-key.pem`, `server-b.pem`, `server-b-key.pem`); regenerate via `server/certs/generate.sh` if absent.
- Built binary: `cargo build -p ainm-server`.

## Topology

- **Server A** — domain `server-a.local`, the origin (human side).
- **Server B** — domain `server-b.local`, hosts a connected connector agent `beta@server-b.local`.
- Each server runs two listeners: a plain WS listener (`--port`) and an mTLS federation listener (`--federation-port`).

## Steps

1. **Start server A** (origin / human side):
   ```
   target/debug/ainm-server --mode human \
     --identity server-a.local \
     --port 8080 --federation-port 8443 \
     --cert server/certs/server-a.pem --key server/certs/server-a-key.pem --ca-cert server/certs/ca.pem \
     --remote server-b.local
   ```
   (Adjust `--remote` to B's reachable host:fed-port form per `--remote` parsing in `main.rs`.)

2. **Start server B** (hosts the connector agent):
   ```
   target/debug/ainm-server --mode agent \
     --identity server-b.local \
     --port 9080 --federation-port 9443 \
     --cert server/certs/server-b.pem --key server/certs/server-b-key.pem --ca-cert server/certs/ca.pem \
     --remote server-a.local
   ```

3. **Connect a connector agent to B** (`beta@server-b.local`) via the TypeScript SDK (`@ainm/connector`) against `ws://<B>:9080/ws`, with a bound account on B. Confirm B logs `Agent registered: beta@server-b.local`.

4. **On A**, as a human, create a federated thread (thread_id `server-a.local:<uuid>`) and **first address `beta@server-b.local`** so B records the human invitation (P2 consent requires the human to have invited the agent). Then send a message `to: beta@server-b.local`.

5. **Observe on B:** the relay arrives at `POST /ainm/v1/relay`; B verifies the server signature + the sender's per-message signature (lazily fetching the sender's key from A's `/.well-known/ainm/participant/{sender}` if needed), then routes through `deliver_message`. **Expected:** the connected `beta` connector receives the message over its WebSocket (its `onMessage` fires).

## Pass criteria

- `beta@server-b.local` (a real connected connector on B) receives the message originated on A.
- If `beta` was **not** human-invited on B first, the message is **denied** by the consent gate and a `system@local` notice is delivered to humans on B (no silent drop).
- No panics; both federation listeners stay up.

## Notes / known limits (P3a scope)

- **Hosted assistant over federation is NOT spawned** in P3a (the relay path passes `state_for_hosted_turn: None`); only connected connector agents receive federated delivery. Hosted-target-over-federation is deferred.
- **`sender_type` is derived** on B from the registered identity `type_` (resolved during signature verification), not carried in the relay envelope. P3b may carry it explicitly.
- **Outbound cross-server agent→agent addressing** (an agent on A addressing an agent on B by `to`, with reply-routing-home and second-server hop re-accounting) is **P3b**, not P3a. P3a makes the inbound seam real; P3b adds the A2A semantics.
- `local_delivery_tx` is still sent alongside `deliver_message` (consumed by the reference echo loop / backfill); in a real deployment nothing else consumes it.
