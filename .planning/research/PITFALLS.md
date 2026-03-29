# iOS Reference Client Pitfalls Research

**Domain:** iOS Reference Client for AI-Native Messaging Protocol (AINM v3.3)
**Researched:** 2026-03-29
**Confidence:** HIGH (iOS platform constraints well documented by Apple; AINM integration pitfalls derived from protocol spec analysis; community patterns verified from Stream, Matrix iOS, and messaging-specific SwiftUI implementations)

---

## Critical Pitfalls

These mistakes cause rewrites, data loss, or fundamental architectural failures.

---

### Pitfall 1: RFC 8785 JSON Canonicalization Mismatch Between Swift and Rust

**What goes wrong:**
Messages signed by the iOS client fail signature verification on the Rust server (or vice versa). The Ed25519 signature is mathematically valid on the platform that produced it but invalid on the other. This failure is intermittent -- it only surfaces with certain message content (Unicode characters, non-ASCII keys in content block metadata, decimal numbers). ASCII-only test messages pass, creating false confidence.

**Why it happens:**
The AINM security spec (Section 4) requires RFC 8785 (JSON Canonicalization Scheme) to produce deterministic bytes for signing. RFC 8785 has two notoriously tricky requirements: (1) UTF-16 lexicographic key sorting (not UTF-8 byte order -- these differ for characters above U+FFFF), and (2) ECMAScript-compatible float-to-string conversion using the RYU algorithm. Swift's `JSONEncoder` with `.sortedKeys` does NOT produce RFC 8785 output -- it sorts keys by UTF-8 byte order and uses its own float formatting. Swift's `JSONSerialization` is similarly non-compliant. The Rust server uses `serde_json_canonicalizer` which follows RFC 8785 correctly. Two "sorted JSON" implementations that use different sort orders will produce different byte sequences for the same logical message, causing signature mismatch.

**How to avoid:**
- Write a dedicated RFC 8785 JCS implementation in Swift. The spec is ~15 pages. The critical details are: key sorting must use UTF-16 code unit comparison (not Unicode scalar comparison), and number formatting must match ECMAScript's `Number.prototype.toString()`. Do not use `JSONEncoder`/`JSONSerialization`.
- Port the official JCS test vectors (github.com/cyberphone/json-canonicalization, includes edge cases for Unicode, floats, and sorting) and run them as unit tests in the Swift package.
- Create a cross-platform signing integration test: sign a message on iOS, send the canonical bytes and signature to the Rust server, verify. And vice versa. This test must run in CI on every commit.
- AINM mostly avoids floats (timestamps are strings, IDs are strings), which reduces the float-formatting risk -- but do not rely on this; content block extensions may introduce numeric fields.

**Warning signs:**
- Signature verification failures that only happen with emoji, CJK characters, or accented text in messages
- Tests pass locally (iOS-to-iOS signing) but fail when the Rust server verifies the same message
- Messages with only ASCII content always work; messages with Unicode content sometimes fail
- The JCS output of the Swift implementation differs from the output of the `serde_json_canonicalizer` Rust crate for the same input

**Phase to address:**
Phase 1 (Foundation). JCS + Ed25519 signing must be the first thing built and cross-platform tested. Every other feature depends on being able to send verified messages.

---

### Pitfall 2: WebSocket Connection Lifecycle Mismanagement

**What goes wrong:**
The app works perfectly in foreground testing on WiFi but silently loses messages in real-world usage. Users experience: messages sent while offline vanishing, duplicate messages after reconnection, the app stuck in a "connecting" state after switching from WiFi to cellular, and missing messages that arrived while the app was backgrounded. The app may appear to be connected (no error visible) while actually disconnected.

**Why it happens:**
iOS aggressively suspends background apps -- there is no API to keep an arbitrary WebSocket connection alive in the background. `URLSessionWebSocketTask` connections are torn down when the app enters background (within ~30 seconds). WiFi-to-cellular transitions create a new network interface that silently kills the existing TCP connection. Developers make several compounding mistakes: (1) treating TCP connection open as "ready to send" without completing the `ainm.initialize` handshake, (2) not detecting that `URLSessionWebSocketTask.receive()` is single-shot (must be re-called after every message), (3) not monitoring network path changes with `NWPathMonitor`, (4) not replaying the AINM handshake on every reconnection.

