# Feature Research — v3.3 Universal Agent Client

**Domain:** Universal multi-agent messaging client (extending AINM iOS reference client)
**Researched:** 2026-04-18
**Confidence:** MEDIUM-HIGH (mix of verified industry patterns + informed extrapolation to AINM)

---

## Scope Guardrails

This research is **additive** to the v3.1 protocol (57 requirements) and v3.2 iOS client (34 requirements).
The following are **NOT in scope for new research** — they already ship:

- Tool call lifecycle events (TOOL-01..05) — AINM protocol defines Started / Progress / Completed, iOS renders as collapsible indicators
- @-mentions (MIXD-04, AI-02) — spec supports cross-type, iOS supports autocomplete + highlighting
- Agent Cards (DISC-01..05, AI-03) — A2A-compatible discovery, iOS browses them
- Three-tier memory (MEMO-01..06, AI-04, AI-05) — active / conversation / archival, iOS reads + deletes facts
- Presence states (MIXD-03) — online/offline for humans, available/busy/processing for agents
- Interactive blocks (CONT-03, INT-01..05) — Button/Form/Table/Approval render natively
- Streaming lifecycle (STRM-01..04, STR-01..04) — token-by-token with cancel
- MCP + A2A compatibility bridges at protocol layer (COMP-01..04) — MCP server CAN be an agent participant per spec; what's missing is connector + iOS UX

**What IS new for v3.3:** Making many agents live in one thread comfortably, getting them in through connectors, giving MCP servers a client-side presence, and laying compound-memory plumbing for v3.4.

---

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist when joining a 2026 multi-agent client. Missing any of these = product feels incomplete vs Claude Desktop, ChatGPT with MCP apps, Slack-with-bots, Cline, Continue.dev.

#### Multi-Agent Thread UX

| # | Feature | Why Expected | Complexity | Dep. on Existing |
|---|---------|--------------|------------|------------------|
| MA-01 | **Distinct visual identity per agent** in thread (avatar + display name + provider domain) | Slack/Discord precedent: each bot has its own avatar/name. Already true for humans — must match for agents when >1 agent in thread | LOW | MSG-02, MSG-09 (iOS already shows agent badge; extend to N agents) |
| MA-02 | **Per-agent typing/processing indicators** (multiple concurrent indicators) | With 3 agents in a thread, seeing "Claude is thinking… GPT is thinking…" is expected; collapsing all under a single "typing" is wrong | LOW | STR-02, MIXD-03 (protocol already supports per-participant presence; iOS needs layout for N indicators) |
| MA-03 | **Message attribution always visible** (sender avatar + name on every agent message, never collapsed like iMessage group of same sender) | In multi-agent threads, visually collapsing consecutive messages by sender breaks attribution. Users need to see *which agent* said each line | LOW | MSG-02 (change the consecutive-sender collapse rule when sender type=agent) |
| MA-04 | **Tool-call attribution to calling agent** (tool call bubble shows which agent invoked it) | When Claude Code runs `Bash` and GPT runs `web_search`, user must see which is which. Current AI-01 shows tool calls but attribution becomes load-bearing at N>1 | LOW | AI-01, TOOL-01..05 (add `issuer_agent_id` display to existing tool indicators) |
| MA-05 | **@-mention routing** delivers only to the mentioned agent(s), not all agents | Server-side gating. Without this, every agent replies to every message and threads become a runaway cost loop. Slack/Discord do this; must be first-class | MEDIUM | MIXD-04, AI-02 (iOS composer supports mentions; server AgentRegistry must gate delivery by mention-set) |
| MA-06 | **Silence / pause an agent** in a thread (still visible, stops receiving messages) | Slack "mute this app"; Discord "disable bot". Essential escape hatch when an agent is spammy, off-topic, or expensive | MEDIUM | New server capability + iOS participant-row action |
| MA-07 | **Remove agent from thread** (participant removal) | Basic thread management. Protocol probably has participant list ops already; needs UX surface | LOW | Existing participant model; iOS long-press on agent row |
| MA-08 | **Agent-to-agent message visibility** (user sees when Agent A replies to Agent B, with visual link) | When agents coordinate in a thread, the user is the supervisor — they must see the conversation, not have it hidden. Unlike traditional chat-between-bots (which hides it), AINM's value is making it visible | LOW | MSG-02, MIXD-04 (same message-render path; subtle reply-indicator when `in_reply_to` is another agent) |

#### Connector Discovery & Onboarding

