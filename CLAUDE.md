<!-- GSD:project-start source:PROJECT.md -->
## Project

**AI-Native Messaging Protocol & App**

An open, federated messaging protocol designed from the ground up for AI agent communication — where humans and agents are equal participants in conversations. The protocol defines how agents declare capabilities, exchange messages, stream responses, and render rich interactive UI. A native iOS app serves as the reference client, showcasing what messaging looks like when agents are first-class citizens, not bolted-on bots.

**Core Value:** The protocol must enable mixed human-agent threads where any participant (human or agent) can seamlessly join, communicate, and coordinate — with the same quality of experience regardless of who's a person and who's an AI.

### Constraints

- **Protocol-first**: Spec must be complete and implementable before building the iOS app
- **Open standard**: Protocol spec and reference implementations are fully open source
- **Federation**: No single point of control — any server can participate in the network
- **Transport**: Needs research — must work well for native iOS, real-time streaming, and server-to-server federation
- **Security**: Cryptographic identity and message signing are non-negotiable from day one
<!-- GSD:project-end -->

<!-- GSD:stack-start source:research/STACK.md -->
## Technology Stack

## 1. Protocol Specification Format & Tooling
### Recommended: Custom Spec in Markdown + JSON Schema + AsyncAPI
| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| Markdown (RFC-style) | N/A | Primary spec prose | Human-readable, diff-friendly, familiar to protocol authors. IETF uses markdown for drafts via mmark/kramdown-rfc |
| JSON Schema 2020-12 | Draft 2020-12 | Message schema definitions | Industry standard for validating JSON payloads. Strong tooling in both Rust (`jsonschema` crate) and Swift (`JSONSchema.swift`). MCP and A2A both use JSON Schema |
| AsyncAPI | v3.1.0 | Transport binding definitions | Purpose-built for event-driven/messaging APIs. Supports WebSocket, SSE, HTTP bindings natively. Code generation for docs, clients, servers. Complements (not replaces) the prose spec |
| Docusaurus | v3.x | Spec website/documentation | React-based, supports versioning, MDX for interactive examples, SEO-friendly. Used by most major open-source projects (React, Babel, etc.) |
### How the Spec Layers Work Together
### What NOT to Use
| Rejected | Why Not |
|----------|---------|
| OpenAPI 3.1 alone | Designed for request-response REST APIs. Our protocol is bidirectional, event-driven, streaming. AsyncAPI was built for exactly this |
| Protobuf as primary spec format | Binary format is not human-readable. Terrible for a spec people need to read and implement. Use JSON on the wire with JSON Schema for validation. Protobuf can be added later as an optional optimized encoding |
| IETF xml2rfc format | Overly complex toolchain for a project that is not (yet) pursuing IETF standardization. If/when we submit to IETF, we can transpile from markdown |
| Custom DSL | Inventing a schema language is a multi-year distraction. JSON Schema 2020-12 is the standard |
### Spec Validation Tooling
| Tool | Purpose |
|------|---------|
| `jsonschema` (Rust crate, v0.28+) | Validate message payloads against schemas in test suites |
| `@asyncapi/generator` | Generate docs, mock servers, and client stubs from AsyncAPI definition |
| `@asyncapi/cli` | Lint and validate AsyncAPI spec files in CI |
| JSON Schema Test Suite | Cross-implementation conformance testing |
## 2. Reference Server Implementation
### Recommended: Rust with Axum
| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| Rust | 1.85+ (2024 edition) | Server language | Memory safety without GC pauses (critical for real-time messaging). Predictable p99 latency. Connection costs kilobytes, not megabytes. Matrix's Conduit proves Rust is viable for federated messaging servers (32 MiB RAM fresh install vs Synapse's hundreds) |
| Axum | 0.8.x | HTTP/WebSocket framework | Built by the Tokio team. Modern async design. Native Tower middleware ecosystem. Built-in WebSocket and SSE support. Community consensus for new Rust web projects in 2026 |
| Tokio | 1.49+ | Async runtime | The async runtime for Rust. Axum is built on it. Battle-tested at scale |
| tokio-tungstenite | 0.28.x | WebSocket connections | Most-used Rust WebSocket library. Lightweight, async, integrates with Tokio |
| jsonrpsee | 0.26.x | JSON-RPC 2.0 implementation | Production-grade JSON-RPC over HTTP and WebSocket. Used by Polkadot/Substrate. MCP and A2A both use JSON-RPC; this keeps us aligned |
| PostgreSQL | 16+ | Primary datastore | Multi-tenant, concurrent writes, JSONB for flexible message metadata, full-text search, LISTEN/NOTIFY for real-time. Federated servers need concurrent access; SQLite locks on writes |
| sqlx | 0.8.x | Database driver | Compile-time SQL verification. Async. No ORM overhead |
| serde + serde_json | 1.x | Serialization | The Rust standard for JSON. Zero-copy deserialization |
| jsonschema (crate) | 0.28+ | Runtime schema validation | Validate incoming messages against protocol schemas. Supports JSON Schema 2020-12 |
| ring | 0.17.x | Cryptography | Ed25519 signing, SHA-512 hashing. Audited. Used by rustls |
| rustls | 0.23.x | TLS | Pure-Rust TLS. No OpenSSL dependency. Safer, simpler builds |
| tracing | 0.1.x | Observability | Structured logging with span context. Works with OpenTelemetry |
### Why Rust Over Alternatives
| Alternative | Why Not for This Project |
|-------------|--------------------------|
| **TypeScript/Node.js** | Cannot match Rust's latency profile for real-time messaging. GC pauses spike p99. Higher per-connection memory. Fine for prototyping, wrong for a reference server that demonstrates the protocol's performance characteristics |
| **Go** | Viable alternative, but Rust's ownership model catches concurrency bugs at compile time. For a federated protocol where message integrity matters, compile-time safety is worth the learning curve. Also: Matrix's Go homeserver (Dendrite) uses more resources than Conduit |
| **Elixir/Erlang** | Great concurrency model, but smaller ecosystem for cryptographic verification and JSON Schema tooling. Protocol alignment with JSON-RPC is weaker |
### Server Architecture
| WebSocket|    | REST/SSE  |     | Federation |
| Handler  |    | Handler   |     | Handler    |
## 3. iOS Client Stack
### Recommended: Pure SwiftUI + Swift Concurrency
| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| SwiftUI | iOS 18+ / Xcode 16+ | UI framework | Apple's declarative UI. LazyVStack for message lists, ScrollViewReader for auto-scroll, native animations. iOS 18 brings significant stability improvements. Messaging apps are the ideal SwiftUI use case |
| Swift Concurrency | Swift 6.1+ | Async/networking | `async/await`, `Actor` for thread safety, `AsyncStream` for WebSocket message handling. No third-party reactive framework needed |
| URLSessionWebSocketTask | iOS 13+ | WebSocket client | Native Apple API. Integrates with URLSession (auth, certs, background). No third-party dependency. Good enough for most cases |
| Network.framework (NWConnection) | iOS 14+ | Fallback/advanced networking | Lower-level control for custom close codes, network transition handling (WiFi to cellular), advanced TLS configuration. Use only where URLSession is insufficient |
| GRDB.swift | 7.10.x | Local database | SQLite toolkit with modern Swift API. Supports migrations, observation (database changes trigger UI updates), and direct SQL when needed. SwiftData is too immature (no background tasks, poor large dataset performance). GRDB is the consensus choice for production iOS database needs in 2026 |
| Apple CryptoKit | iOS 13+ | Cryptography | Native Ed25519 signing via `Curve25519.Signing`. Hardware-accelerated. No third-party crypto dependency. Exactly matches the protocol's signing requirements |
| swift-protobuf | 1.28+ | Optional protobuf encoding | Only if we add protobuf as an optimized wire format later. Not needed for v1 |
| KeychainAccess | 4.x | Secure key storage | Wrapper around iOS Keychain for storing Ed25519 private keys, auth tokens. Simpler API than raw Security framework |
### What NOT to Use
| Rejected | Why Not |
|----------|---------|
| **SwiftData** | Missing background task support. Performance degrades with large datasets. SwiftUI-view-coupled (hard to use in background sync). The messaging domain requires fast bulk inserts and queries on potentially thousands of messages -- SwiftData is not ready for this |
| **Core Data** | Legacy API. GRDB provides everything Core Data does with less boilerplate and better Swift integration |
| **Alamofire** | Adds dependency for HTTP networking that URLSession handles natively. We are primarily WebSocket-based, not REST-based |
| **Realm** | Proprietary (MongoDB). Sync features we do not need. GRDB on SQLite is lighter and we control the data format |
| **RxSwift / Combine** | Swift Concurrency (`async/await`, `AsyncStream`) has replaced the need for reactive frameworks in new SwiftUI projects. Combine is Apple's but poorly documented and not evolving |
| **gRPC-Swift 2** | Requires iOS 18 minimum. While we target iOS 18+, gRPC adds complexity for the client side where WebSocket + JSON-RPC suffices. gRPC is better suited for server-to-server federation (see Transport section) |
### iOS App Architecture
| Pattern | Choice | Rationale |
|---------|--------|-----------|
| Architecture | **MVVM + Repository** | MVVM is natural for SwiftUI's `@Observable` / `@StateObject`. Repository pattern isolates data access (network + local DB). TCA is overkill for a v1 client |
| State management | **@Observable (iOS 17+)** | Simpler than `ObservableObject`. Less boilerplate. Apple's recommended approach from WWDC 2024+ |
| Navigation | **NavigationStack + NavigationPath** | Type-safe, programmatic navigation. Works well for deep linking (open specific conversation/thread) |
| Dependency injection | **Environment + Protocol-based** | SwiftUI's `@Environment` for singletons (database, network). Protocols for testability |
## 4. Transport Layer
### Recommended: WebSocket (primary) + HTTP/SSE (fallback) + HTTP (federation)
| Context | Transport | Why |
|---------|-----------|-----|
| **Client-to-Server** (real-time) | WebSocket | Bidirectional, persistent, low-latency. Required for streaming agent responses, typing indicators, real-time message delivery. iOS handles WebSocket well via URLSessionWebSocketTask |
| **Client-to-Server** (fallback) | HTTP + SSE | For environments where WebSocket is blocked (corporate proxies). HTTP POST for sending, SSE for receiving. A2A protocol uses this pattern |
| **Server-to-Server** (federation) | HTTP POST + mutual TLS | Simpler than persistent connections between servers. Federation messages are less frequent and can tolerate slightly higher latency. Matrix uses HTTP for federation. Mutual TLS provides server identity verification |
### Why NOT gRPC Everywhere
| Issue | Impact |
|-------|--------|
| Browser support | gRPC requires gRPC-Web proxy for browser clients (future web client). WebSocket works natively everywhere |
| iOS complexity | gRPC-Swift 2 requires iOS 18. Adds heavy dependency. WebSocket is built into iOS |
| Debugging | Binary protobuf is hard to inspect. JSON over WebSocket is trivially debuggable with any proxy tool |
| Protocol simplicity | JSON-RPC 2.0 over WebSocket is the simplest possible transport that works. Lower implementation barrier for third-party server/client authors |
### Wire Protocol
| Layer | Format | Rationale |
|-------|--------|-----------|
| Envelope | JSON-RPC 2.0 | Standard, well-tooled, used by MCP. Method-based routing. Built-in request/response correlation via `id` |
| Message payloads | JSON | Human-readable, debuggable, universal. Agent UI blocks (A2UI-style) are naturally JSON |
| Schema validation | JSON Schema 2020-12 | Runtime validation of message payloads. Schemas published as part of spec |
| Signing | Ed25519 over canonical JSON | Deterministic serialization (sorted keys, no whitespace) before signing. Same approach as AMP protocol |
## 5. Security Stack
### Recommended Stack
| Technology | Purpose | Implementation |
|------------|---------|----------------|
| Ed25519 (Curve25519) | Message signing, identity keys | Server: `ring` crate. iOS: Apple CryptoKit `Curve25519.Signing` |
| TLS 1.3 | Transport encryption | Server: `rustls`. iOS: URLSession (automatic). Federation: mutual TLS with server certificates |
| JSON Canonicalization (JCS) | Deterministic serialization for signing | RFC 8785. Small library, implement from spec. Ensures same message produces same signature regardless of JSON key ordering |
| SHA-512 | Content hashing | Built into Ed25519 signing. Also for message content hashes (integrity verification) |
| JOSE / JWK | Key representation | JSON Web Key format for publishing public keys. Standard, well-tooled |
### Identity Model
### What NOT to Use
| Rejected | Why Not |
|----------|---------|
| RSA | Larger keys, slower signing. Ed25519 is faster and produces smaller signatures (64 bytes vs 256+ bytes) |
| ECDSA (P-256) | More complex implementation, historical implementation bugs (Sony PlayStation hack). Ed25519 is misuse-resistant by design |
| W3C DID | Interesting (ANP uses it) but adds blockchain/decentralized identity complexity. Email-like identity with Ed25519 keys at well-known URLs is simpler and sufficient for v1. DID can be added as an alternative identity method later |
| Signal Protocol (Double Ratchet) | Designed for end-to-end encrypted 1:1 messaging. Our protocol has server-visible messages (needed for agent processing, search, context management). E2EE is a v2 feature if needed |
## 6. Spec Publishing & Documentation
### Recommended Stack
| Tool | Version | Purpose | Why |
|------|---------|---------|-----|
| Docusaurus | 3.x | Spec website | Versioning support (critical as spec evolves), MDX for interactive protocol examples, React components for message format visualizers. Used by most major protocol/library docs |
| AsyncAPI Studio | Latest | Spec editing | Visual editor for AsyncAPI definitions. Live preview of transport bindings |
| Redocly CLI | Latest | API reference docs | Generates beautiful API reference from AsyncAPI/OpenAPI. Embed in Docusaurus |
| GitHub Pages / Vercel | N/A | Hosting | Free, CI/CD on push, custom domain support |
| JSON Schema Store | N/A | Schema distribution | Publish schemas to schemastore.org for IDE autocompletion in downstream implementations |
### What NOT to Use
| Rejected | Why Not |
|----------|---------|
| mdBook | Great for Rust docs, but lacks versioning, MDX support, and the React component ecosystem for interactive protocol examples |
| GitBook | Commercial. Free tier is limited. We want fully open toolchain |
| Stoplight | SaaS. Vendor lock-in for spec editing. AsyncAPI Studio is open source |
| Hugo / Jekyll | Older static site generators. No built-in versioning. Docusaurus is purpose-built for technical documentation |
## 7. Development & CI/CD Tooling
| Tool | Purpose | Why |
|------|---------|-----|
| cargo (Rust) | Build, test, lint server | Standard Rust toolchain |
| clippy | Rust linting | Catches common mistakes, enforces idioms |
| cargo-nextest | Test runner | Faster parallel test execution, better output |
| Xcode 16+ | iOS development | Required for SwiftUI iOS 18+ |
| Swift Package Manager | iOS dependency management | Native. No CocoaPods/Carthage complexity |
| GitHub Actions | CI/CD | Free for open source. Matrix strategy for testing against multiple Rust/Xcode versions |
| Docker | Server deployment | Containerized reference server for easy federation testing |
| docker-compose | Local federation testing | Spin up 2+ federated servers locally to test inter-server messaging |
## 8. Supporting Libraries (Server-Side Detail)
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| uuid | 1.x | Message IDs | Every message needs a unique identifier. UUIDv7 for time-ordered IDs |
| chrono | 0.4.x | Timestamps | RFC 3339 timestamps on all protocol messages |
| tower | 0.5.x | Middleware | Rate limiting, auth, logging. Axum is built on Tower |
| tower-http | 0.6.x | HTTP middleware | CORS, compression, request/response tracing |
| dashmap | 6.x | Concurrent HashMap | In-memory session state, connection tracking. Lock-free reads |
| deadpool-postgres | 0.14.x | Connection pool | Async PostgreSQL connection pooling |
## Alternatives Considered (Summary)
| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| Spec format | AsyncAPI + JSON Schema | OpenAPI 3.1 | Not designed for bidirectional/event-driven protocols |
| Server language | Rust | Go | Rust catches concurrency bugs at compile time; critical for protocol correctness |
| Server framework | Axum | Actix-web | Axum has better ecosystem alignment (Tower, Tokio team), nearly identical performance |
| Server DB | PostgreSQL | SQLite (Turso) | Federation requires concurrent writes from many connections; PostgreSQL handles this |
| iOS DB | GRDB.swift | SwiftData | SwiftData immature: no background tasks, poor large dataset perf |
| Transport | WebSocket + HTTP | gRPC | WebSocket is simpler, works everywhere (browsers, iOS, CLI), easier to debug |
| Crypto | Ed25519 | ECDSA P-256 | Ed25519 is misuse-resistant, faster, smaller signatures |
| Docs site | Docusaurus | mdBook | Docusaurus has versioning, MDX, React components |
| Wire format | JSON | Protobuf | Human-readable, debuggable, matches JSON-RPC and A2UI. Protobuf optional later |
## Installation (Server)
# Prerequisites
# Create project
# Core dependencies (Cargo.toml)
# axum = { version = "0.8", features = ["ws"] }
# tokio = { version = "1.49", features = ["full"] }
# tokio-tungstenite = "0.28"
# jsonrpsee = { version = "0.26", features = ["server", "ws-client"] }
# sqlx = { version = "0.8", features = ["runtime-tokio", "postgres", "json", "uuid", "chrono"] }
# serde = { version = "1", features = ["derive"] }
# serde_json = "1"
# jsonschema = "0.28"
# ring = "0.17"
# rustls = "0.23"
# tracing = "0.1"
# tracing-subscriber = "0.3"
# uuid = { version = "1", features = ["v7"] }
# chrono = { version = "0.4", features = ["serde"] }
# tower = "0.5"
# tower-http = { version = "0.6", features = ["cors", "trace", "compression"] }
# dashmap = "6"
## Installation (iOS Client)
## Installation (Spec Tooling)
# Spec documentation
# Schema validation
## Sources
- [AsyncAPI v3.1.0 Specification](https://www.asyncapi.com/docs/reference/specification/v3.1.0)
- [Axum on crates.io (v0.8.8)](https://crates.io/crates/axum)
- [tokio-tungstenite on crates.io (v0.28.0)](https://crates.io/crates/tokio-tungstenite)
- [jsonrpsee on crates.io (v0.26.0)](https://crates.io/crates/jsonrpsee)
- [GRDB.swift v7.10.0](https://github.com/groue/GRDB.swift)
- [Apple CryptoKit - Curve25519.Signing](https://developer.apple.com/documentation/cryptokit/curve25519/signing)
- [gRPC-Swift 2 announcement](https://www.swift.org/blog/grpc-swift-2/)
- [JSON Schema 2020-12](https://json-schema.org/draft/2020-12)
- [jsonschema Rust crate](https://crates.io/crates/jsonschema)
- [A2UI v0.8 (Google)](https://github.com/google/A2UI)
- [Conduit (Rust Matrix homeserver)](https://conduit.rs/)
- [Matrix federation security improvements 2025](https://media.ccc.de/v/matrix-conf-2025-74569-improving-the-security-of-the-federation-protocol)
- [Rust web framework comparison 2026](https://aarambhdevhub.medium.com/rust-web-frameworks-in-2026-axum-vs-actix-web-vs-rocket-vs-warp-vs-salvo-which-one-should-you-2db3792c79a2)
- [PointFree SQLiteData (GRDB alternative analysis)](https://www.pointfree.co/blog/posts/168-sharinggrdb-a-swiftdata-alternative)
<!-- GSD:stack-end -->

<!-- GSD:conventions-start source:CONVENTIONS.md -->
## Conventions

Conventions not yet established. Will populate as patterns emerge during development.
<!-- GSD:conventions-end -->

<!-- GSD:architecture-start source:ARCHITECTURE.md -->
## Architecture

Architecture not yet mapped. Follow existing patterns found in the codebase.
<!-- GSD:architecture-end -->

## Development Workflow

This project uses the **superpowers** skills to drive non-trivial work, not GSD. The skill system
surfaces when each applies; the core flow and entry points:

- **Brainstorming** (`superpowers:brainstorming`) — before any new feature or creative work. Turns
  an idea into an approved design doc in `docs/superpowers/specs/`. Required before implementation.
- **Writing plans** (`superpowers:writing-plans`) — turn an approved spec into an executable
  implementation plan.
- **Executing plans** (`superpowers:executing-plans`, `superpowers:subagent-driven-development`) —
  work a plan with review checkpoints.
- **TDD** (`superpowers:test-driven-development`) — default discipline for implementation code.
- **Systematic debugging** (`superpowers:systematic-debugging`) — for any bug, test failure, or
  unexpected behavior; find root cause before proposing a fix.
- **Verification before completion** (`superpowers:verification-before-completion`) — run the
  verification and confirm the output before claiming anything is done, fixed, or passing.

Design specs live in `docs/superpowers/specs/`. Commit in small, focused increments.

> **GSD is retired for this project.** The `.planning/` directory remains as the historical record
> of the v3.1–v3.3 milestones, but it is no longer the active workflow — do not route new work
> through `/gsd:*` commands.



## Developer Profile

> Profile not yet configured.