**How to avoid:**
- Model the connection as an explicit state machine with these states: `disconnected` -> `connecting` (TCP) -> `handshaking` (ainm.initialize sent, waiting for ainm.initialized) -> `connected` (protocol ready) -> `disconnecting`. Only expose `send()` in the `connected` state. Queue messages sent in other states.
- Use `NWPathMonitor` to detect network interface changes and proactively reconnect. Do not wait for a send/receive failure to discover the connection is dead.
- Implement exponential backoff with jitter for reconnection: start 1s, max 30s, add random 0-500ms jitter to prevent thundering herd when the server restarts.
- On every reconnection, replay the full `ainm.initialize` / `ainm.initialized` handshake with capability negotiation. Never cache capabilities across sessions.
- Register for `UIApplication.willEnterForegroundNotification` and trigger immediate reconnection.
- Use `beginBackgroundTask(expirationHandler:)` on background entry for ~30 seconds to gracefully close the WebSocket and flush pending messages to the local queue.
- Implement server-side message replay: on reconnect, request messages since the last received `server_timestamp` to fill the gap.
- Wrap `URLSessionWebSocketTask.receive()` in an `AsyncThrowingStream` that automatically re-calls receive after each message. Handle stream termination (cancellation, error) explicitly.

**Warning signs:**
- Messages appear in a different order than sent
- "Connection lost" state that never recovers without force-quitting the app
- Messages sent while on cellular never arrive (WiFi worked fine)
- The receive loop silently stops after a single message

**Phase to address:**
Phase 1 (Foundation/Networking). The WebSocket state machine is the transport foundation. Build this before any UI.

---

### Pitfall 3: SwiftUI Message List Performance Collapse with Large Conversations

**What goes wrong:**
Scrolling becomes janky or the app crashes when a conversation exceeds ~1,000 messages. Initial load of a long conversation takes 3-5 seconds with a blank screen. Fast scrolling shows blank frames. Memory climbs to 500MB+ for a single conversation view, leading to jetsam termination on older devices.

**Why it happens:**
`LazyVStack` inside `ScrollView` does NOT recycle views the way `UITableView` does. It creates views lazily on scroll-down but retains all previously created views in memory. Testing shows memory exhaustion and crash around 5,000 messages on iPhone 11 Pro Max. Each AINM message can contain multiple content blocks (text, code, images, interactive forms) making each cell a complex view hierarchy. `defaultScrollAnchor(.bottom)` with large datasets causes a blank window on initial load because SwiftUI attempts to measure all content to determine the scroll position. Additionally, SwiftUI only retains the state of top-level views in a `ForEach` when they leave and re-enter the visible area -- nested view state (like form input in an interactive block) is lost.

**How to avoid:**
- Implement pagination from day one: load only the most recent 50 messages initially. Load older messages on scroll-up with a "load more" trigger.
- Maintain a view window of at most ~200 messages in the active SwiftUI view. When the user scrolls down past the window, remove messages from the top of the data source (and vice versa).
- Use an inverted scroll approach for chat: rotate the ScrollView 180 degrees, reverse the message order, rotate each cell 180 degrees back. This makes "scroll to bottom" the natural default position and eliminates `defaultScrollAnchor` issues.
- Make message views lightweight: use `.task` modifier to defer heavy rendering (Markdown parsing, syntax highlighting, image loading) until the cell is actually visible.
- Pre-render `AttributedString` from Markdown when messages are received (in the repository layer), cache in the model. Do not parse Markdown during scroll.
- Profile with Instruments (SwiftUI profiler) from Phase 2. Set a budget: 16ms per frame (60fps), <150MB memory for a 2,000-message conversation.
- Use `Equatable` conformance on message view models to prevent unnecessary redraws.

**Warning signs:**
- Memory usage climbs linearly as the user scrolls through a conversation (visible in Instruments)
- Frame drops during fast scrolling (visible in Instruments as >16ms layout passes)
- Blank frames appear when scrolling quickly
- App terminations in crash logs with `EXC_RESOURCE` or jetsam memory pressure codes
- Nested view state (form inputs inside interactive blocks) resets when scrolling away and back

**Phase to address:**
Phase 2 (Core UI). The message list architecture must be built with pagination and memory management from the start. Retrofitting pagination into a "load all messages" design is a full rewrite.

---

### Pitfall 4: Streaming Token Rendering Causes UI Thread Starvation

**What goes wrong:**
During agent streaming responses, the UI freezes: the user cannot scroll, tap buttons, or cancel the stream. Text appears in large chunks rather than smooth token-by-token flow. The "cancel stream" button does not respond. On older devices, the app may receive a watchdog termination for unresponsive main thread.

**Why it happens:**
Each `ainm.stream.delta` notification arrives as a WebSocket message (potentially 50-100+ per second for fast models). If every delta triggers a SwiftUI state update via `@Observable` or `@Published`, and the state change causes a view body re-evaluation, the main thread spends its entire budget on layout passes. AINM supports parallel streaming across multiple block indices within a single stream -- meaning deltas for a text block and a code block can interleave. If each delta for each block triggers an independent state update, the combinatorial effect multiplies the problem. Swift's `@Observable` macro triggers synchronous view updates on every property mutation.

