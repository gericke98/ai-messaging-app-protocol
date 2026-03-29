#!/usr/bin/env bash
# DEMO-03: Partition Healing
#
# Proves that when a remote server goes offline, messages are persisted
# to disk and automatically delivered when it comes back online.
#
# Scenario:
#   1. Start both servers (server-a human, server-b agent)
#   2. Send a message from server-a to verify federation works
#   3. Stop server-b (simulating partition)
#   4. Send messages from server-a (they queue in server-a's persistent queue)
#   5. Restart server-b
#   6. Verify queued messages are delivered to server-b
#
# Prerequisites:
#   - Docker and docker-compose installed
#   - Run from the demo/ directory
#
# Manual alternative (no Docker):
#   Terminal 1: cargo run -- --mode human --identity server-a.local --cert certs/server-a.pem --key certs/server-a-key.pem --ca-cert certs/ca.pem --federation-port 8443 --remote server-b.local:9443 --data-dir ./data-a
#   Terminal 2: cargo run -- --mode agent --identity server-b.local --cert certs/server-b.pem --key certs/server-b-key.pem --ca-cert certs/ca.pem --federation-port 9443 --remote server-a.local:8443 --port 9080 --data-dir ./data-b
#   Then Ctrl+C terminal 2, send messages in terminal 1, restart terminal 2.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DEMO_DIR="$(dirname "$SCRIPT_DIR")"
cd "$DEMO_DIR"

echo "========================================"
echo "  DEMO-03: Partition Healing"
echo "========================================"
echo ""

# Step 1: Build and start both servers
echo "[Step 1] Starting both servers..."
docker-compose up -d --build
echo "Waiting 5s for servers to initialize..."
sleep 5

# Step 2: Verify federation works (check logs for successful relay)
echo ""
echo "[Step 2] Verifying federation is working..."
echo "Check server-a logs for relay activity:"
docker-compose logs server-a 2>&1 | tail -5
echo ""

# Step 3: Stop server-b (simulate partition)
echo "[Step 3] Stopping server-b (simulating network partition)..."
docker-compose stop server-b
echo "server-b stopped."
echo ""

# Step 4: Send messages (they will queue)
echo "[Step 4] Messages sent while server-b is down will queue in server-a's persistent store."
echo "In the server-a REPL, type a message and press Enter."
echo "The message will be enqueued (check server-a logs for '[relay] Enqueued message for server-b.local (persistent)')."
echo ""
echo "Press Enter when you have sent at least one message..."
read -r

# Step 5: Restart server-b
echo "[Step 5] Restarting server-b..."
docker-compose start server-b
echo "Waiting 10s for server-b to initialize and queued messages to drain..."
sleep 10

# Step 6: Verify delivery
echo ""
echo "[Step 6] Checking server-b received the queued messages..."
echo "server-b logs (look for received messages):"
docker-compose logs --tail=20 server-b 2>&1 | grep -E "agent.*Received|relay.*Successfully" || echo "(no matching log lines yet)"
echo ""
echo "server-a logs (look for successful relay):"
docker-compose logs --tail=20 server-a 2>&1 | grep -E "relay.*Successfully" || echo "(no matching log lines yet)"
echo ""

echo "========================================"
echo "  DEMO-03 Complete"
echo "========================================"
echo ""
echo "Expected results:"
echo "  - server-a logs show: '[relay] Enqueued message for server-b.local (persistent)'"
echo "  - After restart, server-a logs show: '[relay] Successfully relayed to server-b.local'"
echo "  - server-b logs show the agent received and responded to the queued message"
echo ""
echo "To clean up: docker-compose down -v"
