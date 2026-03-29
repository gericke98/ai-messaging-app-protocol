#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

echo "Generating AINM demo federation certificates..."

# 1. Generate CA key and self-signed cert (10 year validity)
openssl genpkey -algorithm ED25519 -out ca-key.pem
openssl req -new -x509 -key ca-key.pem -out ca.pem -days 3650 \
  -subj "/CN=AINM Demo CA/O=AINM/C=US"

# 2. Generate Server A key, CSR, and CA-signed cert
openssl genpkey -algorithm ED25519 -out server-a-key.pem
openssl req -new -key server-a-key.pem -out server-a.csr \
  -subj "/CN=server-a.local/O=AINM Demo/C=US"
openssl x509 -req -in server-a.csr -CA ca.pem -CAkey ca-key.pem \
  -CAcreateserial -out server-a.pem -days 365 \
  -extfile <(printf "subjectAltName=DNS:server-a.local,DNS:server-a,DNS:localhost")
rm -f server-a.csr

# 3. Generate Server B key, CSR, and CA-signed cert
openssl genpkey -algorithm ED25519 -out server-b-key.pem
openssl req -new -key server-b-key.pem -out server-b.csr \
  -subj "/CN=server-b.local/O=AINM Demo/C=US"
openssl x509 -req -in server-b.csr -CA ca.pem -CAkey ca-key.pem \
  -CAcreateserial -out server-b.pem -days 365 \
  -extfile <(printf "subjectAltName=DNS:server-b.local,DNS:server-b,DNS:localhost")
rm -f server-b.csr

rm -f ca.srl
echo "Done. Files: ca.pem ca-key.pem server-a.pem server-a-key.pem server-b.pem server-b-key.pem"
