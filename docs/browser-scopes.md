# Exact browser origin grants

This is the next implementation slice under browser-documents.md. Selection and v3 transport authority are implemented source; no origin grant, Chrome permission request or document operation has been activated. Runtime interaction remains deferred. Specs and code only; no test/fixture harness is introduced.

## Native record

Use a dedicated versioned SQLite scope store for mutable permission metadata, distinct from the pairing credential's DPAPI directory. Each record has a native-generated non-nil UUID/revision, durable owner actor, exact selected-pointer revision, pairing UUID/credential revision, immutable browser application UUID/revision, canonical origin, supported operation set and display creation time. Model/page/extension text cannot supply native record IDs or turn a record into an accepted task. No credential is stored in this database.

Canonical origin is ASCII `https://` plus a domain host using the standard HTTPS port, at most 512 bytes. Parse with the shared URL implementation, reject credentials, controls/whitespace, IP literals, path/query/fragment, trailing DNS dot, explicit default-port spelling and noncanonical case/encoding. Require exact equality with the parsed origin serialization. Unicode names must be explicitly represented by their canonical ASCII form. A Chrome pattern is derived only as this exact origin plus `/*`; it is not parsed back as native authority. Native equality remains exact even if Chrome host permissions cover a wider set.

The initial operation set is explicit bounded reading and same-origin navigation. Fill/submit are separate future operations requiring their actual accepted-action/approval integration; no blanket browser-control bit is introduced. Store a sorted nonempty duplicate-free set of supported operations. Reading/navigation permission does not establish mailbox/account/project identity or a successful workflow.

Initialization rejects any occupied unversioned database, unknown/ambiguous schema marker or malformed required tables before schema mutation. Initialize the supported schema transactionally. Publish under an immediate transaction with a fixed bounded busy wait, enforce at most 32 saved records and reject duplicate owner/selection/origin rows instead of merging or widening them. New native revisions are insert-only; changing operations requires explicit old-revision revocation and a new proposal. Read validates row identity against its bounded serialized body. Revocation checks exact UUID/revision and owner, removes that one record and cannot erase a replacement. Failed/uncertain commit is reconciled by exact status; no automatic write retry.

## Two-sided proposal and ownership

Visible native Settings creates at most one pending proposal for its exact currently authenticated selected connection. Consume the original native owner-management proof before awaiting owner/database work. Generate proposal IDs natively and bind owner, selected reference, pairing, app/revision, native session/transport generation, action epoch, canonical origin and explicit operations. Its fixed 45-second monotonic deadline includes all queue/browser/user waits; current original proof must still be valid at durable publication. Keep native management lifetime separate from selected transport lifetime.

The extension receives the exact proposal only over its authenticated native port with current v3 status. It displays the origin and operation set in the popup. Only a direct popup button gesture may call `chrome.permissions.request` for the derived exact host pattern; background/native arrival cannot grant it. Check actual permission after request. Retain one owned request until the browser promise completes, even if its popup closes, and never let an old result attach to a newer proposal/connection. Before reporting completion recheck native connection/selection/action context, original proposal ID/revision and monotonic deadline. A late browser-only grant is reported as such and does not recreate native authority.

The native decision handler independently rechecks its original pending proposal, current session/selection/epoch, original owner proof and exact permission-result correlation before queuing persistence. The permission result is extension-observed evidence, never an owner grant on its own. The actual blocking writer retains its admission slot through completion; publication validates native current context after any database wait immediately before commit. Do not hold Runtime.local across SQLite, browser APIs or thread waits. The saved grant is not a DOM capability or executable AcceptedIntent.

Settings close/release cancels pending scope management but preserves only an already authenticated selected transport. Stop/Pause/action-epoch change, lock, disconnect, selected change and credential revocation withdraw the pending scope and all derived document admission before any later asynchronous result. `permissions.onRemoved` withdraws extension ownership immediately; native revocation messages remain correlated to exact current grant/revision. Any persisted native record remaining after independent Chrome permission removal is unavailable until actual permission is re-established through explicit setup; it cannot silently authorize an API call.

## Recovery and entry points

Settings must list/clear valid native grant records independently of catalog discovery or extension availability. A corrupt scope database is unavailable and preserved; do not delete/reinitialize it automatically. No page content is needed for permission management. Display actual saved/Chrome-only/unavailable state without calling any of them a verified live page or account.

| Entry point | Required boundary |
| --- | --- |
| Shared canonical origin and operation parser | Reject ambiguous or broadened input before proposal construction |
| Scope SQLite open/read/publish/revoke | Exact supported schema, bounded records, original commit authorization and revision-bound recovery |
| Native Settings proposal/list/revoke | Visible native management lifetime, original owner proof, exact selected session |
| Authenticated extension proposal receipt | Strict typed correlation and current observation owner; no user gesture inferred |
| Popup permission button | Direct gesture, exact derived host pattern, one retained browser promise |
| Permission result / onRemoved | No stale proposal adoption, no implicit grant expansion, immediate withdrawal |
| Future document/effect ingress | Requires actual native grant plus current Chrome permission, document identity and accepted task; absent in this slice |

The scope store and handshake will be implemented incrementally with source/static evidence. No Chrome permissions or sites are exercised during implementation, and their absence must not be relabeled as qualified M5 behavior.
