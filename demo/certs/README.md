# AINM Demo Federation Certificates

Pre-generated self-signed certificates for the two-server federation demo.

## Files

| File | Purpose |
|------|---------|
| ca.pem | CA certificate (both servers trust this) |
| ca-key.pem | CA private key (only needed to regenerate) |
| server-a.pem | Server A TLS certificate (CN=server-a.local) |
| server-a-key.pem | Server A TLS private key |
| server-b.pem | Server B TLS certificate (CN=server-b.local) |
| server-b-key.pem | Server B TLS private key |

## Regenerate

Run `./generate.sh` to regenerate all certificates. Requires `openssl` CLI.

## SANs

Both server certificates include Subject Alternative Names for Docker and local use:
- server-a.pem: DNS:server-a.local, DNS:server-a, DNS:localhost
- server-b.pem: DNS:server-b.local, DNS:server-b, DNS:localhost
