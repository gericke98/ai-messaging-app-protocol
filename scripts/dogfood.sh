#!/usr/bin/env bash
# Launch a DB-backed human-mode AINM server for iOS dogfooding (IOS-11).
#
# What this gives you:
#   - Local WebSocket listener on  ws://localhost:8080  (plain TCP, no TLS) — the iOS
#     simulator connects here. server_domain == localhost, so accounts bind as
#     username@localhost and the hosted agent is provisioned as assistant@localhost.
#   - A federation mTLS listener on :8443 (uses the certs below). The simulator does NOT
#     use this; --mode human requires the cert flags to be present so we pass the repo's
#     dev certs. The local WS listener is plain TCP and does not validate the cert CN, so a
#     server-a.local cert is fine for a localhost dogfood.
#   - An interactive REPL in this terminal (connects as alice@localhost). Ignore it — the
#     WS server runs in parallel and is what the app talks to. Type /quit to stop the server.
#
# Prereqs:
#   - A reachable Postgres (see server/docker-compose.yml's `postgres` service, or a local PG)
#     with a database matching AINM_DATABASE__URL. Migrations apply automatically on connect.
#   - Set AINM_LLM__API_KEY for real assistant replies (otherwise a mock LLM client is used
#     and the loop runs but produces no substantive reply).
set -euo pipefail

: "${AINM_DATABASE__URL:=postgres://ainm:dev_password@localhost:5432/ainm}"
export AINM_DATABASE__URL
unset AINM_EMAIL__RESEND_API_KEY 2>/dev/null || true   # -> MockEmailSender logs the OTP code
if [ -z "${AINM_LLM__API_KEY:-}" ]; then
  echo "[dogfood] WARNING: AINM_LLM__API_KEY unset — the assistant loop runs but produces no LLM reply." >&2
fi
echo "[dogfood] DB: $AINM_DATABASE__URL"
echo "[dogfood] server: ws://localhost:8080  (server_domain = localhost; hosted agent = assistant@localhost)"
echo "[dogfood] point the iOS app serverURL at  ws://localhost:8080  ; watch this log for the signup OTP."

SERVER_DIR="$(cd "$(dirname "$0")/../server" && pwd)"
cd "$SERVER_DIR"
exec cargo run --bin ainm-server -- \
  --mode human \
  --port 8080 \
  --identity localhost \
  --cert certs/server-a.pem \
  --key certs/server-a-key.pem \
  --ca-cert certs/ca.pem
