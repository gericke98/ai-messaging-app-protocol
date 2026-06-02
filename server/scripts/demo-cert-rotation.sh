#!/usr/bin/env bash
# DEMO-04: Certificate Rotation
#
# Proves that when a server's TLS certificate is rotated, federation
# continues without downtime because the tls_fingerprints array
# supports rotation overlap and TOFU auto-updates the pin.
#
# Scenario:
#   1. Start both servers and verify federation works
#   2. Generate a new certificate for server-b
#   3. Compute fingerprints for both old and new certs
#   4. Update server-b's code/config to serve tls_fingerprints: [new, old]
#   5. Restart server-b with the new certificate
#   6. Send a message and verify federation still works
#   7. Check logs for TOFU auto-update
#
# Prerequisites:
#   - Docker and docker-compose installed
#   - OpenSSL installed
#   - Run from the server/ directory
#
# Manual alternative (no Docker):
#   1. Start both servers as in DEMO-03 manual instructions
#   2. cd certs && bash generate.sh rotate-server-b
#   3. Note the two fingerprints printed
#   4. Stop server-b (Ctrl+C)
#   5. Restart server-b with --cert certs/server-b-rotated.pem --key certs/server-b-rotated-key.pem --additional-fingerprint <old_fingerprint>
#   6. The .well-known endpoint will serve both fingerprints: [new_fp, old_fp]
#   7. server-a's TOFU check will see the old pinned fingerprint in tls_fingerprints
#      and auto-update its pin to the new cert's fingerprint

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DEMO_DIR="$(dirname "$SCRIPT_DIR")"
cd "$DEMO_DIR"

echo "========================================"
echo "  DEMO-04: Certificate Rotation"
echo "========================================"
echo ""

# Step 1: Start both servers
echo "[Step 1] Starting both servers..."
docker-compose up -d --build
echo "Waiting 5s for servers to initialize..."
sleep 5

# Step 2: Verify federation works
echo ""
echo "[Step 2] Verifying federation works before rotation..."
echo "server-a logs:"
docker-compose logs --tail=5 server-a 2>&1 | grep -E "federation|relay" || echo "(checking...)"
echo ""

# Step 3: Generate rotated certificate
echo "[Step 3] Generating rotated certificate for server-b..."
cd certs
bash generate.sh rotate-server-b
cd "$DEMO_DIR"
echo ""

# Step 4: Compute fingerprints
echo "[Step 4] Certificate fingerprints:"
OLD_FP=$(openssl x509 -in certs/server-b.pem -outform DER 2>/dev/null | shasum -a 256 | awk '{print "sha256:" $1}')
NEW_FP=$(openssl x509 -in certs/server-b-rotated.pem -outform DER 2>/dev/null | shasum -a 256 | awk '{print "sha256:" $1}')
echo "  Current (old): $OLD_FP"
echo "  Rotated (new): $NEW_FP"
echo ""

# Step 5: Swap certificate and restart server-b with BOTH fingerprints
# CRITICAL: For TOFU auto-update to work, server-b must serve tls_fingerprints: [new, old]
# during the rotation window. The --additional-fingerprint flag adds the old fingerprint
# alongside the new cert's fingerprint in the .well-known response.
echo "[Step 5] Swapping server-b certificate with dual-fingerprint rotation window..."
cp certs/server-b.pem certs/server-b-backup.pem
cp certs/server-b-key.pem certs/server-b-key-backup.pem
cp certs/server-b-rotated.pem certs/server-b.pem
cp certs/server-b-rotated-key.pem certs/server-b-key.pem
echo "Certificate files swapped."
echo ""
echo "Restarting server-b with --additional-fingerprint to serve both old and new fingerprints..."
echo "This enables TOFU auto-update: server-a sees [new_fp, old_fp] in tls_fingerprints,"
echo "recognizes old_fp as the pinned one, and auto-updates its pin to new_fp."
# Stop server-b, restart with additional_fingerprint CLI arg
docker-compose stop server-b
# Temporarily modify docker-compose to add --additional-fingerprint flag
# In production, this would be a config change; for demo we restart manually
docker-compose run -d --name server-b-rotated \
  --service-ports server-b \
  --additional-fingerprint "$OLD_FP"
echo "Waiting 5s for server-b to restart with dual fingerprints..."
sleep 5
# Also restart server-a so it re-resolves server-b identity (picks up [new, old] fingerprints)
docker-compose restart server-a
echo "Waiting 5s for server-a to re-resolve identities..."
sleep 5

# Step 6: Verify federation continues
echo ""
echo "[Step 6] Verifying federation continues after cert rotation..."
echo "Send a message in the server-a REPL to test federation."
echo "Press Enter after sending a message..."
read -r

echo "server-a logs (look for TOFU pinning and successful relay):"
docker-compose logs --tail=20 server-a 2>&1 | grep -E "tofu|relay.*Success" || echo "(no matching log lines yet)"
echo ""
echo "server-b logs:"
docker logs server-b-rotated 2>&1 | tail -10 | grep -E "agent.*Received|federation" || echo "(checking...)"
echo ""

# Step 7: Restore original certificate
echo "[Step 7] Restoring original server-b certificate..."
docker stop server-b-rotated 2>/dev/null || true
docker rm server-b-rotated 2>/dev/null || true
cp certs/server-b-backup.pem certs/server-b.pem
cp certs/server-b-key-backup.pem certs/server-b-key.pem
rm -f certs/server-b-backup.pem certs/server-b-key-backup.pem
rm -f certs/server-b-rotated.pem certs/server-b-rotated-key.pem
echo "Original certificate restored."
echo ""

echo "========================================"
echo "  DEMO-04 Complete"
echo "========================================"
echo ""
echo "Expected results:"
echo "  - server-a logs show: '[tofu] First contact -- pinned server-b.local -> sha256:...'"
echo "  - After rotation, federation relay succeeds (server-a relays to server-b)"
echo "  - server-b agent receives and responds to the message"
echo ""
echo "Key mechanism:"
echo "  - server-b restarts with new cert + --additional-fingerprint (old fp)"
echo "  - server-b serves tls_fingerprints: [new_fp, old_fp] in .well-known"
echo "  - server-a's TOFU check sees old_fp in the array, auto-updates pin to new_fp"
echo "  - Federation continues seamlessly"
echo ""
echo "To clean up: docker-compose down -v"