**How to avoid:**
- Throttle UI updates: accumulate deltas in a buffer on a background actor/queue. Flush the buffer to the UI at most every 16-33ms (30-60fps). Use `DispatchSource.makeTimerSource` or a `Task` with `Task.sleep(nanoseconds:)` coalescing loop.
- Make the streaming text view a leaf view that manages its own `Text` content independently from the parent message list. This isolates delta updates from triggering re-layout of the entire list.
- Use `AttributedString` append operations rather than rebuilding the entire string on every token.
- For parallel block streaming, batch deltas by stream_id and flush all block updates in a single UI transaction.
- Process WebSocket messages on a background actor. Batch and throttle before dispatching to the `@MainActor`.
- Test the worst case early: an agent streaming 100+ tokens/second into 3 parallel blocks while the user is actively scrolling the conversation. This must be smooth.

**Warning signs:**
- UI becomes unresponsive during agent responses
- Xcode Organizer shows main thread hang reports
- Text appears in bursts rather than smooth character-by-character rendering
- The cancel button requires multiple taps to register
- CPU usage spikes to 100% main thread during streaming

**Phase to address:**
Phase 3 (Streaming). The throttling and batching architecture must be designed before implementing delta rendering. The streaming pipeline is: WebSocket -> background actor (batch/throttle) -> @MainActor (UI update).

---

### Pitfall 5: Ed25519 Private Key Loss or Inaccessibility

**What goes wrong:**
Users lose their cryptographic identity (and all ability to send verified messages) after app reinstall, device migration, or iOS update. Alternatively, the key becomes inaccessible when the device is locked, causing background message signing to fail silently. Or the key unexpectedly syncs to another device via iCloud Keychain, creating identity conflicts.

**Why it happens:**
Ed25519 (`Curve25519.Signing.PrivateKey`) has NO native Keychain `SecKey` representation -- unlike P-256 keys which the Keychain stores as first-class `kSecClassKey` items. Ed25519 keys must be stored as generic password items (`kSecClassGenericPassword`) containing the raw key bytes. This has three consequences developers miss: (1) default accessibility for generic passwords may make the key unavailable when the device is locked (`kSecAttrAccessibleWhenUnlocked` is the default, which breaks background signing), (2) generic passwords sync to iCloud Keychain by default unless explicitly prevented with `kSecAttrSynchronizable: false`, creating duplicate identities across devices, (3) app deletion removes Keychain items on iOS 13+ unless the item was shared via a Keychain access group with another installed app.

**How to avoid:**
- Store Ed25519 private key raw bytes (`privateKey.rawRepresentation`, 32 bytes) as `kSecClassGenericPassword` with these explicit attributes:
  - `kSecAttrAccessible: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly` (available after first unlock, survives lock screen, does NOT sync to iCloud, does NOT survive backup restore to different device)
  - `kSecAttrSynchronizable: false` (explicit -- do not rely on default)
  - Use a stable `kSecAttrAccount` key like `"com.ainm.identity.ed25519.private"`
- Reconstruct the key with `Curve25519.Signing.PrivateKey(rawRepresentation: storedBytes)`.
- Implement a key existence check on every app launch. If the key is missing, guide the user through re-registration or key recovery.
- Design a key recovery mechanism at the protocol level (out-of-band key backup, social recovery, or server-assisted recovery) before shipping. Without this, key loss = permanent identity loss.
- Use `KeychainAccess` library (in the stack) but verify its defaults match these requirements -- do not assume the library sets the correct accessibility.
- Test: install app, create identity, delete app, reinstall. Verify the key is gone (expected behavior) and the app handles it gracefully.

**Warning signs:**
- Users report "identity lost" after iOS update or device migration
- Message signing fails intermittently when the device is locked (key inaccessible)
- Users on multiple devices have the same identity key (iCloud sync leak)
- Key is present in unencrypted iTunes backups

**Phase to address:**
Phase 1 (Foundation/Security). Key storage is the foundation of identity. Get it right before building any signing, sending, or identity UI.

---

### Pitfall 6: GRDB Observation Triggering Excessive SwiftUI Redraws

**What goes wrong:**
The UI redraws the entire conversation list or message thread on every database write, even when the change is unrelated to the visible content. CPU spikes when messages arrive in a background conversation. Scrolling stutters when the database is being written to. The app feels sluggish despite having a fast network connection.

