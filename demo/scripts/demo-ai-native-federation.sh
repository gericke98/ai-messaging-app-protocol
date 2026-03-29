#!/usr/bin/env bash
# DEMO-05: AI-Native Federation
#
# Demonstrates all four AI-native federation features (AINF-01 through AINF-04)
# across two federated servers, proving AINM is fundamentally different from
# generic messaging federation.
#
# Features proven:
#   1. Agent Card discovery across servers (AINF-01)
#   2. Tool call visibility across federation (AINF-02)
#   3. Memory context propagation (AINF-03)
#   4. Real-time streaming across federation (AINF-04)
#
# Prerequisites:
#   - Rust toolchain (cargo)
#   - Run from anywhere -- script resolves paths automatically
#
# Can also be run via Docker:
#   cd demo && docker-compose up

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DEMO_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_DIR="$(dirname "$DEMO_DIR")"

echo "============================================"
echo "  AINM AI-Native Federation Demo (Phase 9)"
echo "============================================"
echo ""
echo "This demo proves four AI-native features work across federation:"
echo "  1. Agent Card discovery across servers (AINF-01)"
echo "  2. Tool call visibility across federation (AINF-02)"
echo "  3. Memory context propagation (AINF-03)"
echo "  4. Real-time streaming across federation (AINF-04)"
echo ""

# Build
echo "[1/3] Building demo..."
cd "$DEMO_DIR"
cargo build --quiet 2>&1
echo "  Build complete."
echo ""

# Generate certificates if not present
CERTS_DIR="$DEMO_DIR/certs"
if [ ! -f "$CERTS_DIR/server-a.pem" ]; then
    echo "[2/3] Generating certificates..."
    bash "$CERTS_DIR/generate.sh" 2>/dev/null || true
else
    echo "[2/3] Certificates already present."
fi
echo ""

# Launch instructions
echo "[3/3] Launching federated servers..."
echo ""

echo "  Start TWO terminals and run:"
echo ""
echo "  Terminal 1 (Server A - Human REPL):"
echo "    cd $DEMO_DIR"
echo "    cargo run -- --mode human --identity server-a.local --port 8080 \\"
echo "      --federation-port 8443 \\"
echo "      --ca-cert certs/ca.pem \\"
echo "      --cert certs/server-a.pem \\"
echo "      --key certs/server-a-key.pem \\"
echo "      --remote server-b.local:9443 \\"
echo "      --data-dir ./data-a"
echo ""
echo "  Terminal 2 (Server B - Headless Agent):"
echo "    cd $DEMO_DIR"
echo "    cargo run -- --mode agent --identity server-b.local --port 9080 \\"
echo "      --federation-port 9443 \\"
echo "      --ca-cert certs/ca.pem \\"
echo "      --cert certs/server-b.pem \\"
echo "      --key certs/server-b-key.pem \\"
echo "      --remote server-a.local:8443 \\"
echo "      --data-dir ./data-b"
echo ""
echo "  NOTE: Add '127.0.0.1 server-a.local server-b.local' to /etc/hosts"
echo "        if not already present."
echo ""
echo "============================================"
echo "  Demo Scenario (type in Terminal 1 REPL)"
echo "============================================"
echo ""
echo "Step 1: DISCOVER (AINF-01)"
echo "  alice> /discover helper@server-b.local"
echo "  Expected: Agent card with name, description, capabilities"
echo "  Proves:   Cross-server agent card resolution via HTTPS"
echo ""
echo "Step 2: BASELINE MESSAGE"
echo "  alice> Hello across federation!"
echo "  Expected: Echo response from helper@server-b.local"
echo "  Proves:   Basic federated message relay works"
echo ""
echo "Step 3: STREAMING (AINF-04 / DEMO-05)"
echo "  alice> hello"
echo "  Expected: Streamed response arriving token-by-token from server B"
echo "  Watch:    StreamStart -> Delta -> Delta -> ... -> StreamEnd"
echo "  Proves:   Real-time streaming works across federation boundary"
echo ""
echo "Step 4: TOOL CALL VISIBILITY (AINF-02)"
echo "  alice> weather"
echo "  Expected: ToolCallStarted -> ToolCallProgress -> ToolCallCompleted"
echo "  Watch:    Each event arrives individually across federation"
echo "  Proves:   Tool call lifecycle transparency across servers"
echo ""
echo "Step 5: MEMORY CONTEXT (AINF-03)"
echo "  alice> memory"
echo "  Expected: Agent responds with memory context acknowledgment"
echo "  Watch:    Context frames logged on Terminal 2 (server B)"
echo "  Proves:   Memory/context propagation across federation"
echo ""
echo "Step 6: EXIT"
echo "  alice> /quit"
echo ""
echo "============================================"
echo "  Success Criteria"
echo "============================================"
echo ""
echo "  [ ] /discover resolves remote agent capabilities (AINF-01)"
echo "  [ ] Streaming renders token-by-token, not all-at-once (AINF-04)"
echo "  [ ] Tool call lifecycle events visible in sequence (AINF-02)"
echo "  [ ] Memory context logged on server B Terminal 2 (AINF-03)"
echo "  [ ] All responses come from helper@server-b.local (cross-server)"
echo ""
echo "============================================"
echo ""
echo "Demo script ready. Start the two terminals above and follow the steps."
