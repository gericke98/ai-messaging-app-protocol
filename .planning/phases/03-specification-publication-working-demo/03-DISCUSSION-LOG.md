# Phase 3: Specification Publication & Working Demo - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md -- this log preserves the alternatives considered.

**Date:** 2026-03-28
**Phase:** 03-specification-publication-working-demo
**Areas discussed:** Demo scope & architecture, Schema organization, Spec versioning, Publication site

---

## Demo scope & architecture

### Demo type

| Option | Description | Selected |
|--------|-------------|----------|
| CLI terminal demo | Rust server + terminal REPL. Human types messages, agent responds with streaming + rich content | :heavy_check_mark: |
| Web-based demo | Rust server + browser UI (HTML/JS). Chat-like interface with rendered blocks | |
| Scripted scenario | Pre-scripted binary, zero interaction, pure protocol proof | |

**User's choice:** CLI terminal demo
**Notes:** Lowest complexity, proves protocol without UI distractions

### Agent brain

| Option | Description | Selected |
|--------|-------------|----------|
| Mock/scripted | Hardcoded responses showcasing different block types. No API key, works offline | :heavy_check_mark: |
| Real LLM via Claude API | Calls Claude API for responses. Requires API key, non-deterministic | |
| Both (mock default, LLM optional) | Mock by default, --llm flag for real API | |

**User's choice:** Mock/scripted
**Notes:** No external dependencies, reproducible, immediate clone-and-run

### Demo depth

| Option | Description | Selected |
|--------|-------------|----------|
| Core flow + highlights | Identity, threading, signing, streaming, 2-3 interactive blocks, Agent Card, 1 tool call | :heavy_check_mark: |
| Maximum coverage | Every spec feature including memory, bridges, sub-threading | |
| Minimal viable | Just identity + thread + message + streaming + 1 block | |

**User's choice:** Core flow + highlights
**Notes:** Exercises: identity, security, transport, threading, streaming, content (Text/Code/Form/Button/Approval), Agent Card, tool visibility. Defers: federation, memory tiers, MCP/A2A bridges, sub-threading

### Architecture

| Option | Description | Selected |
|--------|-------------|----------|
| Single binary | One cargo binary: server + client + agent in one process | :heavy_check_mark: |
| Separate processes | Server + client + agent as 3 separate binaries | |

**User's choice:** Single binary
**Notes:** Simpler for external devs to clone and run

---

## Schema organization

### Schema file structure

| Option | Description | Selected |
|--------|-------------|----------|
| Per-domain bundles | One schema file per spec domain, mirrors spec/ structure | :heavy_check_mark: |
| One schema per message type | Granular files (30+), maximum composability | |
| Single monolithic schema | One large ainm-protocol.schema.json | |

**User's choice:** Per-domain bundles
**Notes:** 12 schema files (11 domains + common.schema.json), mirrors spec/ directory

### AsyncAPI structure

| Option | Description | Selected |
|--------|-------------|----------|
| Single AsyncAPI doc | One asyncapi.yaml with all channels, refs to schema files | :heavy_check_mark: |
| Split per transport | Separate files for WebSocket and HTTP bindings | |

**User's choice:** Single AsyncAPI document
**Notes:** References per-domain JSON Schema files via $ref

### Schema derivation approach

| Option | Description | Selected |
|--------|-------------|----------|
| Extract from inline JSON examples | Read specs, find examples, derive schemas, add prose constraints | :heavy_check_mark: |
| Write schemas first, reconcile | Design schemas from requirements, then check against prose | |
| Auto-generate then refine | Tooling to auto-generate drafts from examples | |

**User's choice:** Extract from inline JSON examples
**Notes:** Formalization pass, not redesign. Spec prose + examples = ground truth

---

## Spec versioning

### Initial version

| Option | Description | Selected |
|--------|-------------|----------|
| 0.1.0 | Working draft, SemVer with 0.x freedom | :heavy_check_mark: |
| 1.0.0 | Signals confidence, full backwards compat from day one | |
| Draft-01 | IETF-style draft numbering | |

**User's choice:** 0.1.0
**Notes:** Precedent: MCP and A2A both started at 0.1.0

### Version scope

| Option | Description | Selected |
|--------|-------------|----------|
| Single protocol version | All specs + schemas share one version | :heavy_check_mark: |
| Per-spec file versioning | Each spec has its own version number | |

**User's choice:** Single protocol version
**Notes:** Block-level versioning (Phase 1 D-15) handles content evolution separately

### Backwards compatibility rules

| Option | Description | Selected |
|--------|-------------|----------|
| SemVer with 0.x freedom | Standard SemVer, 0.x allows breaking changes | :heavy_check_mark: |
| Strict from day one | Full SemVer discipline even in 0.x | |
| You decide | Claude's discretion | |

**User's choice:** SemVer with 0.x freedom
**Notes:** Breaking = field removal/rename, type change, message type removal, signing rule changes. Non-breaking = optional fields, new types, new methods

---

## Publication site

### Site structure

| Option | Description | Selected |
|--------|-------------|----------|
| Spec-centric with reference sidebar | Spec prose as primary content, schema reference + AsyncAPI explorer as sections | :heavy_check_mark: |
| Tutorial-first | Lead with tutorials, spec as reference | |
| Minimal GitHub README | Skip Docusaurus, spec files readable on GitHub | |

**User's choice:** Spec-centric with reference sidebar
**Notes:** Landing page + /spec/ (11 files) + /schemas/ + /asyncapi/ + /demo/ + /changelog

### Hosting

| Option | Description | Selected |
|--------|-------------|----------|
| GitHub Pages | Free, GitHub Actions CI/CD, custom domain | :heavy_check_mark: |
| Vercel | Free tier, preview deploys, vendor dependency | |
| You decide | Claude's discretion | |

**User's choice:** GitHub Pages
**Notes:** Standard for open-source protocol docs, zero cost

### AsyncAPI integration

| Option | Description | Selected |
|--------|-------------|----------|
| Embedded React component | @asyncapi/react-component in MDX page | :heavy_check_mark: |
| Link to external AsyncAPI Studio | Off-site interactive viewing | |
| Static render only | Generated HTML/Markdown, no interactivity | |

**User's choice:** Embedded React component
**Notes:** Addresses STATE.md concern about AsyncAPI + Docusaurus integration

---

## Claude's Discretion

- Terminal output formatting for rich content blocks
- Demo agent's specific scripted conversation scenarios
- Schema $ref linking strategy and common.schema.json decomposition
- Docusaurus theme/styling choices
- GitHub Actions workflow configuration
- CHANGELOG.md format
- Landing page copy and quick-start guide content

## Deferred Ideas

None -- discussion stayed within phase scope.