**Why it happens:**
GRDB's `ValueObservation` is powerful but coarse-grained by default. A common pattern is: observe "all messages in thread X" with `ValueObservation.tracking { db in try Message.filter(threadId: threadId).fetchAll(db) }`. When any message in that thread changes (new arrival, status update, read receipt), GRDB re-fetches the entire result set and SwiftUI diffs the whole array. With 500+ messages per thread, this is 500+ Equatable comparisons on every database write. Worse: if the observation is on the main queue (the default), the re-fetch blocks the UI thread. If you have separate observations for the conversation list and the message view, a single incoming message triggers two full re-fetches.

**How to avoid:**
- Use `ValueObservation` with narrow queries: observe only the visible page of messages (e.g., the most recent 50), not the entire thread. Update the observation when the user scrolls to a new page.
- Use GRDB's `DatabaseRegionObservation` for lightweight change detection -- it tells you "something changed in table X matching condition Y" without re-fetching. Combine with manual, targeted re-fetches.
- Schedule observations to deliver on a background queue (`.scheduling(.async(onQueue: .global()))`), then dispatch results to `@MainActor`. Never let GRDB fetch on the main thread for large result sets.
- For the conversation list, observe only the `conversations` table with preview fields (last message preview, timestamp, unread count). Do not join the full messages table.
- Use `fetchAll` with explicit column selection when you only need preview data -- avoid fetching large `content` JSONB columns for list views.
- Implement proper `Identifiable` and `Equatable` on your message view models so SwiftUI's diff is as cheap as possible (compare IDs, not content).

**Warning signs:**
- Instruments shows GRDB fetch operations on the main thread taking >10ms
- CPU spikes when messages arrive in conversations the user is not viewing
- The conversation list stutters when a background sync writes messages
- Memory spikes from re-fetching large result sets on every observation trigger

**Phase to address:**
Phase 2 (Core UI / Persistence). The observation and pagination strategy must be co-designed with the message list. They are the same problem.

---

### Pitfall 7: Not Respecting the ainm.initialize Handshake Contract

**What goes wrong:**
The iOS client sends protocol messages before completing the `ainm.initialize` / `ainm.initialized` exchange, and the server rejects them with error code -32001. Features work after a fresh app launch but break after reconnection because the client reuses stale capability information. Streaming-related messages are sent to a server session that did not negotiate streaming support.

**Why it happens:**
Developers treat the WebSocket `open` event as "ready to send protocol messages." But AINM transport spec (Section 4.2-4.3) requires a handshake exchange before any protocol method can be called. The handshake includes capability negotiation: both sides declare which features they support (`streaming`, `interactive`, `memory`, etc.). Only features declared by both sides are available for the session. If the client caches capabilities from a previous session and the server has changed (upgraded, downgraded, different config), the client may attempt operations the current session does not support.

**How to avoid:**
- Model the connection state explicitly. `tcpConnected` is NOT `protocolReady`. Only expose a `send(method:params:)` API after `ainm.initialized` response is received and parsed.
- Store negotiated capabilities in the session object, scoped to the current connection. Clear and re-negotiate on every reconnection.
- Queue outbound messages that arrive during the handshake phase. Flush the queue after `ainm.initialized`. Drop messages that require capabilities the new session does not support (and notify the caller).
- Handle handshake failure explicitly: if `ainm.initialize` returns an error (version mismatch, auth failure), surface a user-visible error. Do not silently retry forever.
- Validate each outbound message against the session's negotiated capabilities before sending.

**Warning signs:**
- Server logs show rejected messages with "session not initialized" or "capability not negotiated" errors
- Features work after fresh app launch but break after reconnection
- Intermittent "method not found" JSON-RPC errors for streaming or interactive methods
- The client sends `ainm.stream.cancel` to a session that did not negotiate streaming

