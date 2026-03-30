# Phase 11: Core Messaging UI - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-29
**Phase:** 11-core-messaging-ui
**Areas discussed:** Message thread design, Conversation list layout, Content block rendering, Composer & navigation

---

## Message Thread Design

### Bubble Style

| Option | Description | Selected |
|--------|-------------|----------|
| Chat bubbles | iMessage-style: user right-aligned accent, others left-aligned secondary | |
| Flat list (Slack-style) | All messages left-aligned with avatar + name + timestamp header | |
| Hybrid | Chat bubbles for human messages, flat/wide for agent messages | ✓ |

**User's choice:** Hybrid
**Notes:** Human messages in chat bubble, agent messages in flat list so it's easier to show tools, code blocks, etc.

### Agent Visual Distinction

| Option | Description | Selected |
|--------|-------------|----------|
| Icon + type label | Small colored badge next to name showing 'Agent' or agent type | ✓ |
| Color differentiation only | Agents get distinct accent color, layout difference signals it | |
| Avatar shape | Humans circular, agents rounded-square | |

**User's choice:** Icon + type label
**Notes:** None

### Message Grouping

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, group consecutive | Show avatar + name only on first in sequence, tighter spacing | ✓ |
| No, always show sender | Every message gets full avatar + name + timestamp header | |

**User's choice:** Yes, group consecutive
**Notes:** None

### Thread Creation

| Option | Description | Selected |
|--------|-------------|----------|
| New thread button + participant picker | FAB or nav bar opens composer with title and participant list | |
| Direct compose | Tap '+', type participants via @-mention in 'To:' field, thread on first send | ✓ |
| You decide | Claude picks based on protocol's thread.create flow | |

**User's choice:** Direct compose
**Notes:** None

---

## Conversation List Layout

### Row Content

| Option | Description | Selected |
|--------|-------------|----------|
| Standard messaging | Avatar, title, last message preview, relative timestamp, unread badge | ✓ |
| Rich with participant count | Same as standard + participant count and avatar stack | |
| Minimal | Title + timestamp only | |

**User's choice:** Standard messaging
**Notes:** None

### Sort Order

| Option | Description | Selected |
|--------|-------------|----------|
| Most recent activity | Newest messages float to top | ✓ |
| Pinned + recent | User can pin threads to top, rest by recency | |
| You decide | Claude picks simplest for v1 | |

**User's choice:** Most recent activity
**Notes:** None

### Empty State

| Option | Description | Selected |
|--------|-------------|----------|
| Illustration + CTA | SF Symbol, message, prominent 'Start a Conversation' button | ✓ |
| Just text | Centered message, no button | |
| You decide | Claude picks pattern | |

**User's choice:** Illustration + CTA
**Notes:** None

---

## Content Block Rendering

### Code Block Syntax Highlighting

| Option | Description | Selected |
|--------|-------------|----------|
| Native AttributedString | Lightweight regex-based highlighter, ~10 languages, no dependency | ✓ |
| Splash (SPM library) | John Sundell's library, Swift-focused, limited other languages | |
| TreeSitter via SPM | Accurate multi-language, heaviest dependency | |
| You decide | Claude picks approach | |

**User's choice:** Native AttributedString
**Notes:** None

### Image Block Display

| Option | Description | Selected |
|--------|-------------|----------|
| Aspect-fit with max height | Full width, 300pt max height, tap for fullscreen + zoom | ✓ |
| Thumbnail grid | Multiple images as grid, single full width | |
| You decide | Claude picks pattern | |

**User's choice:** Aspect-fit with max height
**Notes:** None

### Multi-Block Layout

| Option | Description | Selected |
|--------|-------------|----------|
| Vertical stack, full width | Blocks in sequence, each full width, natural reading flow | ✓ |
| Collapsible sections | Text inline, code/image collapsible | |
| You decide | Claude picks layout | |

**User's choice:** Vertical stack, full width
**Notes:** None

---

## Composer & Navigation

### Navigation Structure

| Option | Description | Selected |
|--------|-------------|----------|
| NavigationStack only | Conversation list root, push to detail, no tab bar | ✓ |
| Tab bar (3 tabs) | Conversations, Agents, Settings | |
| You decide | Claude picks based on feature set | |

**User's choice:** NavigationStack only
**Notes:** None

### Composer Design

| Option | Description | Selected |
|--------|-------------|----------|
| Simple text + send | Auto-growing TextField, send button, plain text, Markdown on render | ✓ |
| Rich composer with toolbar | Text field + Markdown formatting buttons + attachment | |
| You decide | Claude picks design | |

**User's choice:** Simple text + send
**Notes:** None

### Connection Status Placement

| Option | Description | Selected |
|--------|-------------|----------|
| Subtle nav bar subtitle | Small text under title, disappears when connected 5+ seconds | ✓ |
| Banner overlay | Colored banner slides down when disconnected | |
| You decide | Claude picks placement | |

**User's choice:** Subtle nav bar subtitle
**Notes:** None

### Light/Dark Mode

| Option | Description | Selected |
|--------|-------------|----------|
| Follow system | SwiftUI semantic colors, no in-app toggle | ✓ |
| In-app toggle | Settings override (Light / Dark / System) | |

**User's choice:** Follow system
**Notes:** None

---

## Claude's Discretion

- FileBlock download UX design (MSG-07)
- Markdown rendering approach for text blocks (MSG-03)
- Pagination implementation (50 messages, load-more on scroll-up)
- Offline mode UX details (PLT-02)
- Unread badge logic

## Deferred Ideas

None — discussion stayed within phase scope.
