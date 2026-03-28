---
phase: 2
slug: agent-native-features
status: draft
shadcn_initialized: false
preset: none
created: 2026-03-28
---

# Phase 2 -- UI Design Contract (Protocol Rendering Semantics)

> Visual and interaction contract for the AINM protocol specification. Phase 2 is spec-only (no application code), so this contract defines the **client rendering semantics** that the protocol spec prescribes for conforming clients. These are the visual and interaction rules that go INTO the spec documents.

---

## Design System

| Property | Value |
|----------|-------|
| Tool | none -- protocol spec project, not a frontend app |
| Preset | not applicable |
| Component library | not applicable -- spec defines a declarative component catalog (D-08) |
| Icon library | not applicable -- spec does not prescribe icon choices |
| Font | not applicable -- spec does not prescribe font choices |

**Note:** This UI-SPEC defines protocol-level rendering semantics, not application-level design tokens. The protocol spec defines WHAT interactive blocks exist, their fields, their states, and their interaction patterns. Conforming clients choose their own design systems but MUST implement the interaction contracts below.

---

## Spacing Scale

Not applicable for protocol specification. Clients implement their own spacing.

Exceptions: none

---

## Typography

Not applicable for protocol specification. Clients implement their own typography.

---

## Color

### Protocol-Defined Semantic Styles

The protocol defines semantic style tokens for interactive blocks. These are NOT hex values -- they are semantic names that clients map to their own design system.

| Semantic Style | Usage | Required Elements |
|----------------|-------|-------------------|
| `primary` | Default call-to-action, affirmative action | ButtonBlock (D-05), ApprovalBlock approve option, FormBlock submit |
| `secondary` | Alternative/neutral action | ButtonBlock secondary style, ApprovalBlock "Request Changes" option, TableBlock row actions |
| `danger` | Destructive or rejecting action | ButtonBlock danger style, ApprovalBlock reject option |

**Source:** RESEARCH.md ButtonBlock `style` field, ApprovalBlock `options[].style` field.

These three semantic styles are the ONLY style tokens the protocol defines. Clients MUST support all three. The spec does not prescribe colors -- it prescribes intent.

---

## Interactive Block Rendering Contracts

### Contract 1: ButtonBlock

| Property | Spec Value | Source |
|----------|------------|--------|
| Required fields | `label`, `action_id` | RESEARCH.md ButtonBlock schema |
| Optional fields | `style` (primary/secondary/danger), `disabled` (boolean, default false), `confirmation` (string) | RESEARCH.md ButtonBlock schema |
| Fallback format | `[Button: {label}]` | D-08, Phase 1 mandatory fallback (D-14) |
| Interaction | Click triggers ActionBlock message in thread (D-07) | CONTEXT.md D-07 |
| Confirmation gate | If `confirmation` is non-null, client MUST show the confirmation text and require explicit approval before sending the ActionBlock | RESEARCH.md ButtonBlock fields |
| Disabled state | If `disabled: true`, button is rendered but not clickable; no ActionBlock is sent | RESEARCH.md ButtonBlock fields |

### Contract 2: FormBlock

| Property | Spec Value | Source |
|----------|------------|--------|
| Required fields | `action_id`, `fields[]` | RESEARCH.md FormBlock schema |
| Optional fields | `title`, `submit_label` (default: "Submit") | RESEARCH.md FormBlock schema |
| Field types (8 total) | `text`, `textarea`, `number`, `select`, `multiselect`, `checkbox`, `date`, `datetime` | RESEARCH.md recommended field types |
| Field properties | `name`, `label`, `field_type`, `required`, `default`, `placeholder`, `validation` (regex), `options` (for select types) | RESEARCH.md FormBlock field schema |
| Fallback format | `[Form: {title} - fields: {field names comma-separated}]` | RESEARCH.md FormBlock example |
| Interaction | Submit triggers ActionBlock with `data` containing field name-value pairs (D-07) | CONTEXT.md D-07 |
| Validation | Client MUST validate `required` fields before submission; client SHOULD validate `validation` regex before submission | RESEARCH.md field properties |

### Contract 3: TableBlock