**Phase to address:**
Phase 1 (Foundation/Networking). The handshake is the first thing that happens on the wire. Build it before any protocol method.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Load all messages into LazyVStack without pagination | Simpler code, works in demos with 20 messages | Memory crash at ~5K messages, scroll jank at 1K+ | Never -- pagination must be designed from day one |
| Use `JSONEncoder(.sortedKeys)` instead of proper RFC 8785 JCS | Ships faster, works for ASCII-only test messages | Signature failures on Unicode content, impossible to debug in production | Never -- cross-platform signing requires exact JCS compliance |
| Store Ed25519 key in UserDefaults | Quick prototyping, easy to inspect | Key exposed to unencrypted backups, no access control, App Store rejection risk | Only in `#if DEBUG` builds with a hard guard |
| Skip WebSocket reconnection logic | Faster initial development | App appears broken after any network hiccup, loses user trust immediately | Never -- reconnection is table stakes for any messaging app |
| Single global GRDB `ValueObservation` for all messages in a thread | Simple setup, single subscription | UI redraws everything on any DB change, CPU waste, jank | Only in very early prototype with <100 messages, must be refactored before first user test |
| Process every streaming delta synchronously on main thread | Simpler code path, no concurrency to manage | UI freezes during agent streaming, unresponsive to user input | Never -- throttling must be part of the streaming pipeline design |
| Hardcode server URL | Faster dev setup | Cannot test against different servers, blocks multi-server/federation testing, violates 12-factor principles | Acceptable in first 2 weeks of Phase 1, must be configurable before Phase 1 ends |
| Skip fallback rendering for unknown block types | Fewer view components to build | Blank spaces in messages when the server sends newer block types the client does not recognize | Never -- the protocol requires fallback rendering, and server evolution will produce unknown blocks |

## Integration Gotchas

