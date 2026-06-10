#!/bin/sh
# Fly.io entrypoint: materialize federation certs from secrets, then exec the server.
#
# run_federation requires --cert/--key/--ca-cert even though single-home-server
# v1 never federates (the mTLS listener binds but receives no peers). The PEMs
# arrive as base64 Fly secrets so no private key is baked into the image:
#   flyctl secrets set AINM_FED_CERT_B64=... AINM_FED_KEY_B64=... AINM_FED_CA_B64=...
# Generate them from server/certs/generate.sh output (see docs/deploy/fly.md).
set -eu

CERT_DIR=/data/certs
mkdir -p "$CERT_DIR"

for pair in "AINM_FED_CERT_B64:server.pem" "AINM_FED_KEY_B64:server-key.pem" "AINM_FED_CA_B64:ca.pem"; do
  var="${pair%%:*}"
  file="${pair##*:}"
  val="$(printenv "$var" || true)"
  if [ -z "$val" ]; then
    echo "[fly-entrypoint] missing secret $var — set it with flyctl secrets" >&2
    exit 1
  fi
  printf '%s' "$val" | base64 -d > "$CERT_DIR/$file"
done
chmod 600 "$CERT_DIR/server-key.pem"

# AINM_IDENTITY defaults to the Fly app hostname; override for a custom domain.
IDENTITY="${AINM_IDENTITY:-${FLY_APP_NAME:-ainm-server}.fly.dev}"

exec ainm-server \
  --mode agent \
  --identity "$IDENTITY" \
  --port 8080 \
  --federation-port 8443 \
  --cert "$CERT_DIR/server.pem" \
  --key "$CERT_DIR/server-key.pem" \
  --ca-cert "$CERT_DIR/ca.pem" \
  --data-dir /data
