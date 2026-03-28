---
sidebar_position: 13
title: Running the Demo
---

# Running the Demo

A single-binary Rust CLI demo that proves the AINM protocol end-to-end: one human and one agent in a thread with streaming, rich interactive content, and cryptographic message signing.

## Prerequisites

- **Rust 1.85+** -- install via [rustup.rs](https://rustup.rs/)

## Quick Start

```bash
# Clone the repository
git clone https://github.com/gericke98/ai-native-messaging.git
cd ai-native-messaging/demo

# Build and run
cargo run --bin ainm-demo
```

The demo starts a local Axum WebSocket server, registers identities for a human and a mock agent, connects the agent as an in-process WebSocket client, and opens a terminal REPL for human input. No external dependencies, no API keys, works completely offline.

## What to Try

Type these commands to see different protocol features in action:

| Command | What It Demonstrates |
|---------|---------------------|
| `hello` | Streaming response with visible token-by-token rendering |
| `form` | FormBlock rendered in terminal with labeled fields |
| `approve` | ApprovalBlock with Approve/Reject options |
| `tool` | Tool call lifecycle: Started, Progress, Completed |
| `button` | Three styled ButtonBlocks (primary, secondary, danger) |
| `/card` | Agent Card display for `helper@demo.ainm.local` |
| `/quit` | Exit the demo |

## What You Will See

- **Streaming:** Text appears token by token, demonstrating the stream start/delta/end lifecycle
- **Rich Content:** Forms, buttons, and approvals render as structured terminal output
- **Signature Verification:** Every message shows a `[VERIFIED]` badge confirming Ed25519 signature validity
- **Tool Transparency:** Tool call events show the full lifecycle visible to all participants
- **Agent Card:** The `/card` command displays the mock agent's capability manifest

## Architecture

The demo is a single Rust binary with these components:

- **Axum WebSocket server** on `127.0.0.1:9090`
- **Mock agent** connected via tokio-tungstenite as an in-process WebSocket client
- **Terminal REPL** using rustyline for human input
- **Ed25519 signing** via the `ring` crate with RFC 8785 canonical JSON
- **Broadcast channels** for bidirectional message routing between human and agent

All messages follow the AINM protocol wire format: JSON-RPC 2.0 envelopes with Ed25519-signed content.
