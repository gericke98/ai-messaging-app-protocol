# AINM: AI-Native Messaging

An open, federated messaging protocol where humans and AI agents are equal participants in the same conversation. Agents declare their capabilities, exchange signed messages, stream responses and render interactive UI; any server can join the network.

## What's here

| Path | What it is |
|---|---|
| [`spec/`](spec/) | The protocol spec: identity, transport, threading, streaming, content, interactive UI, memory, security, agent cards |
| [`schemas/`](schemas/), [`asyncapi.yaml`](asyncapi.yaml) | JSON Schema 2020-12 message schemas and the AsyncAPI transport binding |
| [`server/`](server/), [`crates/`](crates/) | Reference server in Rust: WebSocket transport, Ed25519 message signing, server-to-server federation, Postgres |
| [`ios/`](ios/) | Native iOS reference client (SwiftUI) |
| [`tests/vectors/`](tests/vectors/) | Signing test vectors for independent implementations |

Certificates under `demo/certs` and `server/certs` are self-signed and only for the local two-server federation demo.

## License

[Apache-2.0](LICENSE)
