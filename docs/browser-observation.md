# Native-issued browser document observation

This is the next source slice after the v4 scope setup path. It implements bounded document metadata discovery and exact native selection before semantic page extraction. No discovery, browser API, page content or permission is exercised during implementation. It does not complete the Gmail/X/prompt workflows or open action admission.

## Admission and scheduling

An explicit visible native Settings action may inspect documents for one exact saved native scope with the Read operation. Consume the original owner-management proof before owner/scope/catalog reads. Validate the current principal/actor, immutable selected pointer and pairing, selected authenticated native session/generation, action epoch and scope UUID/revision. The exact configured app revision must still exist. This is a concrete setup observation, not inferred permission for passive monitoring or a model-proposed task.

Use one owned request per selected connection, created with a fresh native UUID and a fixed five-second monotonic deadline starting before queued work. The request repeats the exact scope reference, origin, native session/generation, selection revision and action epoch. It neither carries arbitrary script nor lets the extension choose native identity. If validation or storage work consumes the budget, do not publish an extension request. Keep storage/native inspection outside Runtime.local and retain actual blocking ownership until completion.

The next browser protocol version must require a typed nullable observation request in status and a correlated typed result; advance handshake/comparison domains together. Do not accept a missing field as an older-version fallback. The selected transport's one-second status polling continues while browser APIs are in flight. An async document job must not block the status receive handler and accidentally manufacture a three-second transport expiry. Result delivery uses the same contiguous request sequence as Poll/ScopeResult. There is one bounded result slot, no unbounded promise/request queue.

The extension snapshots its private per-connection observation owner before any browser API. Every awaited result must still match that owner, request UUID, scope reference, selection, action epoch, native session/generation and original deadline. A timeout withdraws publication immediately but does not claim Chrome canceled an outstanding API. Retain the actual operation slot until the pending API settles; do not overlap a replacement job with abandoned work. Disposal invalidates its result and local candidates synchronously.

## Candidate discovery

Check current `permissions.contains` for the exact derived host pattern, then query tabs matching that pattern. Native origin equality remains the stricter boundary; Chrome pattern matching alone is insufficient. Do not add the broad `tabs` permission merely to read unrelated tab metadata. Add only the documented webNavigation permission needed for document identity and lifecycle; event listeners discard unrelated details without logging or retention.

The initial adapter supports top-level frames only. Keep at most 16 candidates; exceeding the limit reports truncated/unavailable selection rather than guessing which tab is the target. Validate every returned tab/window ID as a positive bounded integer and reject incognito, discarded or unsupported tab states. Do not focus, activate, navigate, reload or create a tab during discovery. A page title and URL are untrusted metadata, never authenticated account/project proof.

Resolve frame zero with webNavigation.getFrame and require a canonical non-nil document UUID and active lifecycle. Re-read the tab and exact document using documentId, tabId and frameId together before accepting a candidate. Require exact current canonical HTTPS origin, unchanged full URL, tab/window identity and a matching lifecycle generation. Full URLs are bounded to 2,048 UTF-8 bytes, reject credentials/control characters and require canonical URL serialization; origin uses the existing strict domain policy. They remain transient private setup metadata, not logs, metrics or durable model instructions. Unsupported frames or inaccessible identity produce explicit unavailable status.

Capture a checked monotonic lifecycle revision before each asynchronous read. Register event handlers synchronously at worker startup. Relevant navigation start/commit/error, history/fragment update, tab replacement/removal/update, activation and window-focus changes synchronously invalidate in-flight candidates before attempting any asynchronous reconciliation. It is acceptable for this first adapter to conservatively invalidate the whole bounded roster; do not ignore an event to retain apparent success. documentId does not substitute for this revision, and DOMContentLoaded is not required for BFCache invalidation.

Candidate results are bound to the original request and contain browser tab/window/frame/document identity, exact URL/origin and observation revision only. No body text, cookies, account name, DOM node or screenshot is read in this slice. Validate the complete typed result and its frame-size bound natively before retaining it. Duplicate tab/document entries, mixed origins, malformed lifecycle or a late result invalidate the roster; never partially publish plausible entries as verified complete discovery.

## Native selection and recovery

The native roster is transient and expires with the original five-second request; a response does not extend it. Explicit selection consumes the exact current roster/request and candidate identity while rechecking current selected connection, scope, original management context and action epoch. A fresh extension revalidation round must use a new bounded request, not turn an expired roster into a live document by copying its IDs. No automatic refresh/selection loop is allowed.

Native selection creates an opaque native document UUID/revision bound to the validated source identity and scope. Browser-supplied numeric IDs and URL text cannot directly name a native target. This document reference is observation provenance, not an executable grant or accepted task. Future read/navigation dispatch still requires a durable accepted action, exact target/operation, current native scope, actual Chrome permission and fresh document revalidation. Existing task authority must not be rewritten if a later UI selection changes.

The subsequent semantic bridge must run on the existing NativeEffects/ExecutionController owner after Store durably claims an exact action revision/dispatch. Its Action.target_id must resolve to the immutable native document binding; the ReadPage origin must equal that binding's strict browser::Origin, and Navigate must satisfy the same stricter origin policy even though the general action URL parser accepts more URLs. The bridge rechecks the existing durable actor/grant/approval/session lease immediately before issuing the one observation/effect and retains ownership through the actual browser operation and result finalization. It must not open a second Store, seed AcceptedIntent from a page/extension result, or convert this setup roster into a normal semantic read. No accepted-intent producer currently exists for that bridge; the boundary stays dormant until implemented. A missing/ambiguous document becomes NeedsInput, not a guessed tab or automatic dispatch replay.

Settings hides/cancellation revoke management requests and transient rosters immediately without treating an already authenticated selected connection as disconnected. Lock, disconnect, Stop/Pause/action epoch, selection or credential/scope revocation withdraw both native and extension observation authority. An old cleanup only cancels its exact request UUID. Failed/uncertain observation can be requested again explicitly with a new identity; no browser mutation has been issued by this slice. The five-second roster lifetime can leave little selection time after API work: show expired state and an explicit Refresh action instead of extending the original lifetime to make the interface appear ready.

## Entry points and deferred work

| Entry point | Required ownership |
| --- | --- |
| Native inspect/cancel/status | Visible Settings, original actor/proof/context, exact saved Read scope, one owned request |
| Typed status request/result | Required version, exact request/scope/session/selection/epoch, contiguous control sequence |
| Extension discovery/revalidation | Private observation owner, actual permission, fixed deadline, one actual in-flight API owner |
| Navigation/tab/window events | Synchronous invalidation before async reconciliation; no retained unrelated metadata |
| Native candidate selection | Exact current roster and revalidated source; fresh opaque native document identity |
| Later semantic/action bridge | Accepted task and fresh scope/document/element authority; absent here |

Semantic observation, DOM mutation revisions, opaque element handles, accepted ReadPage/navigation dispatch and application-specific account/project predicates follow this slice. No document metadata helper is reported as a working mailbox, social feed or coding-app workflow. UI must reuse the approved divided cards and explicit scope summaries, displaying observed identity and unavailability without prototype data.

Primary API references: [webNavigation](https://developer.chrome.com/docs/extensions/reference/api/webNavigation) documents its manifest permission, frame-zero identity, documentId/getFrame correlation, lifecycle changes and BFCache behavior; [tabs](https://developer.chrome.com/docs/extensions/reference/api/tabs) documents tab query and metadata permissions. Those are API facts, not runtime qualification of this adapter.