| Property | Spec Value | Source |
|----------|------------|--------|
| Required fields | `columns[]`, `rows[]` | RESEARCH.md TableBlock schema |
| Optional fields | `title`, `row_actions[]` | RESEARCH.md TableBlock schema |
| Column properties | `key`, `label`, `sortable` (boolean), `align` (left/right/center) | RESEARCH.md TableBlock columns |
| Row actions | Array of `{ action_id, label, style }` -- rendered as action buttons per row | RESEARCH.md TableBlock row_actions |
| Fallback format | Markdown table with headers and all rows | RESEARCH.md TableBlock example |
| Sorting | Client-side only; if `sortable: true` on a column, client MUST allow toggling sort order | RESEARCH.md TableBlock capabilities |
| Pagination | Not in v1 -- all rows are inline | RESEARCH.md: "Pagination deferred to future versions" |
| Interaction | Row action click triggers ActionBlock with row data context (D-07) | CONTEXT.md D-07 |

### Contract 4: ApprovalBlock

| Property | Spec Value | Source |
|----------|------------|--------|
| Required fields | `action_id`, `title`, `status`, `options[]` | RESEARCH.md ApprovalBlock schema |
| Optional fields | `description` | RESEARCH.md ApprovalBlock schema |
| Status values | `pending` (initial), `approved`, `rejected`, `changes_requested` | RESEARCH.md ApprovalBlock states |
| Option properties | `value`, `label`, `style` (primary/secondary/danger) | RESEARCH.md ApprovalBlock options |
| Fallback format | `[Approval: {title} - Status: {status}]` | RESEARCH.md ApprovalBlock example |
| Immutability | Once status is non-pending, the block is immutable -- client MUST NOT allow further actions | RESEARCH.md: "Once resolved, the ApprovalBlock is immutable" |
| Interaction | Option selection triggers ActionBlock with `data: { value: "{selected_value}" }` (D-07) | CONTEXT.md D-07 |
| Server enforcement | Server MUST reject ActionBlocks targeting resolved ApprovalBlocks | RESEARCH.md Pitfall 4 |

### Contract 5: ActionBlock (Response Block)

| Property | Spec Value | Source |
|----------|------------|--------|
| Required fields | `ref_message_id`, `ref_block_index`, `action_id`, `data` | RESEARCH.md ActionBlock schema |
| Fallback format | `[Action: clicked '{label}']` or `[Action: submitted '{form_title}']` | RESEARCH.md ActionBlock example |
| Visibility | ActionBlocks are thread messages visible to ALL participants (D-07) | CONTEXT.md D-07 |
| Deduplication | Server MUST deduplicate by message `id` (UUIDv7) | RESEARCH.md Pitfall 4 |

---

## Streaming Rendering Contracts

### Contract 6: Stream Start (Placeholder Layout)

| Property | Spec Value | Source |
|----------|------------|--------|
| Event | `ainm.stream.start` | RESEARCH.md Pattern 1 |
| Client behavior | On receiving stream-start, client MUST render placeholders for ALL declared blocks immediately (D-03) | CONTEXT.md D-03 |
| Block layout | `blocks[]` array declares type, version, source for each block | RESEARCH.md streaming lifecycle |
| Layout immutability | Block layout is immutable for the stream duration -- no blocks added or removed mid-stream | RESEARCH.md Pitfall 1 |
| Parallel blocks | Client MUST support rendering multiple blocks streaming simultaneously (D-03) | CONTEXT.md D-03 |

### Contract 7: Stream Delta (Incremental Rendering)

| Property | Spec Value | Source |
|----------|------------|--------|
| Event | `ainm.stream.delta` | RESEARCH.md Pattern 1 |
| Addressing | `block_index` integer referencing position in stream-start `blocks[]` array (D-04) | CONTEXT.md D-04 |
| Granularity | Token-level: 1 to few tokens per delta (D-01) | CONTEXT.md D-01 |
| Streamable blocks | TextBlock and CodeBlock only in v1 | RESEARCH.md Open Question 1 |
| Client behavior | Append delta content to the referenced block's rendered output in real-time | RESEARCH.md streaming lifecycle |

### Contract 8: Stream End (Collapse)

