# Federation Pitfalls Research

**Domain:** Adding multi-server federation to an existing single-server AI-native messaging protocol (AINM v3.2)
**Researched:** 2026-03-28
**Confidence:** HIGH (grounded in extensive real-world post-mortems from Matrix, XMPP, ActivityPub, and email)

---

## Critical Pitfalls

These mistakes cause protocol rewrites, security breaches, or multi-year engineering sinkholes.

---

### Pitfall 1: Origin-Server Authority Becomes Split-Brain Under Network Partitions

**What goes wrong:**
AINM v3.2 specifies "origin-server authority for thread state" -- one server is the authoritative source for each thread. This seems simple until the origin server goes offline. Remote participants keep sending messages that queue on their local servers. When the origin server returns, two divergent histories exist: messages the origin server knew about, and messages that accumulated on remote servers. If the origin server simply appends the queued messages, ordering may violate causal dependencies. If it rejects them, users lose messages.

The problem compounds in multi-party threads spanning 3+ servers. Server B sends a reply to a message from Server C, but Server A (the origin) was offline and has no record of Server C's message yet. When Server A comes back, it receives Server B's reply before Server C's original -- the causal chain is broken.

**Why it happens:**
"Origin-server authority" is a simplification that works perfectly when the origin is always available. In a federated network, the origin WILL be unavailable -- hardware failures, maintenance windows, DNS outages, DDoS attacks. The design was correct for a single-server protocol but doesn't account for the fundamental reality of distributed systems: partitions are not exceptional, they are normal.

Matrix solved this by having NO authoritative server -- every server that participates in a room has equal authority, which is why state resolution is so complex. AINM chose the opposite extreme (single authority) which avoids state resolution complexity but creates a single point of failure per thread.

