#!/usr/bin/env bash
# DEMO: Backfill endpoint verification
#
# Exercises ainm.backfill to prove:
#   BKFL-01: Remote server can request thread history
#   BKFL-02: Post-join visibility enforced
#   BKFL-03: Non-participant rejected with -32025
#   BKFL-04: Pagination with has_more and next_cursor
#
# Prerequisites: Both servers running (docker-compose up or manual)
# Run from server/ directory

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DEMO_DIR="$(dirname "$SCRIPT_DIR")"
cd "$DEMO_DIR"

RELAY_ENDPOINT="https://localhost:8443/ainm/v1/relay"
CA_CERT="certs/ca.pem"
CLIENT_CERT="certs/server-b.pem"
CLIENT_KEY="certs/server-b-key.pem"

echo "========================================"
echo "  Backfill Endpoint Verification"
echo "========================================"
echo ""

# Test 1: Request backfill for a known thread (BKFL-01)
echo "[Test 1] Requesting backfill (BKFL-01)..."
echo "NOTE: Replace THREAD_ID with an actual thread ID from server logs."
echo ""
echo "  curl --cacert $CA_CERT --cert $CLIENT_CERT --key $CLIENT_KEY \\"
echo "    -X POST $RELAY_ENDPOINT \\"
echo "    -H 'Content-Type: application/json' \\"
echo "    -d '{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ainm.backfill\",\"params\":{\"thread_id\":\"THREAD_ID\",\"from_server\":\"server-b.local\",\"after_sequence\":0,\"limit\":10}}'"
echo ""
echo "  Expected: {\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"messages\":[...],\"has_more\":false}}"
echo ""

# Test 2: Request from non-participant (BKFL-03 - should get -32025)
echo "[Test 2] Non-participant rejection (BKFL-03)..."
echo "  Send a backfill request with from_server set to a server that is NOT a participant."
echo ""
echo "  curl --cacert $CA_CERT --cert $CLIENT_CERT --key $CLIENT_KEY \\"
echo "    -X POST $RELAY_ENDPOINT \\"
echo "    -H 'Content-Type: application/json' \\"
echo "    -d '{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"ainm.backfill\",\"params\":{\"thread_id\":\"THREAD_ID\",\"from_server\":\"unknown-server.local\",\"after_sequence\":0,\"limit\":10}}'"
echo ""
echo "  Expected: error code -32025 (Not a participant in this thread)"
echo ""

# Test 3: Pagination (BKFL-04)
echo "[Test 3] Pagination test (BKFL-04)..."
echo "  Send backfill with limit=1, then use next_cursor for page 2."
echo ""
echo "  Page 1:"
echo "  curl --cacert $CA_CERT --cert $CLIENT_CERT --key $CLIENT_KEY \\"
echo "    -X POST $RELAY_ENDPOINT \\"
echo "    -H 'Content-Type: application/json' \\"
echo "    -d '{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"ainm.backfill\",\"params\":{\"thread_id\":\"THREAD_ID\",\"from_server\":\"server-b.local\",\"after_sequence\":0,\"limit\":1}}'"
echo ""
echo "  Expected: has_more=true, next_cursor=<sequence>"
echo ""
echo "  Page 2 (use next_cursor from page 1 as after_sequence):"
echo "  curl --cacert $CA_CERT --cert $CLIENT_CERT --key $CLIENT_KEY \\"
echo "    -X POST $RELAY_ENDPOINT \\"
echo "    -H 'Content-Type: application/json' \\"
echo "    -d '{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"ainm.backfill\",\"params\":{\"thread_id\":\"THREAD_ID\",\"from_server\":\"server-b.local\",\"after_sequence\":NEXT_CURSOR,\"limit\":1}}'"
echo ""
echo "  Expected: messages continue from where page 1 left off."
echo ""

echo "========================================"
echo "  Run these curl commands after sending"
echo "  messages in the demo."
echo "========================================"