| Property | Spec Value | Source |
|----------|------------|--------|
| Event | `ainm.stream.end` | RESEARCH.md Pattern 1 |
| Status values | `completed`, `cancelled` | RESEARCH.md collapse semantics |
| Collapse on completed | Client replaces streaming placeholder with the permanent signed message | RESEARCH.md collapse semantics |
| Collapse on cancelled | Partial content preserved with `cancelled` flag; client renders partial content with visual cancelled indicator | RESEARCH.md collapse semantics |
| Audit trail | `stream_id` preserved as metadata on the final message | RESEARCH.md collapse semantics |

### Contract 9: Stream Cancel (Client-Initiated)

| Property | Spec Value | Source |
|----------|------------|--------|
| Event | `ainm.stream.cancel` (client to server) | CONTEXT.md D-02 |
| Direction | Client-initiated only -- no server-initiated cancel (D-02) | CONTEXT.md D-02 |
| Client behavior | Client MUST provide a visible cancel affordance during active streaming | CONTEXT.md D-02 |
| Result | Partial message preserved with `cancelled` status | CONTEXT.md D-02 |

### Contract 10: Stream Error

| Property | Spec Value | Source |
|----------|------------|--------|
| Event | `ainm.stream.error` | RESEARCH.md streaming errors |
| Error code range | `-32040` to `-32049` | RESEARCH.md error code ranges |
| Client behavior | If `partial_preserved: true`, render partial content with error indicator; otherwise show error state | RESEARCH.md streaming errors |

---

## Tool Visibility Rendering Contracts

### Contract 11: Tool Call Events as Collapsible Indicators

| Property | Spec Value | Source |
|----------|------------|--------|
| Event types | `tool.call.started`, `tool.call.progress`, `tool.call.completed` | RESEARCH.md Pattern 3, TOOL-01/02/03 |
| Default render state | Collapsed -- showing tool name + current status | RESEARCH.md rendering hint |
| Expanded render state | Shows tool input, progress details, result summary, duration | RESEARCH.md rendering hint |
| Visibility | Visible to ALL thread participants (TOOL-04) | REQUIREMENTS.md TOOL-04 |
| Progress display | Optional `progress.current` / `progress.total` for determinate progress; `progress.detail` for text description | RESEARCH.md tool progress schema |
| Completed status | `success` or `error` -- client renders accordingly | RESEARCH.md tool completed schema |
| Duration display | `duration_ms` on completed events -- client SHOULD display human-readable duration | RESEARCH.md tool completed schema |
| Fallback | Tool events include a `content[]` array with TextBlock fallback for clients that do not support collapsible rendering | RESEARCH.md Pattern 3 |

---

## Memory Rendering Contracts

### Contract 12: Context Frame (Not Directly Rendered)

Context frames (`ainm.context.frame`) are delivered to agents, not rendered to humans. No client rendering contract.

### Contract 13: Memory Operations (Not Directly Rendered)

Memory JSON-RPC methods (`ainm.memory.store`, `ainm.memory.query`, `ainm.memory.evict`) are private agent-server operations. No client rendering contract. Memory queries are NOT visible to other participants (RESEARCH.md Open Question 3).

---

## Agent Card Contracts

### Contract 14: Agent Card Discovery and Display

| Property | Spec Value | Source |
|----------|------------|--------|
| Endpoint | Agent Card at URL from identity resolution `agent_card_url` field (D-10) | CONTEXT.md D-10 |
| Caching | HTTP ETag / If-None-Match (D-10) | CONTEXT.md D-10 |
| A2A compatibility | Top-level fields match A2A Agent Card spec; AINM extensions namespaced under `ainm` object | CONTEXT.md D-09, RESEARCH.md Pitfall 5 |
| Required A2A fields | `name`, `description`, `url`, `version`, `capabilities`, `skills` | RESEARCH.md Agent Card schema |
| AINM extension fields | `ainm.contentTypes`, `ainm.streaming`, `ainm.threading`, `ainm.memory`, `ainm.tools` | CONTEXT.md D-11, D-12 |
| Client rendering | Clients SHOULD display agent name, description, and capability summary when viewing participant info | RESEARCH.md Agent Card schema |

---

## Copywriting Contract

These are the protocol-level copy patterns that the spec defines. Conforming clients use these as defaults.

