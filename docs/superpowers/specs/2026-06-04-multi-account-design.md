# Multi-Account / Multi-Device Keys — Design (v1)

**Date:** 2026-06-04
**Status:** Approved design
**Author:** brainstorming session (Santiago + Claude)
**Context:** The iOS reference client stores exactly one Ed25519 keypair (a fixed Keychain slot
`kSecAttrAccount = "ed25519.privateKey"`) and one global GRDB database (`applicationSupportDirectory/
ainm.sqlite`), and the RC sub-project persists a single `ainm.session.*` identity. This means one
account per device: onboarding a second identity reuses the same device key and (pre-RC) hit the 409
wall. This sub-project makes the device hold N accounts — each with its own keypair and its own local
data — with one active at a time and fast switching.

## 1. Goal & scope

A device can hold multiple AINM accounts; exactly one is active and connected at a time; the user can
add, switch between, and remove accounts; each account's keypair and local data (threads, messages,
memory) are isolated. The existing single account is migrated in (no data loss).

**Decisions (locked in brainstorming):**
- **Switcher model:** one active account at a time. Switching disconnects the current identity and
  reconnects as the target (the WS connection is single-identity; AppState is single-valued).
- **Per-account data isolation:** each account gets its own GRDB database file.
- **Migrate the existing account in:** one-time migration moves the legacy key + DB under the
  current account (becomes account #1).
- **No Face-ID re-prompt on in-session switch:** Face ID gates launch-resume only (existing RC
  behavior); switching while the app is open does not re-prompt.

**Out of scope:** simultaneous multi-account (concurrent live WebSockets / merged inbox); cross-account
agent threads; syncing the account set across devices; changing the identity/handshake crypto.

## 2. Architecture overview

```
AccountRegistry (UserDefaults, source of truth)
  ├─ accounts: [AccountSummary{ address, serverURL, username, faceIDEnabled }]
  └─ activeAddress: String?

KeyManager(account: address)        → Keychain slot "ed25519.privateKey.<address>"
AppDatabase.make(forAccount:)       → applicationSupportDirectory/ainm-<sanitized-address>.sqlite

AppState
  ├─ accounts / activeAccount (mirror the registry)
  ├─ bootstrap(): resume activeAddress (Face-ID-gated per RC) → open its DB → connect
  ├─ switchAccount(to:): disconnect → repoint KeyManager + reopen DB → connect (no Face-ID re-prompt)
  ├─ addAccount(): run onboarding/recovery for a NEW identity → register → switch to it
  └─ removeAccount(_:): delete key + DB files + registry entry; if active, switch/onboarding

One-time migration (first launch of this build):
  legacy fixed-slot key + RC ainm.session.* present
    → move key to "ed25519.privateKey.<address>", rename ainm.sqlite(+wal/shm) to ainm-<addr>.sqlite,
      seed registry {that account, active}, clear legacy ainm.session.* keys.
```

## 3. Phase 1 — Account registry + per-account keys

**`AccountRegistry`** (new, `App/AccountRegistry.swift`): the source of truth, persisted in
UserDefaults (non-secret).
- `struct AccountSummary: Codable, Identifiable { let address: String; var serverURL: String;
  var username: String; var faceIDEnabled: Bool }` (`id == address`).
- Stored as a JSON array under one key + `activeAddress: String?` under another.
- API: `all() -> [AccountSummary]`, `active() -> AccountSummary?`, `upsert(_:)`, `remove(address:)`,
  `setActive(address:)`, `contains(address:)`.

**`KeyManager` becomes account-keyed.** Replace the fixed `keychainAccount` constant with a
per-account tag derived from the address: the Keychain `kSecAttrAccount` becomes
`"ed25519.privateKey.\(address)"`. The `KeyManager` is constructed for (or its methods take) an
account address. `getOrCreateKeypair`, `hasStoredKeypair`, `deleteKeypair`, restore all operate on
that account's slot. (Keep the in-memory test storage abstraction.)

**One-time key migration.** A `LegacyMigration.migrateIfNeeded()` run at launch (before routing):
if the legacy fixed-slot key exists AND `ainm.session.serverURL`/`username` (RC keys) exist AND the
registry is empty → read the legacy key, write it to the `<address>`-keyed slot, seed the registry
with `{address, serverURL, username, faceIDEnabled}` as active. Idempotent (guarded by "registry
empty" + a done-flag). Leaves the legacy slot in place only until the DB migration (Phase 2)
completes, then clears the legacy `ainm.session.*` keys.

**Files:** `App/AccountRegistry.swift` (new), `Crypto/KeyManager.swift`, `App/LegacyMigration.swift`
(new, or a function on AccountRegistry).

## 4. Phase 2 — Per-account database

**`AppDatabase.make(forAccount address: String)`** → `applicationSupportDirectory/
ainm-\(sanitized(address)).sqlite` (sanitize `@`/`.` to filesystem-safe, e.g. replace with `_`, and
keep a short hash suffix to avoid collisions). `makeDefault()` is retired (or kept only for the
legacy path). The GRDB migrator runs identically on each per-account DB.

**One-time DB migration** (part of `LegacyMigration`): if the legacy `ainm.sqlite` exists and the
migrated account's keyed DB does not, rename `ainm.sqlite` **and its WAL sidecars** (`ainm.sqlite-wal`,
`ainm.sqlite-shm`) to the `ainm-<addr>.sqlite[-wal|-shm]` names. (Move all three together; if WAL
sidecars are absent that's fine.) After a successful DB move + key migration, clear the legacy
`ainm.session.*` UserDefaults keys (migration complete).

**Files:** `Persistence/AppDatabase.swift`, `App/LegacyMigration.swift`.

## 5. Phase 3 — AppState active-account model

Replace the RC single-identity `ainm.session.*` reads with the registry.
- Add `accounts: [AccountSummary]` and `activeAccount: AccountSummary?` (mirror the registry; refresh
  on change).
- `setup()` constructs the active account's `KeyManager` + `AppDatabase`; if no active account,
  remains in onboarding.
- `bootstrap()`: resume `registry.active()` — set serverURL/username, Face-ID gate per RC
  (`SessionGateView` when `faceIDEnabled`), open its DB, connect.
- `switchAccount(to address:)`: disconnect the current client; repoint `keyManager` + reopen
  `AppDatabase` for the target; update `activeAddress`; reconnect. No Face-ID re-prompt. Rewire the
  AINMClient callbacks against the new DB.
- `addAccount()`: enter onboarding (signup OR recovery) for a NEW identity. On a successful connect,
  `registry.upsert(...)` + `setActive` (the verify/finish flow from RC already produces the bound
  key — now stored in the new account's keyed slot). The onboarding flow must target a fresh
  KeyManager slot for the new address, not the active account's.
- `removeAccount(_ address:)`: delete the account's Keychain key + DB files + registry entry. If it
  was active: switch to another account if one exists, else drop to onboarding. (Subsumes RC's
  `signOut`, which becomes "remove active account".)

**Key sequencing concern (call out for the planner):** onboarding a new account creates a keypair
*before* the address is finalized (signup returns the address). The implementation must ensure the
new account's key lands in the `<new-address>` slot — either generate into a provisional slot and
rename on address assignment, or derive the slot from the address once known and bind that. The
planner must resolve this explicitly (the current single-slot `getOrCreateKeypair` sidesteps it).

**Files:** `App/AppState.swift`, `App/AINMMessengerApp.swift` (launch migration + routing),
`UI/Onboarding/*` (thread the target account through Add).

## 6. Phase 4 — Account-switcher UI

A switcher surface reachable from the Conversations screen (e.g. a profile/account chip in the
toolbar → sheet, or a row in a Settings sheet):
- Lists `accounts` with the active one marked (checkmark) and showing `username@host`.
- **Add Account** → onboarding/recovery (`addAccount()`), then switches to the new account.
- Tap a non-active account → `switchAccount(to:)` (shows the reconnecting banner from RC).
- Remove (swipe-to-delete or long-press, with a confirmation) → `removeAccount(_:)`.

**Files:** `UI/Accounts/AccountSwitcherView.swift` (new), `UI/ConversationList/ConversationListView.swift`
(entry point).

## 7. Phase 5 — Tests

iOS XCTest (decode/logic-level, no network):
1. `AccountRegistry` round-trips accounts + activeAddress; `upsert`/`remove`/`setActive` behave;
   removing the active account clears/repoints active.
2. `KeyManager` keyed storage: keys for address A and address B are independent (creating/deleting one
   doesn't affect the other); `hasStoredKeypair(address:)` is per-account.
3. Legacy migration: given a legacy fixed-slot key + `ainm.session.*` + legacy `ainm.sqlite`, migration
   produces the keyed key slot, the keyed DB file, a seeded active registry entry, and is idempotent
   (second run is a no-op); clears legacy session keys.
4. `AppDatabase.make(forAccount:)` yields distinct file paths for distinct addresses and a working
   migrated schema.
5. `AppState.switchAccount` updates `activeAccount` + the registry's `activeAddress` and opens the
   target DB (assert via the active DB identity / a probe row).

Server: no change expected — the RC idempotent-bind already lets distinct per-account keys bind. (If
a multi-account test surfaces a server gap, note it; don't speculatively change the server.)

## 8. Testing & DoD

1. A device can onboard a second account without losing the first; both appear in the switcher.
2. Switching accounts disconnects + reconnects as the new identity and shows only that account's
   threads/memory; no Face-ID re-prompt mid-session.
3. The pre-existing single account is migrated in on upgrade (its key + conversations preserved),
   appearing as the active account; migration is idempotent.
4. Removing an account deletes its key + DB; removing the active one repoints or returns to onboarding.
5. Per-account key isolation verified; registry + migration + switch covered by tests; iOS bundle
   green; server suite unaffected (green except the two known pre-existing failures).
6. A short runbook note documents add / switch / remove and the one-time migration.

## 9. Build order

Phase 1 → 2 → 3 → 4 → 5. Phases 1–2 (keyed storage + DB + migration) are load-bearing and must land
before the AppState model (3) can switch; 4 is the UI; 5 locks it. The new-account key-slot
sequencing (§5 call-out) is the riskiest detail — resolve it in Phase 3's plan.
