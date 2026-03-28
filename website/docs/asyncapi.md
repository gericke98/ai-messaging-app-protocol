---
title: Transport Bindings (AsyncAPI)
sidebar_label: Transport Bindings
slug: /asyncapi
---

# AsyncAPI Transport Bindings

The AINM protocol defines its transport layer using [AsyncAPI v3.1.0](https://www.asyncapi.com/), the industry standard for event-driven API specifications.

The AsyncAPI document describes all WebSocket channels, operations, and message schemas for client-server communication.

## Channels

| Channel | Description | Operations |
|---------|-------------|------------|
| `session` | Connection lifecycle | initialize, initialized, shutdown |
| `messaging` | Message exchange | message.send, message.deliver |
| `threading` | Thread management | thread.create, thread.join, thread.leave, sub-thread.create |
| `presence` | Participant status | presence.update, presence.notify |
| `streaming` | Real-time streaming | stream.start, stream.delta, stream.end, stream.cancel, stream.error |
| `memory` | Context management | memory.store, memory.query, memory.evict, context.push |
| `toolEvents` | Tool call visibility | tool.call.started, tool.call.progress, tool.call.completed |
| `federation` | Server-to-server | federation.relay |

## Download

- **[asyncapi.yaml](/asyncapi.yaml)** -- Full AsyncAPI 3.1.0 specification
- **[JSON Schemas](https://github.com/gericke98/ai-native-messaging/tree/main/schemas)** -- All 12 protocol schema files on GitHub

## Viewing the Spec

You can explore the AsyncAPI document interactively using [AsyncAPI Studio](https://studio.asyncapi.com/):

1. Download the `asyncapi.yaml` file above
2. Open [AsyncAPI Studio](https://studio.asyncapi.com/)
3. Import the file to browse channels, operations, and message schemas

## Schema References

The AsyncAPI document references 12 JSON Schema 2020-12 files that define all protocol message types:

| Schema | Definitions | Domain |
|--------|-------------|--------|
| `common.schema.json` | 11 | Shared types (UUIDv7, Address, Timestamp, Signature) |
| `identity.schema.json` | 7 | Identity registration and resolution |
| `security.schema.json` | 7 | Signing, verification, anti-injection |
| `content.schema.json` | 6 | Content blocks (text, code, image, file) |
| `transport.schema.json` | 21 | Session, messaging, threading, presence |
| `interactive.schema.json` | 9 | Forms, buttons, tables, approvals |
| `streaming.schema.json` | 6 | Stream lifecycle (start, delta, end) |
| `threading.schema.json` | 7 | Threads, sub-threads, mentions |
| `agent-card.schema.json` | 9 | Agent capability discovery |
| `memory.schema.json` | 8 | Three-tier memory management |
| `tool-visibility.schema.json` | 3 | Tool call event lifecycle |
| `compatibility.schema.json` | 5 | MCP/A2A bridges |