| Element | Copy | Source |
|---------|------|--------|
| Stream cancel affordance | "Stop generating" | Default for client-initiated cancel (D-02) |
| Streaming placeholder | "{Agent name} is typing..." per active block | Default for stream-start rendering |
| Tool call collapsed label | "{tool_name}: {status}" (e.g., "code_search: running") | RESEARCH.md tool event rendering |
| Tool call completed label | "{tool_name}: {result_summary} ({duration})" | RESEARCH.md tool completed schema |
| Approval pending state | Render all options as clickable; show "Pending" badge | RESEARCH.md ApprovalBlock states |
| Approval resolved state | Show selected option highlighted; grey out others; show "Approved" / "Rejected" / "Changes Requested" badge | RESEARCH.md ApprovalBlock immutability |
| Button confirmation prompt | "{confirmation}" (agent-provided text shown in modal/dialog) | RESEARCH.md ButtonBlock confirmation field |
| Form validation error | "This field is required" for required fields; "Invalid format" for regex validation failures | Default -- protocol does not prescribe validation copy |
| Empty block fallback | Render the `fallback` string from the block when block type is unsupported | Phase 1 D-14 mandatory fallback |
| Stream error state | "Generation interrupted -- partial response shown" (when `partial_preserved: true`) | Default for stream error rendering |
| Stream error state (no partial) | "Generation failed -- try again" | Default for stream error rendering |
| ActionBlock confirmation | Client MUST NOT prompt before ActionBlock -- confirmation is handled by ButtonBlock `confirmation` field | RESEARCH.md ActionBlock as thread message |

---

## Registry Safety

| Registry | Blocks Used | Safety Gate |
|----------|-------------|-------------|
| not applicable | Protocol spec project -- no component registries | not applicable |

**Note:** The protocol defines a static component catalog (D-08). The spec IS the catalog. No runtime registry, no third-party component sources.

---

## Interaction State Machine: ApprovalBlock

```
                    +-------------+
                    |   pending   |
                    +------+------+
                           |
              +------------+------------+
              |            |            |
              v            v            v
        +---------+  +----------+  +------------------+
        | approved|  | rejected |  | changes_requested|
        +---------+  +----------+  +------------------+
              (terminal)  (terminal)       (terminal)

All terminal states are immutable. Server rejects further actions.
```

---

## Interaction State Machine: Streaming Lifecycle

```
    [idle] --stream.start--> [streaming]
                                  |
                   +--------------+--------------+
                   |              |              |
              stream.end    stream.cancel   stream.error
              (completed)   (cancelled)     (error)
                   |              |              |
                   v              v              v
              [collapsed]   [partial +      [partial +
               (signed      cancelled       error
                message)     badge]          indicator]
```

---

## Interaction State Machine: Tool Call Lifecycle

```
    tool.call.started --> tool.call.progress (0..N) --> tool.call.completed
                                                              |
                                                    +---------+---------+
                                                    |                   |
                                                 success             error
                                                    |                   |
                                              [collapsed:          [collapsed:
                                               green checkmark]     red indicator]
```

---

## Summary of Protocol-Defined Visual Contracts

| Domain | Contracts Defined | Client MUST Implement |
|--------|-------------------|-----------------------|
| Interactive Blocks | 5 block types (Button, Form, Table, Approval, Action) | All 8 core block types (4 static from Phase 1 + 4 interactive) |
| Semantic Styles | 3 styles (primary, secondary, danger) | Map to client design system |
| Streaming | 5 events (start, delta, end, cancel, error) | Placeholder rendering, incremental append, collapse, cancel affordance |
| Tool Visibility | 3 event types (started, progress, completed) | Collapsible indicators with expand/collapse |
| Agent Cards | 1 discovery endpoint + AINM extensions | Display agent info; cache with ETag |
| Memory | 0 client-rendered elements | No rendering required |
| Fallback | Universal mandatory fallback on all blocks | Render fallback string for unsupported types |

---

## Checker Sign-Off

- [ ] Dimension 1 Copywriting: PASS
- [ ] Dimension 2 Visuals: PASS
- [ ] Dimension 3 Color: PASS
- [ ] Dimension 4 Typography: PASS
- [ ] Dimension 5 Spacing: PASS
- [ ] Dimension 6 Registry Safety: PASS

**Approval:** pending