**How to avoid:**
- Define explicit behavior for "origin server unavailable" in the spec BEFORE implementing federation. The spec must answer: Can remote servers accept messages for a thread when the origin is down? If yes, how are they ordered when the origin returns? If no, what does the user see?
- Implement "queued relay with causal ordering" -- remote servers accept and locally order messages using UUIDv7 timestamps and causal references (each message references the ID of the message it's replying to). When the origin returns, it merges queued messages respecting causal order.
- Set a partition tolerance window (e.g., 7 days). If the origin server is down for longer than this, remote servers can either promote a new origin (protocol-defined process) or mark the thread as degraded.
- Build a "partition simulator" test harness that runs two federated servers, partitions them, generates concurrent activity on both sides, heals the partition, and verifies message ordering and completeness.

**Warning signs:**
- Messages appearing out of order after a server reconnects
- "Message not found" errors when clicking a reply that references a message not yet synced
- Thread participant lists diverging between servers
- Integration tests that only pass when all servers are online simultaneously

**Phase to address:**
Phase 1 (Relay Core). The partition behavior must be specified and tested from the very first relay implementation. Do not ship federation without a partition test suite.

---

### Pitfall 2: mTLS Certificate Lifecycle Becomes an Operational Nightmare

**What goes wrong:**
AINM specifies mutual TLS (mTLS) for server-to-server relay, with TLS certificate fingerprints published at `/.well-known/ainm/server`. This means every federated server must: (1) obtain a TLS certificate, (2) publish its fingerprint, (3) verify remote server fingerprints against their well-known endpoints, and (4) rotate certificates before expiry without breaking federation.

When a server rotates its TLS certificate, its fingerprint changes. Every remote server that cached the old fingerprint will reject connections until they re-fetch the server identity. If the TTL on the cached server identity is 1 hour (as recommended in the transport spec), there is up to a 1-hour window where federation is broken after every certificate rotation. During this window, messages queue up, and if the queue is bounded, messages may be lost.

Worse: Let's Encrypt certificates expire every 90 days. If a server operator forgets to renew (or auto-renewal fails), federation silently breaks. The server still works for local users but is invisible to the federated network. No error is visible to the local operator unless they actively monitor federation health.

**Why it happens:**
mTLS is the right security choice for S2S authentication, but it shifts significant operational complexity onto every server operator. In a centralized system, one team manages certificates. In federation, every independent operator must do it correctly. Email's history proves this fails at scale: misconfigured MX records, expired TLS certificates, and broken DKIM selectors are the #1 cause of email delivery failures between organizations.

The AINM spec defines how fingerprints are published and verified but does not define certificate rotation procedures, overlap periods, or health monitoring.

**How to avoid:**
- Define a certificate rotation protocol: servers MUST publish the new fingerprint at least 24 hours before the old certificate expires. The server identity endpoint SHOULD return both current and upcoming fingerprints during the rotation window.
- Add a `tls_fingerprints` array (plural) to the server identity response, replacing the single `tls_fingerprint` field. Support multiple valid fingerprints during rotation, with each having a `valid_from` and `valid_until` timestamp. This mirrors AINM's existing key rotation model for participant Ed25519 keys.
- Implement federation health checks: each server periodically tests its own federation connectivity by resolving its own server identity from external DNS and verifying the fingerprint matches. Log warnings when certificates are within 14 days of expiry.
- Provide a `ainm-fed-check` CLI tool in the reference server that validates: DNS resolution, TLS certificate validity, fingerprint match with well-known endpoint, relay endpoint reachability. Operators run this after any certificate change.
- Default to short cache TTLs for server identity (5 minutes during rotation, 1 hour otherwise). Include a `rotation_in_progress` boolean in the server identity response to signal remote servers to reduce cache TTL.

**Warning signs:**
- Federation working between some server pairs but not others
- Relay requests failing with TLS handshake errors after certificate renewal
- Server operators not knowing their federation is broken (no local symptoms)
- Message delivery delays that correlate with certificate renewal schedules

**Phase to address:**
Phase 2 (Authentication & Security). Certificate lifecycle must be designed before the first mTLS connection is established in production.

---

### Pitfall 3: Server Identity Spoofing via DNS Poisoning or Well-Known Endpoint Compromise

**What goes wrong:**
AINM's federation discovery relies on DNS resolution to find `/.well-known/ainm/server` endpoints. An attacker who poisons DNS (or compromises the well-known endpoint hosting) can serve a malicious server identity with their own public key and TLS fingerprint. The attacking server then impersonates the legitimate server, receiving and sending messages that appear to come from the legitimate domain.

This is not theoretical. XMPP's dialback protocol suffered exactly this: in 2012, multiple XMPP server implementations were found vulnerable to attacks where malicious servers sent verification responses for domains that never initiated the exchange. Servers accepted these unsolicited responses because they didn't track request-response state, enabling arbitrary domain spoofing.

For AINM, the attack surface is: (1) DNS poisoning redirects `example.com` to attacker IP, (2) attacker serves `/.well-known/ainm/server` with their own keys, (3) attacker receives relay messages destined for `example.com` users, (4) attacker can read messages and forge new ones signed with the spoofed server key.

**Why it happens:**
The well-known endpoint pattern (used by Matrix, ACME/Let's Encrypt, ActivityPub) is convenient but relies entirely on DNS + TLS for trust. If either is compromised, the server identity is compromised. AINM adds Ed25519 server keys on top of TLS, but the server key is itself discovered via the potentially-compromised well-known endpoint -- it's circular trust.

**How to avoid:**
- Implement server key pinning: once a server has successfully federated with a remote server, it pins the remote server's Ed25519 public key. Subsequent connections verify the key hasn't changed. If it has, federation is paused and an alert is raised (TOFU -- Trust On First Use, same as SSH).
- Add key continuity verification: when a server identity's public key changes, the old key MUST sign a "key rotation" attestation (identical to AINM's participant key rotation model). Unsigned key changes are treated as potential compromise.
- Validate the `from_server` field in relay requests against the mTLS client certificate AND the server identity endpoint. All three must be consistent: the mTLS cert, the well-known public key, and the `from_server` claim.
- Consider DNSSEC validation for server discovery. While not universally deployed, DNSSEC prevents DNS poisoning where available, and the reference server should validate DNSSEC when the remote domain supports it.
- Rate-limit and log all server identity changes. A server whose key or fingerprint changes more than once per month should trigger investigation.

**Warning signs:**
- Server identity responses with public keys that don't match previously seen values
- mTLS handshakes succeeding but server signature verification failing (or vice versa)
- A remote server's well-known endpoint returning different results from different network vantage points
- Relay messages arriving from a `from_server` that doesn't match the connecting IP's reverse DNS

**Phase to address:**
Phase 2 (Authentication & Security). Key pinning must be implemented alongside initial federation. Without it, every federation connection is vulnerable to a one-time DNS attack.

---

### Pitfall 4: Message Signature Verification Becomes a Performance Bottleneck at Federation Scale

**What goes wrong:**
AINM requires three levels of validation on every relay request: (1) mTLS certificate verification, (2) server signature verification on the relay payload, (3) individual Ed25519 signature verification on each message against the sender's public key (resolved via identity resolution).

Step 3 is the killer. For each message in a relay batch, the receiving server must: resolve the sender's identity (HTTP GET to a remote server), fetch the public key, verify the signature. If a relay batch contains 50 messages from 30 different senders on 15 different servers, that's potentially 30 identity resolution HTTP requests before the batch can be processed. If identity resolution takes 200ms average (DNS lookup + TLS handshake + HTTP request), that's 6 seconds of blocking I/O per relay batch.

Matrix learned this the hard way: the `get_auth_chain_difference` algorithm -- which serves a similar purpose of validating event authorization chains -- "can often take hundreds of seconds of database time" and was "a large component of database load from state resolution."

**Why it happens:**
Single-server AINM doesn't have this problem because all identities are local. Federation multiplies the identity resolution cost by the number of unique remote senders, and this cost is paid on the hot path of message delivery.

**How to avoid:**
- Aggressive identity caching: cache resolved identity records for the full Cache-Control duration (recommended 1 hour). The receiving server should maintain a local identity cache that handles 99%+ of lookups without remote HTTP calls.
- Pre-warm identity cache during federation handshake: when two servers begin federating, they should exchange the identities of their active users in bulk rather than waiting for per-message lookups.
- Batch identity resolution: when a relay batch arrives with multiple unknown senders from the same server, resolve them in a single batched HTTP request rather than individual GETs. Define a batch identity resolution endpoint (e.g., `POST /.well-known/ainm/resolve-batch`).
- Async signature verification: verify signatures in a background task pool, not on the relay request handler. Accept the relay payload with a 202 (Accepted), queue signature verification, and notify the sending server of rejections asynchronously. This prevents relay requests from timing out during verification.
- Set a maximum batch size for relay requests (e.g., 100 messages). Reject batches exceeding this with a clear error, forcing the sender to split into multiple requests.

**Warning signs:**
- Relay request latency increasing as the number of federated servers grows
- Identity resolution HTTP requests dominating server CPU/network
- Message delivery delays that correlate with federation traffic volume
- Database connection pool exhaustion during signature verification

**Phase to address:**
Phase 1 (Relay Core) for the caching strategy. Phase 3 (Performance) for batch resolution and async verification.

---

### Pitfall 5: Replay Attacks Across Federation Boundaries Bypass Single-Server Deduplication

**What goes wrong:**
AINM's deduplication (security.md Section 7) relies on tracking message IDs and rejecting duplicates. This works within a single server. In federation, an attacker captures a valid signed relay payload (the server signature is intact, the individual message signatures are intact) and re-transmits it to the target server. The target server sees a valid relay from a valid server with valid messages -- but it's a replay.

The attack is powerful because AINM messages are immutable and self-contained: once signed, a message is valid forever (the signature doesn't expire). The deduplication window is "at least 24 hours" -- after 24 hours, the same message ID may no longer be in the deduplication set and could be accepted again.

ActivityPub suffers from exactly this: "HTTP Signatures are ephemeral -- they only authenticate HTTP requests, not long-lived data." But AINM's per-message Ed25519 signatures are permanent, making signed messages valid replay material indefinitely.

**Why it happens:**
The current deduplication spec was designed for client-to-server scenarios where the server is the single source of truth. In federation, messages arrive from external servers where the receiving server has no prior knowledge. The deduplication window is a sliding window that eventually evicts old message IDs, opening a window for replay.

**How to avoid:**
- Persist message IDs permanently (or for the message retention period) in the database, not just in a sliding window. For relay messages, the receiving server MUST check the persistent message store, not just the in-memory deduplication set.
- Add a `nonce` or `relay_id` to relay payloads -- a unique identifier for each relay request (distinct from the message IDs it contains). Receiving servers track relay_ids to detect replayed relay payloads even if the individual message IDs are still in the dedup set.
- Enforce timestamp bounds on relay payloads: add a `relay_timestamp` to the relay params (covered by the server signature). Receiving servers reject relay payloads with timestamps older than 5 minutes. This means replaying a captured payload only works within a 5-minute window.
- Consider adding a challenge-response mechanism to the relay protocol: before accepting a relay, the receiving server sends a nonce to the sender, and the sender includes this nonce in the server signature. This prevents replay entirely but adds a round trip.
- Log and alert on relay payloads where ALL message IDs are already in the dedup set -- this is a strong indicator of replay.

**Warning signs:**
- Duplicate messages appearing after the deduplication window expires
- Relay payloads arriving with timestamps significantly older than the current time
- Users seeing the same message delivered twice, hours or days apart
- Server logs showing identical relay payloads from the same source within a short window

**Phase to address:**
Phase 2 (Authentication & Security). Replay protection must be in the relay protocol before federation is considered production-ready.

---

### Pitfall 6: Selective Backfill Leaks Thread History to Unauthorized Servers

**What goes wrong:**
AINM v3.2 targets "selective backfill for thread history across servers." When a new participant from Server B joins a thread owned by Server A, Server A must send thread history to Server B. If backfill sends the full thread history, Server B gets messages that were exchanged before the new participant joined -- potentially including sensitive content that was shared under the assumption that only the original participants would see it.

This is not just a privacy concern -- it's a security concern. In a multi-server thread, Server B could be malicious (or compromised), and backfill gives it access to historical messages it was never authorized to receive. The thread participants never consented to Server B seeing messages from before the join.

Matrix's backfill has this exact problem: "Excessively large rooms may cause performance problems for servers implementing state backfill APIs" and there are no fine-grained access controls on what history is shared during backfill.

**Why it happens:**
In single-server AINM, when a user joins a thread, the server controls exactly what history they see (based on thread settings). In federation, the "thread history" must be serialized and transmitted to another server that you don't control. Once the messages leave the origin server, there is no way to enforce access controls on the remote server.

**How to avoid:**
- Define backfill visibility levels in the spec: `none` (no history for new joiners from other servers), `summary` (a server-generated summary of prior discussion, no actual messages), `since_invite` (only messages after the participant was invited/mentioned), `full` (complete history). Thread creators choose the level at thread creation time.
- Never send messages to a server that doesn't have a participant in the thread. The relay endpoint should reject messages for threads where the target server has no participants.
- Implement "backfill authorization": when Server B requests backfill, Server A verifies that Server B has a legitimate participant in the thread and only sends messages from the point of that participant's join timestamp onward.
- Consider "backfill digests": instead of sending raw messages, send summaries or metadata (sender, timestamp, content type) that let the new participant understand thread context without exposing the actual content. The participant can request specific messages if needed.
- Make backfill a pull operation, not a push. Server B requests history; Server A decides what to share based on the requesting participant's join time and the thread's backfill policy. Do not automatically push full history on join.

**Warning signs:**
- New participants seeing messages from before they joined, without the thread owner's intent
- Server operators requesting "how do I prevent other servers from seeing old messages"
- Privacy-sensitive threads being created without backfill considerations
- Backfill requests that are disproportionately large relative to the participant's legitimate need

**Phase to address:**
Phase 2 (Backfill). Backfill authorization must be designed before the first backfill message is sent. This cannot be retrofitted without breaking existing behavior.

---

## Technical Debt Patterns

Shortcuts that seem reasonable during initial federation implementation but create long-term problems.

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Skip mTLS, use server signatures only | Faster dev setup, no PKI infrastructure needed | No transport-layer authentication; any server can claim any identity by serving a fake well-known endpoint | Never in production. Acceptable only for local docker-compose testing |
| Cache server identities indefinitely | Eliminates repeated HTTP lookups | Stale fingerprints after certificate rotation, stale keys after server key rotation; federation silently breaks | Never. Always respect Cache-Control headers |
| Single-threaded relay processing | Simpler implementation, no concurrency bugs | Relay throughput bottleneck; one slow identity resolution blocks all relay processing | MVP only. Must be concurrent before any load testing |
| Skip relay `server_signature` verification | Faster message processing, one fewer crypto operation | Any server on the network can inject messages claiming to be from any other server; the mTLS alone doesn't prevent a compromised server from lying about `from_server` | Never |
| Unbounded offline relay queues | No messages lost during server downtime | Disk exhaustion; a malicious server sending millions of messages to an offline target fills the relay queue | Early development only. Must add queue size limits and per-source quotas before federation testing |
| Trust all federated servers equally | No trust infrastructure needed | One malicious server can spam/abuse the entire network with no consequences. Email's open relay history proves this fails catastrophically | Never. Graduated trust from first federation implementation |

## Integration Gotchas

Mistakes specific to integrating federation with AINM's existing single-server identity and signing model.

| Integration Point | Common Mistake | Correct Approach |
|-------------------|----------------|------------------|
| Identity resolution across servers | Calling `/.well-known/ainm/{name}` on the remote server for every message in a relay batch | Cache identity records locally. Batch-resolve unknown identities. Pre-warm cache during federation handshake |
| Ed25519 signature verification with key rotation | Verifying only against the sender's current key, failing messages signed with a recently-rotated key | Follow identity.md Section 6.7: try current key, then check key_history for a key valid at the message's timestamp. Handle the 24-hour overlap period |
| Server Ed25519 key vs. participant Ed25519 key | Confusing the server-level signing key (for relay payloads) with participant-level signing keys (for individual messages) | Server keys sign relay metadata. Participant keys sign message content. They are independent key pairs with independent rotation lifecycles |
| Thread ownership across federation | Assuming the origin server always knows about all thread participants | Remote servers may have participants that joined via other remote servers. The origin server must track all participants across all servers, not just directly-connected ones |
| UUIDv7 message ordering in federation | Relying on UUIDv7 timestamp component for global ordering when clocks are not synchronized across servers | UUIDv7 provides per-server ordering. Cross-server ordering requires the `server_timestamp` assigned by the origin server. Use causal references (reply-to message IDs) for causal ordering |

## Performance Traps

Patterns that work in single-server or small-federation scenarios but fail as the federation grows.

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Synchronous identity resolution on relay hot path | Relay request latency increases linearly with number of unique senders per batch | Async resolution, local cache, batch resolution endpoint | > 10 unique remote senders per relay batch |
| Unbounded relay batch size | Single relay request takes > 30 seconds to process; connection timeouts | Cap batch size at 100 messages; reject larger batches with error | > 200 messages per relay request |
| Per-message relay (no batching) | Network overhead from TCP+TLS handshake per message; connection storms during high traffic | Batch messages per destination server; hold for 50-100ms to collect batch | > 50 messages/second to a single remote server |
| Full thread state in every relay payload | Relay payload size grows with thread history; bandwidth saturation | Send only new messages in relay; backfill only on explicit request | Threads with > 1000 messages |
| Typing/presence notifications via federation | A single user typing in a room with 800 participating homeservers triggers 800 HTTP requests | Rate-limit EDU (Ephemeral Data Unit) federation; only federate presence to servers with active participants; cap at N remote servers | > 50 participating remote servers per thread |
| Naive server discovery on every relay | DNS lookup + HTTPS + JSON parse for `/.well-known/ainm/server` before every relay attempt | Cache server identity with TTL; background refresh; circuit breaker for unreachable servers | > 100 relay requests per minute to the same server |

## Security Mistakes

Federation-specific security issues beyond AINM's existing single-server security model.

| Mistake | Risk | Prevention |
|---------|------|------------|
| Trusting relay payload `from_server` without verifying mTLS certificate identity | Any server can claim to be any other server by spoofing the `from_server` field | Cross-validate `from_server` against the mTLS client certificate Subject/SAN AND the server identity endpoint |
| Not validating that relayed messages are actually FROM users on the sending server | Server A relays a message claiming to be from `alice@serverB.com` -- Server A is laundering messages through its relay | Verify that each message's `from` address has a provider matching the relay's `from_server`. Reject messages where the sender's provider doesn't match the relaying server |
| Accepting relay payloads with no rate limiting | A single malicious federated server sends millions of messages, overwhelming the receiving server (federation-level DDoS) | Per-source rate limits on relay endpoint. Graduated limits based on server trust/reputation. Hard cap at N relay requests per minute per source |
| Serving `/.well-known/ainm/server` over HTTP (not HTTPS) | Server identity response can be tampered with in transit, enabling MITM of all federation traffic | Enforce HTTPS-only for all well-known endpoints. Reject HTTP responses. Pin certificate on first successful connection (TOFU) |
| No relay payload expiry | Captured relay payloads can be replayed indefinitely | Add `relay_timestamp` to relay params (covered by server signature). Reject payloads older than 5 minutes |
| Allowing cross-server message forgery through backfill | During backfill, remote server sends fabricated "historical" messages that were never actually sent | Verify every backfilled message's individual Ed25519 signature against the sender's public key at the message's timestamp. Reject any message with an invalid signature, even during backfill |
| Server key rotation without attestation | A compromised well-known endpoint can silently replace the server's Ed25519 key, enabling impersonation | Require the old key to sign a rotation attestation for the new key (same model as participant key rotation in identity.md) |

## "Looks Done But Isn't" Checklist

Things that appear complete in a federation implementation but are missing critical pieces.

- [ ] **Relay endpoint works:** But does it verify mTLS fingerprint against the well-known server identity? Not just "TLS handshake succeeded" but "certificate fingerprint matches published value."
- [ ] **Messages relay between two servers:** But do they relay correctly when the origin server is down? Test the partition case, not just the happy path.
- [ ] **Signature verification passes:** But does it work for messages signed with a rotated key? Test with sender keys that rotated after the message was signed but before it was relayed.
- [ ] **Server identity endpoint responds:** But does it include the upcoming TLS fingerprint during certificate rotation? Test the rotation overlap period.
- [ ] **Offline queue stores messages:** But is it bounded? What happens when it fills up? Are per-source quotas enforced? Does it survive server restart (persistence, not just in-memory)?
- [ ] **Backfill sends thread history:** But does it respect the requesting participant's join timestamp? Does it enforce the thread's backfill policy? Does it verify the requester actually has a participant in the thread?
- [ ] **Rate limiting works per-client:** But is it also applied per federation source? A federated server is a different threat model than a single client -- it can send much higher volumes legitimately.
- [ ] **Federation health monitoring exists:** But does it alert when federation is silently broken? (e.g., certificate expired, DNS pointing wrong, server identity stale). Silent federation failures are the most dangerous because no user reports them until they try to message someone on the broken server.
- [ ] **Docker-compose demo works:** But does it test network partitions, certificate rotation, key rotation, and replay attacks? Or only the happy path of two healthy servers exchanging messages?

## Recovery Strategies

When federation pitfalls occur despite prevention, how to recover.

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Split-brain thread divergence after partition | MEDIUM | Origin server merges queued messages using causal ordering. If causality is ambiguous, insert a "thread was partitioned" system message and append all messages chronologically. Notify participants that message ordering may be approximate for the partition period |
| mTLS certificate expired, federation broken | LOW | Renew certificate, update well-known endpoint with new fingerprint. Wait for remote servers to refresh cache (up to TTL). Force-refresh by contacting known federation partners out-of-band |
| Server identity spoofed via DNS | HIGH | Rotate server Ed25519 key immediately. Notify all federation partners out-of-band. Audit messages received during the spoofing period. Consider key pinning registry for critical federation partners |
| Relay queue overflow | LOW | Implement queue compaction (deduplicate, drop expired messages). Increase queue capacity. Add per-source quotas to prevent recurrence. Alert sending servers of rejected messages |
| History leaked via permissive backfill | HIGH (data already exposed) | Cannot un-send leaked messages. Notify affected participants. Tighten backfill policy for future joins. Consider message expiry/deletion federation protocol for damage limitation |
| Replay attack delivered duplicate messages | LOW | Remove duplicates from thread. Extend deduplication window. Add relay_timestamp validation. Investigate source of captured relay payload |

## Pitfall-to-Phase Mapping

How roadmap phases should address these federation pitfalls.

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Split-brain under partition (P1) | Phase 1: Relay Core | Network partition test suite passes with correct message ordering after heal |
| mTLS certificate lifecycle (P2) | Phase 2: Authentication | Certificate rotation test: rotate cert, verify federation resumes within 5 minutes |
| DNS/identity spoofing (P3) | Phase 2: Authentication | Key pinning test: change server key without attestation, verify federation rejects connection |
| Signature verification bottleneck (P4) | Phase 1: Relay Core (caching); Phase 3: Performance (batch resolution) | Relay batch with 50 messages from 30 senders processes in < 2 seconds |
| Cross-federation replay (P5) | Phase 2: Authentication | Replay captured relay payload after 5 minutes, verify rejection |
| Backfill history leakage (P6) | Phase 2: Backfill | Join thread with `since_invite` policy, verify no messages before join timestamp are received |
| Typing/presence notification storms (Performance Trap) | Phase 3: Performance | Thread with 100+ remote servers, verify typing notifications are rate-limited to < 10 requests/second |
| Federation spam/abuse (from original PITFALLS.md P8) | Phase 3: Trust & Moderation | New server connects, verify rate limits applied; send 1000 messages, verify throttling activates |
| Server message laundering (Security) | Phase 1: Relay Core | Relay message with `from` address on different server than `from_server`, verify rejection |

## Sources

### Matrix Federation Lessons
- [Matrix State Resolution v2 Explained](https://matrix.org/docs/older/stateres-v2/) -- Complexity of distributed state merging; why naive approaches fail
- [Project Hydra: Improving State Resolution](https://matrix.org/blog/2025/08/project-hydra-improving-state-res/) -- CVE-2025-49090; state resets from crafted event sequences after 7+ years of v2
- [Matrix Federation Security Pre-disclosure (July 2025)](https://matrix.org/blog/2025/07/security-predisclosure/) -- Coordinated security release across all Matrix server implementations
- [Improving Federation Security (Matrix Conference 2025)](https://media.ccc.de/v/matrix-conf-2025-74569-improving-the-security-of-the-federation-protocol) -- Ongoing federation security improvements
- [Auth Chain Difference Algorithm (Synapse)](https://github.com/matrix-org/synapse/blob/develop/docs/auth_chain_difference_algorithm.md) -- Naive implementation takes hundreds of seconds of DB time
- [get_auth_chain_difference Performance Issue](https://github.com/matrix-org/synapse/issues/8622) -- Real-world performance bottleneck in federation
- [Joining Large Rooms is Very Slow](https://github.com/matrix-org/synapse/issues/1211) -- Federation join performance degrades with room state size
- [Typing Notifications Overload with 800 Homeservers](https://matrix.org/blog/2025/01/27/this-week-in-matrix-2025-01-27/) -- EDU federation scaling problem
- [Synapse Refusing Large Federated Rooms](https://github.com/matrix-org/synapse/issues/2255) -- Performance impact of large federated rooms

### XMPP Federation Vulnerabilities
- [XMPP Server Dialback Vulnerability](https://xmpp.org/community/security-notices/vulnerability-in-xmpp-server-dialback-implementations/) -- Unsolicited verification responses enabling domain spoofing
- [XEP-0220: Server Dialback](https://xmpp.org/extensions/xep-0220.html) -- The protocol that was exploited; lessons on request-response state tracking
- [Prosody S2S Documentation](https://prosody.im/doc/s2s) -- Server-to-server federation implementation details and issues

### ActivityPub Federation Issues
- [ActivityPub and HTTP Signatures](https://swicg.github.io/activitypub-http-signature/) -- Ephemeral signatures, version fragmentation, deadlock in authorized fetch
- [HTTP Signatures Continue to Confound (SocialHub)](https://socialhub.activitypub.rocks/t/http-signatures-and-mastodon-continue-to-confound/3801/1) -- Implementation inconsistencies between Mastodon, Misskey, GoToSocial
- [Misskey Federation Flakiness (GoToSocial)](https://github.com/superseriousbusiness/gotosocial/issues/540) -- Real-world interoperability issues
- [The Sisyphean Effort of ActivityPub Migration](https://voidfox.com/blog/the_sisyphean_effort_of_activitypub_migration/) -- No two implementations behave identically
- [Fedify: Building ActivityPub Servers Without the Pain (FOSDEM 2026)](https://fosdem.org/2026/schedule/event/KSEUZT-fedify/) -- Community response to implementation complexity

### Email Federation Lessons
- [Open Mail Relay History](https://en.wikipedia.org/wiki/Open_mail_relay) -- How SMTP's trust-everyone model was exploited for spam
- [SMTP Open Relay Vulnerabilities](https://www.duocircle.com/email-security/smtp-open-relay-vulnerabilities-how-to-prevent-security-breaches) -- Security breach patterns from open relay abuse
- [SPF, DKIM, DMARC Common Setup Mistakes](https://www.infraforge.ai/blog/spf-dkim-dmarc-common-setup-mistakes) -- Email authentication pitfalls directly relevant to server identity
- [Spamhaus: Return of Open Relays](https://www.spamhaus.org/resource-hub/email-security/the-return-of-the-open-relays/) -- Modern open relay abuse patterns

### mTLS and Certificate Management
- [Step v0.8.3: Federation Root Rotation](https://smallstep.com/blog/step-v0.8.3-federation-root-rotation/) -- Certificate rotation in federated PKI
- [mTLS Complete Guide (GitGuardian)](https://blog.gitguardian.com/mutual-tls-mtls-authentication/) -- Operational complexity, trust store gaps, rotation pitfalls
- [mTLS Architecture and Troubleshooting (Axelspire)](https://axelspire.com/vault/architecture/mtls-mutual-tls/) -- Certificate lifecycle challenges at scale

### Federation Design
- [The Perils of Federated Protocols (LWN.net)](https://lwn.net/Articles/687294/) -- Fundamental tension between federation and rapid iteration
- [Wire: Federating Security or Spam](https://wire.com/en/blog/federating-security-or-spam) -- Open vs. closed federation trust models
- [Matrix Decomposition: Academic Analysis](https://matrix.org/blog/2020/06/16/matrix-decomposition-an-independent-academic-analysis-of-matrix-state-resolution/) -- Formal analysis of state resolution correctness

---
*Federation pitfalls research for: AINM v3.2 multi-server federation milestone*
*Researched: 2026-03-28*
