#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

echo "Generating AINM demo federation certificates..."

# 1. Generate CA key and self-signed cert (10 year validity)
# ECDSA P-256 for TLS certs — Ed25519 not supported by macOS Security framework
# (rustls-platform-verifier delegates to system verifier on macOS)
openssl ecparam -genkey -name prime256v1 -noout -out ca-key.pem
openssl req -new -x509 -key ca-key.pem -out ca.pem -days 3650 \
  -subj "/CN=AINM Demo CA/O=AINM/C=US"

# 2. Generate Server A key, CSR, and CA-signed cert
openssl ecparam -genkey -name prime256v1 -noout -out server-a-key.pem
openssl req -new -key server-a-key.pem -out server-a.csr \
  -subj "/CN=server-a.local/O=AINM Demo/C=US"
openssl x509 -req -in server-a.csr -CA ca.pem -CAkey ca-key.pem \
  -CAcreateserial -out server-a.pem -days 365 \
  -extfile <(printf "subjectAltName=DNS:server-a.local,DNS:server-a,DNS:localhost\nextendedKeyUsage=serverAuth,clientAuth")
rm -f server-a.csr

# 3. Generate Server B key, CSR, and CA-signed cert
openssl ecparam -genkey -name prime256v1 -noout -out server-b-key.pem
openssl req -new -key server-b-key.pem -out server-b.csr \
  -subj "/CN=server-b.local/O=AINM Demo/C=US"
openssl x509 -req -in server-b.csr -CA ca.pem -CAkey ca-key.pem \
  -CAcreateserial -out server-b.pem -days 365 \
  -extfile <(printf "subjectAltName=DNS:server-b.local,DNS:server-b,DNS:localhost\nextendedKeyUsage=serverAuth,clientAuth")
rm -f server-b.csr

rm -f ca.srl
echo "Done. Files: ca.pem ca-key.pem server-a.pem server-a-key.pem server-b.pem server-b-key.pem"

# --- Certificate Rotation (for DEMO-04) ---
# Generate a rotated certificate for server-b with a different key
# This simulates a certificate rotation scenario.
if [ "${1:-}" = "rotate-server-b" ]; then
    echo "Generating rotated certificate for server-b..."
    openssl ecparam -genkey -name prime256v1 -noout -out server-b-rotated-key.pem
    openssl req -new -key server-b-rotated-key.pem -out server-b-rotated.csr \
      -subj "/CN=server-b.local/O=AINM Demo/C=US"
    openssl x509 -req -in server-b-rotated.csr -CA ca.pem -CAkey ca-key.pem \
      -CAcreateserial -out server-b-rotated.pem -days 365 \
      -extfile <(printf "subjectAltName=DNS:server-b.local,DNS:server-b,DNS:localhost\nextendedKeyUsage=serverAuth,clientAuth")
    rm -f server-b-rotated.csr ca.srl
    echo "Generated: server-b-rotated.pem, server-b-rotated-key.pem"
    echo "Fingerprint (new):"
    openssl x509 -in server-b-rotated.pem -outform DER | shasum -a 256 | awk '{print "sha256:" $1}'
    echo "Fingerprint (current):"
    openssl x509 -in server-b.pem -outform DER | shasum -a 256 | awk '{print "sha256:" $1}'
fi