| # | Feature | Why Expected | Complexity | Dep. on Existing |
|---|---------|--------------|------------|------------------|
| CN-01 | **Add-agent flow from Agent Discovery screen** (not just browse — also "+ Add to my account") | v3.2 AI-03 shipped discovery (browse Agent Cards); v3.3 must close the loop so discovery leads to an added agent | MEDIUM | AI-03 (extend existing discovery screen), NET-04 (reuse identity) |
| CN-02 | **URL-based connector invite** (`ainm://add-agent?card=https://…`) | Deep link already exists (PLT-05); extending to agent-install is industry-standard (Slack app install URLs, VSCode `vscode:extension/...`) | LOW | PLT-05 (extend DeepLinkHandler with new verb) |
| CN-03 | **QR-code pairing for local/device connectors** (laptop Claude Code → phone AINM) | OAuth device flow precedent; Slack/Discord don't need it, but AINM is iOS-first and many connectors run on the user's laptop. Standard cross-device pattern per IETF draft-ietf-oauth-cross-device-security-16 | MEDIUM | Requires server pairing endpoint + iOS camera view |
| CN-04 | **Paired-device status in settings** (list of my connectors: Claude-Code-on-MacBook, GPT-via-OpenAI, ...) | Users need to audit "what's connected to me". Precedent: iOS Settings > Password & Security > Devices; Slack Settings > Apps | LOW | Existing AgentRegistry data; iOS settings screen |
| CN-05 | **Revoke connector** (pull its credentials/session) | Paired with CN-04. When a user loses their laptop or stops using Claude Code, they must be able to kick the connector | LOW | Server credential revocation + iOS button |

#### MCP Bridge UX

| # | Feature | Why Expected | Complexity | Dep. on Existing |
|---|---------|--------------|------------|------------------|
| MCP-01 | **MCP server appears as one agent-like participant** (`github@mcp.local`, `postgres@mcp.local`) — NOT as a flat list of tools | Mental-model research finding: hosts orchestrate conversation, servers own side effects. The cleanest UX for a messaging app is "MCP server = agent with only tools, no model". User @-mentions it. This mirrors COMP-02 which already says "An MCP server can be exposed as an agent participant in a thread" | MEDIUM | COMP-02, AI-03 (MCP bridge connector publishes an Agent Card; MCP tools surface as `tools` in card) |
| MCP-02 | **MCP tool call attributed to MCP-agent** (not "AINM" or "system") | Same principle as MA-04. When GitHub MCP returns `gh.list_issues`, the tool bubble reads "github@mcp.local ran list_issues" | LOW | MA-04, AI-01 |
| MCP-03 | **MCP Apps inline UI** (iframe / declarative UI returned from MCP tool calls, rendered in-thread) | MCP Apps shipped Jan 2026 (Claude/ChatGPT/VS Code support it). For AINM to be "universal" it must render these. AINM already has declarative InteractiveBlock (CONT-04) — map MCP `_meta.ui.resourceUri` to an AINM block type | HIGH | CONT-04 (reuse A2UI-style component catalog), INT-01..05 (add new block type or extend existing) |
| MCP-04 | **Per-tool consent gates** on first use of each tool | MCP Apps security model includes "hosts can require explicit approval for UI-initiated tool calls". AINM already has ApprovalBlock (INT-04); user should be able to set policy "ask first time github.create_issue runs" | MEDIUM | INT-04, AI-01 (reuse approval UX; add policy store) |

#### Tool Call Transparency (across connector boundaries)

| # | Feature | Why Expected | Complexity | Dep. on Existing |
|---|---------|--------------|------------|------------------|
| TC-01 | **Every tool call visible by default** (not hidden) | Cline precedent: "shows file tree it touches, commands it runs, diffs it proposes". AINM already ships this at protocol level (TOOL-04) — v3.3 must NOT regress when tools come from a connector-bridged agent | LOW | TOOL-01..05, AI-01 (ensure connector tool events translate to AINM ToolCall events on the wire) |
| TC-02 | **Collapsed-by-default with expansion** (Aider/Cline pattern: short one-liner, tap for full args + output) | When Claude Code does 30 Read/Bash calls in a loop, flat expansion wrecks the thread. v3.2 AI-01 already renders collapsible — verify it scales to connector-emitted volume | LOW | AI-01 (already collapsible; verify at N=30+) |
| TC-03 | **Grouped tool calls by turn** ("Claude did 14 tool calls → expand") | Cline's approach when many tools run in a single agent turn. Avoids wall of bubbles | MEDIUM | AI-01 + new grouping logic in iOS message list |
| TC-04 | **Tool call truncation for large outputs** (show first N lines of `Read` / `Bash` output with "show more") | A `Read` of a 3000-line file should not render 3000 lines inline. Cline/Continue cap at ~500 chars preview | LOW | MSG-05 (already renders CodeBlock; add truncation wrapper) |
| TC-05 | **Approve-before-run policy per tool** (user can mark `Bash` as "always ask", `Read` as "auto-allow") | Cline's core UX — "pauses for human approval at each step by default". For AINM, users of Claude Code connector will expect the same. Per-tool, per-agent, per-thread scoping | HIGH | INT-04 (ApprovalBlock), new policy store |

#### Compound Memory (v3.3 groundwork for v3.4)

v3.2 already ships per-agent memory tiers (MEMO-01..06, AI-04, AI-05). v3.3 lays plumbing for v3.4 to cross-pollinate memory across agents. Features below are the **minimum groundwork** — the full compound-memory UX lands in v3.4.