Mistakes specific to connecting the iOS client to the existing AINM Rust server.

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| JSON-RPC dispatch | Treating notifications (no `id`) and requests (with `id`) identically; not correlating responses to pending requests | Maintain a `[Int: CheckedContinuation]` map for pending requests keyed by JSON-RPC `id`. Route incoming messages: if has `id` and `result`/`error` -> resume continuation; if has `method` -> dispatch to notification handler via `AsyncStream`. Use a `Sendable` actor for the pending map |
| Streaming lifecycle | Treating stream deltas as independent messages; losing track of which stream a delta belongs to; not pre-allocating block layout | Maintain a `[UUID: StreamState]` actor. On `ainm.stream.start`, allocate block buffers matching the declared layout array. On `ainm.stream.delta`, append to the correct block buffer by `block_index`. On `ainm.stream.end`, collapse buffers into a permanent signed message and persist to GRDB |
| CryptoKit vs. Rust ring compatibility | Assuming the signature format is automatically compatible | Both use standard Ed25519 (64-byte signatures), but verify: (1) CryptoKit's `signature(for:)` takes raw bytes, not a digest -- do not pre-hash; (2) ring's `Ed25519KeyPair::sign()` also signs raw bytes; (3) public key format is 32 bytes raw in both. Test with shared test vectors from the Ed25519 RFC |
| GRDB observation in SwiftUI | Putting `ValueObservation` in the ViewModel's `init`, causing observation to start before the view is visible and continue after it disappears | Start observations in `.task` modifier (auto-cancelled when view disappears). Use `@Observable` view models that expose published results from GRDB, not raw observation tokens |
| Server timestamps vs. local clock | Using `Date()` for message ordering | Always use `server_timestamp` from the `ainm.message.send` response for canonical ordering. UUIDv7 IDs are time-ordered but use the device clock (which may be wrong). Show messages in UUIDv7 order optimistically, re-sort by `server_timestamp` when the response arrives |
| Interactive blocks + ActionBlocks | Sending the action response as a mutation of the original message | ActionBlocks are NEW messages in the thread (per interactive.md). They reference the original via `ref_message_id` and `ref_action_id`. The original interactive block is immutable. Build the action flow as: user taps -> create new ActionBlock message -> sign -> send -> original block transitions to disabled state locally |
| Content block `version` handling | Only implementing `1.0` and showing blank for anything else | Check MAJOR version. Same MAJOR = render normally, ignore unknown optional fields. Higher MAJOR = render `fallback` string. This is required by content.md Section 3 |

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Parsing Markdown in every message cell during scroll | Frame drops, janky scroll, high CPU in Instruments | Pre-parse Markdown to `AttributedString` when the message is received/fetched, cache in the model. Use background parsing for batches | 50+ messages with Markdown formatting visible simultaneously during fast scroll |
| Loading full image data for ImageBlocks in message list | Memory spikes, scroll hitches, jetsam termination | Use thumbnail URLs in the list view. Load full resolution only when tapped. Use `AsyncImage` with `.resizable()` and explicit `.frame()`. Implement disk cache | 10+ images visible in a single conversation scroll viewport |
| Unbounded streaming delta buffer | Memory grows linearly during long agent responses | Cap buffer at 100KB per block. For collapsed messages, stream large content to disk. Set a maximum stream duration timeout (e.g., 5 minutes) | Agent generates 50KB+ response (common for code generation tasks) |
| Re-signing messages on retry after reconnection | CPU spike re-signing 20+ queued messages, blocking reconnection flow | Cache the signed envelope bytes alongside the message in the outbound queue. Reuse if content unchanged. Only re-sign if the timestamp was updated | Batch retry of 20+ failed messages after reconnection on an older device (iPhone 11) |
| Fetching full JSONB content for conversation list preview | Slow conversation list load, 100MB+ memory for 50 conversations with large messages | Store `preview_text` (first 100 chars of first text block) as a separate indexed column. Query only `id, thread_id, from, timestamp, preview_text` for list | 50+ conversations where messages contain code blocks, forms, or long text |
| Rendering all content blocks eagerly in each message cell | Complex cells with 3+ blocks cause layout passes >16ms | Use lazy rendering: show only the first 2 blocks initially, "Show more" button for messages with 3+. Or use measured pre-layout to only render visible blocks | Messages with 5+ content blocks (common for agent responses with text + code + explanation) |

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Logging message content or signatures via `print()` or `os_log` | Sensitive content visible in device console, sysdiagnose, and Xcode device logs accessible by anyone with physical access | Use `OSLog` with `privacy: .private` for any message content or signature data. Use `#if DEBUG` guards for verbose protocol logging |
| Implementing `urlSession(_:didReceive:completionHandler:)` to bypass TLS validation for dev servers | Man-in-the-middle attack intercepts all messages and signing keys if accidentally shipped to production | Never disable TLS validation. For dev servers, install a local CA certificate on the test device. Use `Info.plist` App Transport Security exceptions only for specific test domains |
| Storing Ed25519 private key with `kSecAttrAccessibleWhenUnlocked` (the default) | Key inaccessible when device is locked, breaking background message signing; key accessible from unencrypted backups | Use `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly` explicitly |
| Not verifying Ed25519 signatures on incoming `ainm.message.deliver` notifications | Accepting spoofed or tampered messages from a compromised server or network attacker | Verify signature on every incoming message before displaying. Fetch sender's public key via identity resolution. Show a warning badge on messages that fail verification rather than silently displaying them |
| Trusting the `source` field without verifying the message signature first | Prompt injection: attacker sends a message with `source: "system"` to trick the UI into displaying it as a system message | Verify signature first. Only then trust the `source` annotation for UI styling. Unsigned messages must not receive system-level visual treatment |
| Caching public keys indefinitely without TTL or rotation awareness | Using a revoked or rotated key to verify messages, accepting invalid signatures | Implement a 1-hour TTL cache for public keys. On verification failure, re-fetch the key before rejecting. Check `key_history` for messages signed with rotated keys (per identity.md Section 6.7) |
| Storing auth tokens or session secrets in `UserDefaults` | Accessible via backup extraction, no encryption at rest, no access control | Store all secrets in Keychain with appropriate accessibility attributes |

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| No optimistic message sending | User taps send, sees nothing for 500ms-2s until server confirms. Feels broken | Insert the message into the UI immediately with a "sending" indicator (spinner/clock icon). Update to "sent" (checkmark) on server confirmation. Show "failed" with retry button on error |
| Showing raw JSON-RPC errors to the user | User sees `{"code": -32010, "message": "Invalid signature"}` | Map AINM error codes to human-readable messages. `-32010` -> "This message could not be verified. Please try again." `-32001` -> "Connection interrupted. Reconnecting..." Log the technical error for debugging |
| Blocking the UI during reconnection | User opens the app after being offline, sees a spinner for 5+ seconds with no content | Show cached messages from GRDB immediately on app open. Display a subtle "Reconnecting..." banner at the top. Sync new messages in background. Remove banner when connected |
| Not rendering `fallback` for unknown block types | Messages from a newer server contain interactive blocks the client does not support -- they render as blank space | Always check block type and version major. Render `fallback` text (required on every block) when unrecognized. This is a protocol requirement, not a nice-to-have |
| No typing/thinking indicator before streaming starts | Agent appears to do nothing for 1-3 seconds between the user's message and the first `ainm.stream.start` event | Show a "thinking..." indicator immediately when a message is sent to a thread containing an agent. Replace with streaming content on first `ainm.stream.start` |
| Tool call events shown as raw JSON | User sees `{"tool": "web_search", "status": "started"}` in the message thread | Render tool calls as native UI cards using the AINM tool-visibility spec: "Searching the web..." with a loading animation, then the result. This is one of AINM's differentiating features -- make it shine |
| Form state lost when scrolling away | User fills out half of a FormBlock, scrolls up to read context, scrolls back -- form is blank | Persist form input state in the view model (not the SwiftUI view state). LazyVStack does not preserve nested view state. Store draft form values keyed by `action_id` |
| Approval blocks with no loading state | User taps "Approve" and nothing happens for 500ms while the action message is being signed and sent | Show an immediate state transition: button -> loading spinner -> confirmed state. Revert to original state if the send fails |

