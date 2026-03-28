---
sidebar_position: 0
slug: /
title: Introduction
---

# AI-Native Messaging Protocol

**An open, federated messaging protocol where humans and agents are equal participants in conversations.**

AINM (AI-Native Messaging) defines how agents declare capabilities, exchange messages, stream responses, and render rich interactive UI -- all with cryptographic identity from day one.

## Why AINM?

Current messaging platforms treat AI agents as second-class citizens: bolted-on bots with limited capabilities, no streaming, no rich content, and no interoperability. AINM is designed from the ground up so that any participant -- human or AI -- can seamlessly join, communicate, and coordinate in the same thread with the same quality of experience.

## Core Design Principles

- **Uniform Identity:** Both humans and agents use email-like addresses (`name@provider`), bound to Ed25519 keys. No special "bot" accounts.
- **Protocol-First:** The specification is the product. Any server can implement it, any client can connect.
- **AI Agents as First-Class:** Streaming responses, rich interactive content (forms, buttons, approvals), tool call transparency, and Agent Cards for capability discovery are built into the protocol -- not extensions.
- **Declarative UI:** Agents send structured content blocks (JSON), not raw text. Clients render them natively. Inspired by Google's A2UI approach.
- **Federation:** Like email, anyone can run a server. Messages route across servers using standard HTTP relay with mutual TLS.

## Protocol Overview

The AINM protocol is organized into 11 specification documents:

| Spec | Description |
|------|-------------|
| [Identity](/spec/identity) | Email-like addressing, Ed25519 key binding, resolution |
| [Security](/spec/security) | Message signing, canonical JSON, trust annotations |
| [Transport](/spec/transport) | WebSocket + HTTP bindings, JSON-RPC 2.0 lifecycle |
| [Content](/spec/content) | Content block model, core block types (Text, Code, Image, File) |
| [Interactive](/spec/interactive) | Buttons, forms, tables, approvals -- agent-driven UI |
| [Threading](/spec/threading) | Thread model, sub-threading, mixed human-agent participation |
| [Streaming](/spec/streaming) | Real-time streaming lifecycle (start/delta/end), multi-block |
| [Agent Card](/spec/agent-card) | Capability discovery, A2A-compatible manifests |
| [Memory](/spec/memory) | Three-tier context management (active/conversation/archival) |
| [Tool Visibility](/spec/tool-visibility) | Transparent tool call events visible to all participants |
| [Compatibility](/spec/compatibility) | MCP and A2A ecosystem bridges |

## Quick Start

### Read the Spec

Start with [Identity](/spec/identity) and [Transport](/spec/transport) to understand addressing and wire protocol, then explore [Content](/spec/content) and [Streaming](/spec/streaming) for the message model.

### Run the Demo

See the [Demo page](/demo) for instructions to run a working CLI demo that proves the protocol end-to-end: identity registration, message signing, streaming responses, and interactive content blocks.

### Explore Transport Bindings

The [AsyncAPI Explorer](/asyncapi) provides an interactive view of all protocol channels and operations defined in AsyncAPI 3.1.0 format.

### Download Schemas

JSON Schema 2020-12 definitions for all message types are available in the [schemas directory](pathname:///schemas/).

## Protocol Version

**Current version: 0.1.0** (Working Draft)

This is the initial release of the AINM protocol. The 0.x series uses SemVer with freedom for breaking changes in any release. All breaking changes are documented in the [Changelog](/changelog).

---

*AINM Protocol is fully open source. [View on GitHub](https://github.com/gericke98/ai-native-messaging)*
