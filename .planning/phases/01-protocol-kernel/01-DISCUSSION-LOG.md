# Phase 1: Protocol Kernel - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md -- this log preserves the alternatives considered.

**Date:** 2026-03-28
**Phase:** 01-protocol-kernel
**Areas discussed:** Spec document organization, Identity resolution mechanism, Message envelope design, Content block extensibility

---

## Spec Document Organization

### Q1: How should the spec documents be structured?

| Option | Description | Selected |
|--------|-------------|----------|
| Modular per-domain | Separate files: identity.md, security.md, transport.md, content.md, threading.md. Each stands alone. | ✓ |
| Single monolithic document | One comprehensive spec file covering everything. | |
| Layered (core + extensions) | Small core spec plus extension specs. | |

**User's choice:** Modular per-domain
**Notes:** None

### Q2: What writing style for the spec prose?

| Option | Description | Selected |
|--------|-------------|----------|
| Developer-friendly first | Clear, accessible prose with examples. RFC keywords for normative statements only. | ✓ |
| RFC-formal throughout | Traditional IETF style with RFC 2119 keywords everywhere. | |
| Hybrid: narrative + formal | Narrative explanation followed by formal normative requirements. | |

**User's choice:** Developer-friendly first
**Notes:** None

### Q3: Should each spec module include inline examples?

| Option | Description | Selected |
|--------|-------------|----------|
| Inline examples throughout | Every message type gets a concrete JSON example next to its definition. | ✓ |
| Separate examples appendix | Examples collected at end or in a separate file. | |
| You decide | Claude picks the approach. | |

**User's choice:** Inline examples throughout
**Notes:** None

### Q4: Where should spec files live in the repo?

| Option | Description | Selected |
|--------|-------------|----------|
| spec/ directory at root | spec/identity.md, spec/security.md, etc. Clean separation. | ✓ |
| docs/spec/ nested under docs | Anticipates Docusaurus site structure. | |
| protocol/ directory | More descriptive name. | |

**User's choice:** spec/ directory at root
**Notes:** None

---

## Identity Resolution Mechanism

### Q1: How should identity resolution work?

| Option | Description | Selected |
|--------|-------------|----------|
| .well-known URL | GET https://provider/.well-known/ainm/name returns identity data. Simple HTTP. | ✓ |
| WebFinger (RFC 7033) | Standard protocol for resolving acct: URIs. More formal. | |
| DNS SRV + .well-known hybrid | DNS SRV for server discovery, .well-known for keys. Most robust. | |

**User's choice:** .well-known URL
**Notes:** None

### Q2: What should the .well-known endpoint return?

| Option | Description | Selected |
|--------|-------------|----------|
| Flat JSON with key + endpoint | { id, public_key, server, type, agent_card_url }. One request, everything needed. | ✓ |
| JWK Set (JSON Web Key) | Standard JWK format with rotation built-in. | |
| You decide | Claude picks the format. | |

**User's choice:** Flat JSON with key + endpoint
**Notes:** None

### Q3: How should key rotation work?

| Option | Description | Selected |
|--------|-------------|----------|
| Key history array | Current key + array of previous keys with validity periods. | ✓ |
| Versioned keys | Each key gets a version number, messages reference key version. | |
| You decide | Claude designs the mechanism. | |

**User's choice:** Key history array
**Notes:** None

### Q4: Should protocol expose participant type (human vs agent)?

| Option | Description | Selected |
|--------|-------------|----------|
| Type in identity metadata | .well-known response includes 'type' field. Transparency without discrimination. | ✓ |
| No type exposure | Fully opaque -- can't tell human from agent. | |
| Optional self-declaration | Type field exists but optional. | |

**User's choice:** Type in identity metadata
**Notes:** None

---

## Message Envelope Design

### Q1: How heavy should the message envelope be?

| Option | Description | Selected |
|--------|-------------|----------|
| Lean envelope | Routing + metadata + signature + content blocks. Trust annotations inside blocks. | ✓ |
| Rich envelope | Routing + signature + top-level metadata + annotations + trust context. | |
| Minimal + extensions | Tiny mandatory envelope with optional 'extensions' object. | |

**User's choice:** Lean envelope
**Notes:** None

### Q2: How should trust annotations work?

| Option | Description | Selected |
|--------|-------------|----------|
| Source field per block | Each block carries 'source': user/agent/system. Verified by signature. | ✓ |
| Separate trust metadata layer | Top-level 'trust' object maps blocks to trust levels. | |
| You decide | Claude designs trust annotations. | |

**User's choice:** Source field per block
**Notes:** None

### Q3: How should message addressing work?

| Option | Description | Selected |
|--------|-------------|----------|
| Thread-addressed | Messages sent TO a thread_id. All members receive. DMs = 2-person thread. | ✓ |
| Participant-addressed | Messages sent TO specific participants. More like email. | |
| Hybrid | Thread-addressed default with optional 'to' for directed messages. | |

**User's choice:** Thread-addressed
**Notes:** None

### Q4: What delivery guarantee?

| Option | Description | Selected |
|--------|-------------|----------|
| At-least-once with dedup | Server guarantees delivery at least once. Unique IDs for dedup. | ✓ |
| Exactly-once | Two-phase delivery. Harder to implement correctly. | |
| Best-effort | Fire-and-forget for streaming, store-and-forward for important. | |

**User's choice:** At-least-once with dedup
**Notes:** None

---

## Content Block Extensibility

### Q1: How extensible should the block system be?

| Option | Description | Selected |
|--------|-------------|----------|
| Closed core + namespaced extensions | Fixed core types + reverse-domain namespacing for custom types. | ✓ |
| Fully open registry | Any block type string valid. Central registry tracks known types. | |
| Strictly closed | Only protocol-defined types valid. New types require spec revision. | |

**User's choice:** Closed core + namespaced extensions
**Notes:** None

### Q2: How should clients handle unknown block types?

| Option | Description | Selected |
|--------|-------------|----------|
| Mandatory fallback field | Every block MUST include 'fallback' with plain text/markdown. | ✓ |
| Type-level fallback hierarchy | Each type declares 'fallback_type', client walks the chain. | |
| You decide | Claude designs the fallback mechanism. | |

**User's choice:** Mandatory fallback field
**Notes:** None

### Q3: Should blocks support independent schema versioning?

| Option | Description | Selected |
|--------|-------------|----------|
| Block-level version field | Each block carries 'version'. Blocks evolve independently. | ✓ |
| Protocol-version only | Block schemas pinned to protocol version. | |
| You decide | Claude determines versioning granularity. | |

**User's choice:** Block-level version field
**Notes:** None

---

## Claude's Discretion

No areas deferred to Claude's discretion.

## Deferred Ideas

None -- discussion stayed within phase scope.