## "Looks Done But Isn't" Checklist

- [ ] **WebSocket reconnection:** Often missing WiFi-to-cellular transition handling -- verify by toggling airplane mode during active streaming, then verifying reconnection and message gap-fill within 10 seconds
- [ ] **Message signing:** Often only tested with ASCII content -- verify with emoji, CJK, RTL text, and messages containing all content block types (text + code + image + interactive)
- [ ] **Streaming collapse:** Often missing the step where accumulated deltas are assembled into a permanent signed message and persisted to GRDB -- verify by killing the app mid-stream and checking if the partial message is recoverable on next launch
- [ ] **Interactive block post-action state:** Often missing disabled/expired state handling -- verify that buttons and forms become non-interactive after the user submits an ActionBlock, and that expired approvals show their final state
- [ ] **Offline message queue:** Often missing retry-with-backoff and deduplication -- verify by sending 10 messages offline, reconnecting, and checking all 10 arrive exactly once in correct order
- [ ] **Pagination boundary:** Often missing the "load more" trigger when scrolling up -- verify by scrolling to the top of a 500+ message conversation and confirming older messages load without scroll position jumping
- [ ] **Background-to-foreground sync:** Often missing the gap-fill for messages received while the app was suspended -- verify by backgrounding the app for 5+ minutes, sending messages from another client, then foregrounding and checking they appear
- [ ] **Memory under load:** Often tested with 10 messages, not 1,000 -- verify by loading a conversation with 2,000+ messages and monitoring memory in Instruments (budget: <150MB)
- [ ] **Fallback rendering:** Often not tested because dev server and client are always in sync -- verify by sending a message with a made-up block type (e.g., `"chart"`) and confirming the fallback text renders
- [ ] **Key accessibility:** Often not tested with locked device -- verify that signing works after `beginBackgroundTask` when the device is locked (requires `AfterFirstUnlock` accessibility)
- [ ] **UUIDv7 ordering vs server timestamp ordering:** Often only one ordering is implemented -- verify that optimistic (UUIDv7) ordering is shown immediately, then corrected to server timestamp ordering when the server confirms

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| JCS canonicalization mismatch | MEDIUM | Write comprehensive cross-platform test suite with RFC 8785 vectors. Fix the Swift JCS implementation. Re-verify stored messages (signatures are immutable in messages). No data loss, but all messages sent with wrong canonicalization cannot be verified by other clients |
| WebSocket lifecycle bugs | MEDIUM | Refactor to explicit state machine pattern. Add integration tests using Network Link Conditioner for simulated poor conditions. ~1 week effort to retrofit state machine if the initial implementation was ad-hoc |
| Message list memory crash | HIGH | Requires architectural rewrite from "load all" to paginated windowed data source. Cannot be patched incrementally. Plan 1-2 weeks for list + observation + GRDB query redesign |
| Streaming UI freeze | MEDIUM | Insert throttling layer between WebSocket and UI. Requires refactoring the delta processing pipeline (~3-5 days) but does not change the data model |
| Lost Ed25519 keys | HIGH | If keys are lost and no recovery mechanism exists, the user's identity is permanently gone. Must design key backup/recovery before shipping. Cannot be retrofitted without identity migration. Recovery mechanism design: 1-2 weeks |
| GRDB observation overload | LOW | Refactor observations to narrower queries and background delivery. Can be done incrementally. ~2-3 days per view that needs optimization |
| Handshake contract violations | LOW | Add state machine enforcement. Queue outbound messages during handshake. ~1 day fix if caught early. Debugging intermittent failures from this bug can take much longer |

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| RFC 8785 JCS mismatch (P1) | Phase 1 (Foundation) | Cross-platform test: sign on iOS, verify on Rust; sign on Rust, verify on iOS. 100% pass on RFC 8785 official test vectors. Include Unicode, emoji, and multi-block messages |
| WebSocket lifecycle (P2) | Phase 1 (Foundation) | Integration test: connect, exchange messages, toggle airplane mode, verify reconnection + handshake replay + gap-fill within 10 seconds |
| Message list performance (P3) | Phase 2 (Core UI) | Instruments profiling: 60fps scroll through 2,000-message conversation. Memory <150MB. No blank frames during fast scroll |
| Streaming UI starvation (P4) | Phase 3 (Streaming) | Stress test: 3 parallel blocks streaming at 100 tokens/sec. Taps register within 100ms. No main thread hangs in Instruments |
| Ed25519 key storage (P5) | Phase 1 (Foundation) | Test matrix: create key, verify signing works after device lock, verify key does NOT sync to iCloud, verify key is gone after app delete + reinstall |
| GRDB observation overload (P6) | Phase 2 (Core UI) | Test: write 50 messages to conversation B while viewing conversation A. Zero visible jank. CPU spike <5% on main thread |
| ainm.initialize handshake (P7) | Phase 1 (Foundation) | Test: send message before handshake -> message queued. Reconnect after server restart -> handshake replays, queued messages flush. Capability mismatch -> user-visible error |
| Interactive block action flow | Phase 4 (Interactive) | Test: submit form -> button disabled immediately -> ActionBlock appears in thread -> original block shows submitted state. Form state survives scroll-away-and-back |
| Streaming collapse + persistence | Phase 3 (Streaming) | Test: complete stream -> collapsed message in GRDB with valid signature. Cancel stream -> partial content handled per spec. Kill app mid-stream -> partial content recoverable |