| # | Feature | Why Expected | Complexity | Dep. on Existing |
|---|---------|--------------|------------|------------------|
| MEM-01 | **Source attribution on memory facts** (each archival fact tagged with `source_agent_id`, `source_thread_id`, `extracted_at`) | Provenance research (arxiv PROV-AGENT 2026, Cisco helpnetsecurity 2026) explicit: "actor-aware memory tags each stored memory with its source actor… avoiding situations where one agent's inference gets treated as ground truth by another agent downstream" | MEDIUM | MEMO-04, AI-05 (extend archival fact schema with provenance fields; iOS shows "learned by Claude Code on 2026-04-18 from #thread-42") |
| MEM-02 | **Connector pushes memory to server** (connectors can call `ainm.memory.write` to persist conversation-extracted facts to server-side memory) | Memory compounds = groundwork for v3.4. Individual agent runtimes (Claude, GPT) already extract facts — protocol needs the write channel | MEDIUM | MEMO-04 (server-side archival store exists); new SDK method |
| MEM-03 | **User inspection of what-will-be-written** (preview of facts a connector is about to persist, with consent) | Anti-pattern otherwise: invisible writes. ChatGPT memory shows "Memory updated" badge inline; AINM should show the fact + source before persist | MEDIUM | MEM-01 + new UI flow |
| MEM-04 | **Per-agent memory read scope** (GPT agent can read Claude's extracted facts IF user allowed) | Table stakes for "compound memory" claim. Without this, memory is per-agent silos. User-controlled sharing defaults | HIGH | MEMO-04 + ACL layer |
| MEM-05 | **Memory export** (user can download all archival facts as JSON) | "Compound memory that belongs to the user" promise requires portability. Mem0/Letta/ChatGPT/Claude all have export | LOW | AI-05 (already reads all facts); add export endpoint |

#### Connector Authentication

| # | Feature | Why Expected | Complexity | Dep. on Existing |
|---|---------|--------------|------------|------------------|
| AU-01 | **Signed-handshake connector identity** (connector proves it owns an Ed25519 key at handshake) | Security non-negotiable for AINM (SECU-01..03 ship already). Connector SDK must participate in signing | LOW | SECU-01, NET-04..06 (reuse same Ed25519 pattern server-side) |
| AU-02 | **Paste-token onboarding** (user copies token from iOS app, pastes into `claude-code-ainm connect`) | Lowest-friction auth for technical users adding their own connector. Precedent: `gh auth login`, VSCode extensions | LOW | AU-01 + server token-mint endpoint |
| AU-03 | **OAuth-style browser flow for hosted connectors** (Claude Pro/Max subscription, OpenAI-hosted) | Hosted runtimes will have their own auth. Precedent: Slack app install, OpenClaw OAuth for Claude Pro | HIGH | New OAuth client + redirect handling |
| AU-04 | **API-key storage for self-managed connectors** (user provides their own OpenAI/Anthropic key to the connector) | Avoids vendor lock-in. Connector stores key locally, never sends to AINM server. Precedent: Cline, Continue.dev | LOW | Connector-SDK level; not iOS |

---

### Differentiators (Competitive Advantage)

Features that set AINM apart — aligned with PROJECT.md Core Value "equal quality of experience regardless of who's a person and who's an AI" + v3.4 compound-memory strategic frame.

| # | Feature | Value Proposition | Complexity | Dep. on Existing |
|---|---------|-------------------|------------|------------------|
| DIFF-01 | **Ed25519-signed tool calls** (every tool call from a connector is cryptographically signed by the agent) | No other client does this. Slack/Discord bots are unsigned. Means a malicious connector can't spoof another agent's tool calls | MEDIUM | SECU-01 extended to tool-call events |
| DIFF-02 | **Protocol-level memory transparency** (user sees which facts went into each agent's context window before they reply) | v3.2 AI-04 already shows "memory context sheet" — this generalizes to "for this reply, Claude loaded facts X,Y,Z from archival, and these 3 messages as active context". Goes beyond ChatGPT/Claude's "memory panel" because it's per-turn, not per-account | MEDIUM | AI-04 (expand context sheet to per-turn) |
| DIFF-03 | **Replaceable agents with inherited memory** (swap Claude-3.5 connector for Claude-4 — new agent inherits all extracted facts via server-side memory) | From PROJECT.md: "Agent replacement is seamless… no context is lost when participants change". Requires MEM-01..04 | HIGH | MEM-01, MEM-02, MEM-04 |
| DIFF-04 | **Parallel agent conference** (ask 2 agents same question, see answers side-by-side) | Structured "side-by-side" rendering vs Slack's interleaved mess. AINM's declarative-UI layer lets us design this well | MEDIUM | MA-02 (parallel indicators), new thread-view layout mode |
| DIFF-05 | **Agent handoff as first-class protocol message** (a HandoffBlock that says "passing to @researcher-gpt" with reason) | CrewAI/LangGraph have handoff as code; AINM can expose it as a visible message so the user is never surprised. Complements MA-08 | MEDIUM | New content block type (extends CONT-02/03) |
| DIFF-06 | **Connector SDK in 2 languages** (TypeScript + Python) | Most agent runtimes are Python. TS alone (v3.2 state) gates 50%+ of the ecosystem. A6 MCP bridge is likely easier in TS; A5 Python connector SDK opens the Python ecosystem | HIGH | Existing TypeScript SDK at `packages/connector` as reference |
| DIFF-07 | **MCP-to-AINM compilation (any MCP server → AINM agent)** | Realizes COMP-02 at the implementation level. Turns every existing MCP server (97M+ downloads/mo per PROJECT.md) into an AINM participant without modification | HIGH | Connector SDK + MCP client library |

---

### Anti-Features (Commonly Requested, Problematic)

Features that look reasonable but would harm the product. Documented explicitly to resist scope creep.

| # | Anti-Feature | Why Requested | Why Problematic | What to Do Instead |
|---|--------------|---------------|-----------------|--------------------|
| ANTI-01 | **"Let agents talk to each other freely in a thread"** (no @-mention gating) | Seems like "true multi-agent". Hands-off orchestration | $47K failure case documented (techstartups.com, Nov 2025): two agents looped for 11 days. Core anti-pattern per Smashing Magazine, Medium/Modexa 2026. Without gating, every agent replies to every message, costs explode, threads become unreadable | **MA-05 mandatory**: agents only respond when @-mentioned (or when explicitly handed-off to via DIFF-05). Default: silence unless addressed |
| ANTI-02 | **"Auto-extract memory without asking"** (ChatGPT default) | Seamless for user. No friction | Memory poisoning attacks (helpnetsecurity Apr 2026). Invisible writes = user can't audit. Agent injects false fact → all future agents trust it as ground truth | **MEM-03 mandatory**: preview before persist; default `auto_extract: false` per agent; user enables opt-in |
| ANTI-03 | **"Hide agent tool calls to reduce noise"** (Cursor/Copilot default) | Cleaner UI. Less scary for non-technical users | Breaks PROJECT.md promise ("equal quality of experience regardless of who's a person"). Hidden actions = hidden costs + hidden risks. Cline shipped the opposite (show everything) and won trust | **TC-01 mandatory**: visible by default, collapsible (TC-02), groupable (TC-03), truncatable (TC-04) — but never hidden |
| ANTI-04 | **"One tap to install any agent, no config"** (Slack-app style) | Fast onboarding | Hides what permissions the agent has, what memory it writes, what tools it can call, what costs it incurs. Creates surprise bills + surprise actions | Show an **install sheet** (like iOS app permissions): "This agent requests → tool: Bash (destructive), memory: write archival, estimated cost: $0.03 per reply". User approves explicitly. See CN-01..05 |
| ANTI-05 | **"Cross-agent memory by default"** (any agent reads any agent's facts) | Makes "compound" feel automatic | Privacy violation by construction. User's Claude-therapy agent's facts should NOT flow to their work GPT-coder agent. Cross-agent read must be user-scoped, opt-in, per-pair | **MEM-04 with default-deny**: user toggles which pairs share. Default: per-agent silos (with provenance MEM-01 in place for v3.4 expansion) |
| ANTI-06 | **"Agent autonomy level slider"** (low/med/high-autonomy toggle) | Seems to give user control | Pseudo-control: "medium autonomy" is undefined. Real control = per-action approval (TC-05) + per-tool policy (MCP-04), not a vague slider | Use **per-tool policy** instead (TC-05 + MCP-04): user decides per-tool whether to require approval |
| ANTI-07 | **"Unlimited agents per thread"** | Seems flexible | UX falls apart at 10+ agents (attribution, presence indicators, routing). 2026 Slack/Discord threads with 10+ bots are widely reported as chaos | **Soft cap at ~5 agents per thread** (warning, not hard block). MA-01..08 scale to this comfortably |
| ANTI-08 | **"Agent-initiated DMs"** (agent can create a new thread with user, unsolicited) | "Proactive assistance" | Spam risk. Permission escalation (an agent you used once now messages you unprompted). Must require explicit opt-in per agent | Require user to be the thread-creator for first contact with a given agent. Agents can post to existing threads only |
| ANTI-09 | **"Built-in agent marketplace with payments"** | Monetization path | Out of scope per PROJECT.md Out-of-Scope. Federation-hostile: one marketplace = one gatekeeper. Let third parties build directories | Agent Cards (DISC-01..05) are discoverable via well-known URLs; users add by URL (CN-02) or search external directories. No first-party marketplace |
| ANTI-10 | **"Proprietary connector protocol for our ecosystem"** | Lock-in | Contradicts open-standard constraint (PROJECT.md). MCP has 97M/mo, A2A has 50+ enterprises — we bridge, not compete | A6 MCP bridge + connector SDK that speaks MCP/A2A natively; no proprietary-only tools |

---

## Feature Dependencies

```
v3.2 shipped: AI-01 (tool calls), AI-02 (@mentions), AI-03 (discovery),
              AI-04 (memory context), AI-05 (archival), PLT-05 (deeplinks),
              MSG-02 / MSG-09 (thread + agent badges), INT-01..05 (interactive blocks),
              STR-01..04 (streaming), NET-04..06 (Ed25519 identity)

Multi-agent UX:
  MA-01 (distinct identity)  ──builds-on──> MSG-09
  MA-02 (parallel typing)    ──builds-on──> STR-02, MIXD-03
  MA-03 (always-attributed)  ──modifies──> MSG-02 consecutive-sender rule
  MA-04 (tool attribution)   ──builds-on──> AI-01, TOOL-01..05
      └──depends-on──> MA-01 (distinct identity must render first)
  MA-05 (mention routing)    ──NEW SERVER──> AgentRegistry gate; uses AI-02
      └──BLOCKS──> ANTI-01 runaway loops
  MA-06 (silence)            ──NEW SERVER────────────> new endpoint + UI
  MA-07 (remove)             ──builds-on──> existing participant ops
  MA-08 (agent-to-agent viz) ──builds-on──> MSG-02 reply-indicator

Connector flow:
  CN-01 (add from discovery) ──builds-on──> AI-03
  CN-02 (URL install)        ──builds-on──> PLT-05 (extend verb)
      └──depends-on──> AU-01 (signed handshake)
  CN-03 (QR pairing)         ──NEW SERVER──> pairing endpoint
      └──depends-on──> AU-02 (paste-token underneath)
  CN-04/05 (status/revoke)   ──builds-on──> AgentRegistry data

MCP bridge:
  MCP-01 (MCP-as-agent)      ──builds-on──> COMP-02 spec + AI-03
      └──depends-on──> A6 MCP bridge connector (v3.3 scope item)
      └──depends-on──> DIFF-07 (compilation path)
  MCP-02 (tool attribution)  ──builds-on──> MA-04
  MCP-03 (MCP Apps UI)       ──builds-on──> CONT-04, INT-01..05
      └──requires──> new block type or extension to existing catalog
  MCP-04 (per-tool consent)  ──builds-on──> INT-04 (ApprovalBlock)

Tool transparency:
  TC-01 (visible default)    ──verifies──> TOOL-04 survives connector bridges
  TC-02 (collapsed)          ──already-works in──> AI-01 (verify at scale)
  TC-03 (grouped)            ──builds-on──> AI-01 + new grouping logic
  TC-04 (truncation)         ──builds-on──> MSG-05
  TC-05 (approve-before-run) ──builds-on──> INT-04 + new policy store
      └──depends-on──> MCP-04 share the same policy model

Compound memory (v3.3 groundwork → v3.4 UX):
  MEM-01 (provenance)        ──extends──> MEMO-04 schema
  MEM-02 (connector writes)  ──NEW SDK METHOD──> connector pushes to server
      └──depends-on──> MEM-01 (must tag on write)
  MEM-03 (preview)           ──builds-on──> MEM-01, AI-04
  MEM-04 (cross-agent read)  ──builds-on──> MEM-01 + new ACL
      └──BLOCKS──> ANTI-05 silo-default violation
  MEM-05 (export)            ──builds-on──> AI-05 (reads facts)

Auth:
  AU-01 (signed handshake)   ──builds-on──> SECU-01, NET-04..06
  AU-02 (paste token)        ──builds-on──> AU-01
  AU-03 (OAuth browser)      ──NEW──> separate auth flow
  AU-04 (API key local)      ──CONNECTOR-SIDE──> no server change

Differentiators compose on top:
  DIFF-01 (signed tool calls) ──builds-on──> AU-01 + TOOL events
  DIFF-02 (per-turn memory viz) ──builds-on──> AI-04 + MEM-01
  DIFF-03 (replaceable agents) ──builds-on──> MEM-01 + MEM-02 + MEM-04
  DIFF-04 (parallel conference) ──builds-on──> MA-02 + new layout
  DIFF-05 (HandoffBlock)       ──NEW BLOCK TYPE──> extends CONT-02/03
  DIFF-06 (Python SDK)         ──NEW──> A5 milestone item
  DIFF-07 (MCP compilation)    ──NEW──> A6 milestone item
```

### Critical Dependency Notes

- **MA-05 (@-mention routing) is a security-class prerequisite**, not a nice-to-have. Without server-side mention gating, ANTI-01 (runaway agent loops) happens by default. This must ship before any multi-agent thread is usable.
- **MEM-01 (provenance) must ship before any cross-agent memory feature** (MEM-02, MEM-04, DIFF-03). Writing to a shared memory store without source tags creates an un-rewindable mess that v3.4 would inherit.
- **AU-01 (signed handshake) gates AU-02/03/04 and DIFF-01**. Without it, connector identity is bearer-token-only and one leaked token = full impersonation.
- **A6 MCP bridge is load-bearing for MCP-01/02/03/04**. Without the connector, MCP servers remain theoretical in COMP-02 (protocol says they can be agents; nothing makes them agents).
- **A5 Python SDK (DIFF-06) is ecosystem-unlocking but not blocking**. TypeScript SDK (v3.2 state) handles Claude Code + OpenAI; Python SDK opens LangChain/CrewAI/LangGraph/custom-Python-agent universes. v3.3 includes it; v3.3 is shippable without it (just narrower).

---

## MVP Definition for v3.3

### P1 — Must Ship (the milestone fails without these)

Core multi-agent thread usability + connector ratification:

1. **MA-01, MA-02, MA-03, MA-04** — distinct identity, parallel indicators, always-attributed messages, tool attribution. Without these, N>1 agents in a thread is unreadable.
2. **MA-05** — server-side @-mention routing gate. Blocks ANTI-01 runaway loops. Security-class.
3. **MA-06, MA-07** — silence + remove agent. Essential escape hatches.
4. **CN-01, CN-02, CN-04, CN-05** — add from discovery, URL invite, list, revoke. Full connector lifecycle.
5. **AU-01, AU-02** — signed handshake + paste-token. Minimum auth for technical-user onboarding.
6. **TC-01, TC-02** — tool calls visible + collapsed by default. Verify v3.2 AI-01 still works under connector-emitted volume.
7. **MCP-01, MCP-02** — MCP server appears as agent participant with attributed tool calls. Delivers A6 milestone item.
8. **MEM-01, MEM-02** — memory provenance + connector memory write. Groundwork for v3.4. Without these, "compound memory" claim is hollow.

### P2 — Should Ship (strong v3.3, acceptable to defer if phase pressure)

1. **MA-08** — agent-to-agent visibility. Nice to have, but MA-01..05 are enough for v3.3.
2. **CN-03** — QR pairing. `paste-token` (AU-02) is sufficient for technical users; QR is polish for laptop-to-phone.
3. **TC-03, TC-04, TC-05** — grouped calls, truncation, per-tool policy. Claude Code generates enough tool calls that these matter; defer only if v3.2 AI-01 already scales well.
4. **MCP-03, MCP-04** — MCP Apps inline UI + per-tool consent. MCP Apps is very new (Jan 2026); ecosystem is small. MCP-04 is a subset of TC-05.
5. **MEM-03, MEM-05** — preview writes + export. Preview is important for trust; export is polish.
6. **AU-03, AU-04** — OAuth browser flow, API-key storage. AU-04 is connector-side (per-connector concern); AU-03 is hosted-runtime only.
7. **DIFF-06** — Python SDK (A5 scope item). Opens the Python ecosystem but TypeScript is enough for reference.
8. **DIFF-07** — MCP compilation (A6 scope item). Overlaps with MCP-01 but framed as reusable library.

### P3 — Defer to v3.4+ (compound memory full UX)

1. **MEM-04** — cross-agent read scope. Core v3.4 feature. Provenance (MEM-01) is the v3.3 groundwork.
2. **DIFF-02** — per-turn memory visualization. v3.4 polish.
3. **DIFF-03** — replaceable agents with inherited memory. v3.4 signature feature.
4. **DIFF-04** — parallel agent conference side-by-side view. Differentiator polish.
5. **DIFF-05** — HandoffBlock. New block type — v3.4 or later.
6. **DIFF-01** — Ed25519-signed tool calls. Nice-to-have hardening; TC-01 (visibility) already gives 80% of the trust story.

---

## Feature Prioritization Matrix

| # | Feature | User Value | Impl Cost | Priority |
|---|---------|------------|-----------|----------|
| MA-01 | Distinct identity per agent | HIGH | LOW | P1 |
| MA-02 | Parallel typing indicators | HIGH | LOW | P1 |
| MA-03 | Always-attributed messages | HIGH | LOW | P1 |
| MA-04 | Tool-call attribution | HIGH | LOW | P1 |
| MA-05 | @-mention server routing | HIGH (blocks runaway) | MEDIUM | P1 |
| MA-06 | Silence agent | HIGH | MEDIUM | P1 |
| MA-07 | Remove agent | MEDIUM | LOW | P1 |
| MA-08 | Agent-to-agent visibility | MEDIUM | LOW | P2 |
| CN-01 | Add-from-discovery | HIGH | MEDIUM | P1 |
| CN-02 | URL invite | HIGH | LOW | P1 |
| CN-03 | QR pairing | MEDIUM | MEDIUM | P2 |
| CN-04 | Connector list | HIGH | LOW | P1 |
| CN-05 | Revoke connector | HIGH | LOW | P1 |
| MCP-01 | MCP-as-agent participant | HIGH | MEDIUM | P1 |
| MCP-02 | MCP tool attribution | HIGH | LOW | P1 |
| MCP-03 | MCP Apps inline UI | MEDIUM | HIGH | P2 |
| MCP-04 | Per-tool consent | MEDIUM | MEDIUM | P2 |
| TC-01 | Visible by default | HIGH | LOW (verify) | P1 |
| TC-02 | Collapsed default | HIGH | LOW (verify) | P1 |
| TC-03 | Grouped by turn | MEDIUM | MEDIUM | P2 |
| TC-04 | Truncation | MEDIUM | LOW | P2 |
| TC-05 | Approve-before-run policy | HIGH | HIGH | P2 |
| MEM-01 | Memory provenance | HIGH (v3.4 blocker) | MEDIUM | P1 |
| MEM-02 | Connector memory write | HIGH (v3.4 blocker) | MEDIUM | P1 |
| MEM-03 | Preview writes | MEDIUM | MEDIUM | P2 |
| MEM-04 | Cross-agent read | HIGH | HIGH | **P3 (v3.4)** |
| MEM-05 | Memory export | MEDIUM | LOW | P2 |
| AU-01 | Signed handshake | HIGH (security) | LOW | P1 |
| AU-02 | Paste-token | HIGH | LOW | P1 |
| AU-03 | OAuth browser | MEDIUM | HIGH | P2 |
| AU-04 | API-key local | HIGH | LOW | P2 (connector-side) |
| DIFF-01 | Signed tool calls | MEDIUM | MEDIUM | P3 |
| DIFF-02 | Per-turn memory viz | HIGH | MEDIUM | **P3 (v3.4)** |
| DIFF-03 | Replaceable agents | HIGH (signature) | HIGH | **P3 (v3.4)** |
| DIFF-04 | Parallel conference | MEDIUM | MEDIUM | P3 |
| DIFF-05 | HandoffBlock | MEDIUM | MEDIUM | P3 |
| DIFF-06 | Python SDK | HIGH (ecosystem) | HIGH | P2 |
| DIFF-07 | MCP compilation | HIGH | HIGH | P2 |

---

## Competitor Feature Analysis

| Feature | Slack (multi-bot) | Claude Desktop / ChatGPT (MCP) | Cline / Continue / Aider | AINM v3.3 Approach |
|---------|-------------------|--------------------------------|--------------------------|--------------------|
| Multi-agent in one conversation | Yes (each bot own avatar via separate app registration) | Limited — one LLM + MCP tools; not multiple LLMs as peers | No (single agent per workspace) | **First-class**: N agents as equal participants (MA-01..08) |
| Tool call visibility | Per-bot; inconsistent | MCP spec supports; UX varies by host | Visible + collapsible + diff-approval (best-in-class) | Match Cline: visible, collapsible, truncated, grouped, approvable (TC-01..05) |
| Connector install | App directory (first-party marketplace) | Config file + restart (Claude Desktop) / MCP registry (ChatGPT) | Extension store | URL + QR + discovery-driven; no first-party marketplace (CN-01..05, ANTI-09) |
| Memory across agents | Not supported | ChatGPT: per-account silo. Claude: per-project silo | None (per-session) | Provenance-tagged shared store with user-scoped ACL (MEM-01..05; v3.4 cross-agent full) |
| Agent-to-agent messaging | Bots don't talk to bots | No | No | **Visible** to user (MA-08); @-mention gated (MA-05) |
| Approval before destructive tool | Not standardized | Per-tool manual config | Per-step default (Cline) | Per-tool, per-agent policy (TC-05, MCP-04) |
| Runaway loop protection | Via app-level rate limits | MCP sandbox + iframe | Per-step approval pauses loop naturally | @-mention gate (MA-05) + per-tool policy (TC-05) + runtime budgets (out of scope v3.3, future) |
| Signed identity | TLS + workspace token | OAuth | API key | Ed25519 per-message (existing) + signed handshake (AU-01) + signed tool calls (DIFF-01, P3) |
| MCP servers as participants | N/A | Tools, not participants | N/A | **Yes** — MCP server = agent-like participant with tool-only Agent Card (MCP-01) |

---

## Confidence Assessment by Area

| Area | Confidence | Reason |
|------|------------|--------|
| Multi-agent UX table stakes (MA-*) | HIGH | Verified against Slack/Discord multi-bot patterns + 2026 agentic-UX sources + Cline/Cursor precedent |
| Connector onboarding (CN-*, AU-*) | MEDIUM-HIGH | Verified against OpenClaw 2026, OAuth device flow IETF draft, Slack app install, VSCode extension sign-in. Some details (QR vs paste-token threshold) are extrapolated |
| MCP bridge mental model (MCP-*) | HIGH | Direct verification against MCP spec + MCP Apps (Jan 2026) + industry mental-model writeup ("hosts orchestrate, servers own side effects"). COMP-02 already in AINM spec |
| Tool call transparency (TC-*) | HIGH | Cline's visible-by-default approach is well-documented industry winner; v3.2 AI-01 already ships the foundation |
| Memory features (MEM-*, DIFF-02/03) | MEDIUM-HIGH | Provenance verified via arxiv PROV-AGENT 2026, Cisco 2026 memory-attack report, mem0.ai State of Agent Memory 2026. Specific MEM-04 cross-agent ACL design is an AINM-specific extrapolation (v3.4 scope) |
| Anti-features (ANTI-*) | HIGH | Every anti-feature backed by a documented 2026 failure case or well-established industry anti-pattern |

---

## Gaps / Open Questions

- **Tool call budget enforcement** — per-turn / per-thread token and action budgets. Oracle + Modexa 2026 call this out as mandatory; AINM could add per-thread `$` budget in v3.4. Not in v3.3 scope but flagged.
- **MCP Apps iframe rendering on iOS** — MCP Apps sandboxed iframes assume web host. iOS native client needs a strategy: embed WKWebView? map to native A2UI components via MCP `_meta.ui.resourceUri` declarative hints? MCP-03 complexity is HIGH partly because this is unsolved.
- **Cost transparency per connector** — users want to know "this reply cost $0.03". Not a v3.3 requirement but closely coupled to TC-05 approve-before-run. Flagged for v3.4.
- **Multi-device consistency** — if a user has AINM on iPhone + iPad, connectors should pair once. Out of scope for v3.3 (iOS-only per PROJECT.md); relevant for v3.4+.

---

## Sources

- [Agentic Design Patterns — Chat Interface Patterns](https://agentic-design.ai/patterns/ui-ux-patterns/chat-interface-patterns)
- [Smashing Magazine — Designing For Agentic AI: Practical UX Patterns For Control, Consent, And Accountability (Feb 2026)](https://www.smashingmagazine.com/2026/02/designing-agentic-ai-practical-ux-patterns/)
- [Must-Know Agentic Design Patterns for 2026 — ProCreator](https://medium.com/procreator-design/what-are-the-must-know-agentive-design-patterns-for-2026-21cf34839a01)
- [MCP Apps — Bringing UI Capabilities To MCP Clients (Jan 26, 2026)](https://blog.modelcontextprotocol.io/posts/2026-01-26-mcp-apps/)
- [MCP Apps — Claude support (The Register, Jan 2026)](https://www.theregister.com/2026/01/26/claude_mcp_apps_arrives/)
- [MCP Apps are here — WorkOS (Jan 27, 2026)](https://workos.com/blog/2026-01-27-mcp-apps)
- [MCP Standard and Ecosystem in 2026 — Apify](https://use-apify.com/blog/mcp-standard-ecosystem-2026)
- [Mem0 vs Letta (MemGPT) — Vectorize 2026](https://vectorize.io/articles/mem0-vs-letta)
- [State of AI Agent Memory 2026 — Mem0](https://mem0.ai/blog/state-of-ai-agent-memory-2026)
- [Agentic AI memory attacks spread across sessions and users — Help Net Security (Apr 2026)](https://www.helpnetsecurity.com/2026/04/14/idan-habler-cisco-agentic-ai-memory-attacks/)
- [PROV-AGENT — Unified Provenance for Tracking AI Agent Interactions (arxiv 2025, updated 2026)](https://arxiv.org/html/2508.02866v2)
- [How Claude Memory Works in 2026 — Shareuhack](https://www.shareuhack.com/en/posts/claude-memory-feature-guide-2026)
- [Claude Memory 2026 Complete Guide — Lumichats](https://lumichats.com/blog/claude-memory-2026-complete-guide-how-to-use)
- [Cline — Autonomous coding agent (GitHub)](https://github.com/cline/cline)
- [Aider vs Cline vs Continue — vibecodemeta 2026](https://vibecodemeta.com/blog/aider-vs-cline-vs-continue/)
- [Continue vs Aider vs Cline — Augment Code](https://www.augmentcode.com/tools/continue-vs-aider-vs-cline-private-ai-coding-assistants-for-regulated-teams)
- [When AI Agents Collide — Multi-Agent Orchestration Failure Playbook 2026 — Cogent](https://cogentinfo.com/resources/when-ai-agents-collide-multi-agent-orchestration-failure-playbook-for-2026)
- [$47,000 AI Agent Failure — Tech Startups](https://techstartups.com/2025/11/14/ai-agents-horror-stories-how-a-47000-failure-exposed-the-hype-and-hidden-risks-of-multi-agent-systems/)
- [Stop the Loop — dev.to, 2026](https://dev.to/alessandro_pignati/stop-the-loop-how-to-prevent-infinite-conversations-in-your-ai-agents-ekj)
- [Multi-Agent Frameworks for Enterprise AI 2026 — Adopt.ai](https://www.adopt.ai/blog/multi-agent-frameworks)
- [LangGraph vs CrewAI vs OpenAI Agents SDK 2026 — Particula](https://particula.tech/blog/langgraph-vs-crewai-vs-openai-agents-sdk-2026)
- [Handoffs — LangChain docs](https://docs.langchain.com/oss/python/langchain/multi-agent/handoffs)
- [Multi-Agent Accountability Co-signs — dev.to](https://dev.to/piqrypt/multi-agent-accountability-who-co-signs-the-handoff-between-your-crewai-agents-gi4)
- [OAuth Cross-Device Security BCP — IETF draft](https://datatracker.ietf.org/doc/draft-ietf-oauth-cross-device-security/)
- [A2A Agent Card — Google Cloud Marketplace](https://docs.cloud.google.com/marketplace/docs/partners/ai-agents/agent-card)
- [Slack — Enabling Interactions with Bots](https://api.slack.com/bot-users)
- [Running Multiple AI Agents as Slack Teammates (Apr 2026)](https://medium.com/@would-you-kindly/how-to-create-a-multi-agent-slack-channel-with-openclaw-82b7bec55374)

---

*Feature research for: AINM v3.3 Universal Agent Client*
*Researched: 2026-04-18*