## Sources

### Apple Documentation
- [Storing CryptoKit Keys in the Keychain](https://developer.apple.com/documentation/cryptokit/storing-cryptokit-keys-in-the-keychain) -- Ed25519 keys stored as generic passwords, not SecKey
- [Curve25519.Signing.PrivateKey](https://developer.apple.com/documentation/cryptokit/curve25519/signing/privatekey) -- API for Ed25519 signing in CryptoKit
- [URLSessionWebSocketTask](https://developer.apple.com/documentation/foundation/urlsessionwebsockettask) -- Single-shot receive, lifecycle management
- [NWPathMonitor](https://developer.apple.com/documentation/network/nwpathmonitor) -- Network transition detection
- [Creating Performant Scrollable Stacks](https://developer.apple.com/documentation/swiftui/creating-performant-scrollable-stacks) -- Apple's guidance on LazyVStack performance

### SwiftUI Message List Performance
- [List or LazyVStack: Choosing the Right Container](https://fatbobman.com/en/posts/list-or-lazyvstack/) -- LazyVStack 52.3s vs List 5.53s for scroll-to-bottom; LazyVStack 78 hangs vs List 4.6
- [SwiftUI List Performance: 10,000+ Items](https://medium.com/@chandra.welim/swiftui-list-performance-smooth-scrolling-for-10-000-items-c64116dc276f) -- Memory and scroll performance at scale
- [Stream: How Our iOS Team Built the SwiftUI SDK Message List](https://getstream.io/blog/build-message-list-swiftui/) -- LazyVStack retains previous states; crash at ~5K messages
- [Building Inverted Scroll for Messaging App](https://www.swiftwithvincent.com/blog/building-the-inverted-scroll-of-a-messaging-app) -- 180-degree rotation pattern for chat scroll

### Streaming and Real-time Rendering
- [From Stream to Screen: Handling GenAI Rich Responses in SwiftUI](https://medium.com/safe-engineering/from-stream-to-screen-handling-genai-rich-responses-in-swiftui-da138acfaa05) -- Background processing and throttling for streaming text
- [Real-time with WebSockets and Swift Concurrency](https://medium.com/@thomsmed/real-time-with-websockets-and-swift-concurrency-8b44a8808d0d) -- AsyncThrowingStream wrapping URLSessionWebSocketTask
- [Iterating over WebSocket Messages with async/await](https://www.donnywals.com/iterating-over-web-socket-messages-with-async-await-in-swift/) -- Single-shot receive pitfall and AsyncStream solution

### GRDB Performance
- [GRDB Performance Wiki](https://github.com/groue/GRDB.swift/wiki/Performance) -- insert() vs save(), batch operations
- [Large Insert Performance Discussion](https://github.com/groue/GRDB.swift/issues/1004) -- Encoder initialization overhead, query cache costs

### JSON Canonicalization
- [RFC 8785: JSON Canonicalization Scheme](https://www.rfc-editor.org/rfc/rfc8785) -- The specification; UTF-16 sorting, ECMAScript float formatting
- [JCS Reference Implementation + Test Vectors](https://github.com/cyberphone/json-canonicalization) -- Cross-platform test vectors for verification
- [serde_json_canonicalizer (Rust)](https://docs.rs/serde_json_canonicalizer/latest/serde_json_canonicalizer/) -- The Rust implementation the AINM server uses

### WebSocket Lifecycle on iOS
- [WebSockets and Swift: Client-side Engineering Challenges (Ably)](https://ably.com/topic/websockets-swift) -- Reconnection, network drops, interface switches
- [Apple Developer Forums: WebSocket Background Execution](https://developer.apple.com/forums/thread/66157) -- iOS does not allow background WebSocket; use APNs

---
*iOS Reference Client pitfalls research for: AINM v3.3*
*Researched: 2026-03-29*
