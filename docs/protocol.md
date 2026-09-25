# Protocol v1

Transport is authenticated TLS WebSocket; plaintext LAN action/media transport is forbidden. An authenticated connection must negotiate version, device/session identity and epochs before application messages. The contract library validates schema and freshness but does not authenticate a peer itself; callers must supply trusted session state from transport authentication. No action transport is enabled until authentication is implemented.

Control JSON has a 65,536-byte limit. Unknown fields, message variants, versions and invalid UUID identities fail closed. Sequence numbers must strictly increase within the authenticated session. Device/session/epoch mismatches are stale. An action expires at `now >= expires_at_ms`; valid duration is at most 30 seconds. Persisted wall-clock expiration does not replace monotonic process deadlines.

Audio uses PCM signed 16-bit little-endian mono at 16 kHz, at most 320 samples per frame (20 ms), 64 queued frames, a 30-second transient-media TTL and 30-second maximum utterance. Image payloads are at most 4 MiB with one latest passive observation queued. These limits are contracts, not a claim that capture/transport is implemented. Rejected frames are not persisted.

Tool requests bind UUID request/task/step/actor/target/grant IDs, device and session identities, sequence, capture/action epochs, expiration and optional exact approval. Actions have enumerated operation kinds; arbitrary shell/JavaScript/CDP commands are absent. Mutating action retry requires reconciliation. Unknown effect is a durable outcome, never success or permission to retry.

Capability manifests identify lane, driver, model/revision, deployment/locality, supported input formats, streaming/cancellation and actual health. Configured/downloading/responding is not ready. Readiness requires a successful actual capability probe for the same revision.

Trace records contain opaque correlation IDs, stage, host, deployment/config revisions, host-local monotonic queue/duration values, outcome and retry count. No content, credentials, transcripts, raw media or speaker vectors are valid trace fields. Cross-host wall clocks are never subtracted for latency. Missing observations remain unavailable.

## Entry points

| Surface | Scope |
| --- | --- |
| `decode_control` / `Envelope::validate` | Wire parser and freshness validation; no authenticated server route yet |
| `PolicyContext::authorize` | Pure authorization used before dispatch; trusted context must be established by controller/local executor |
| `LocalState::apply` | Local control invalidation; cannot grant readiness |
| `Store` | Sole SQLite writer, validated settings and task transitions |
| Tauri typed commands | Local settings/control operations only; no arbitrary execution or voice acceptance |
| MV3 native bridge | Explicit user connection; rejects web-page-originated requests |

No automated contract tests are created under the owner's instruction. This inventory documents enforcement responsibilities and implementation boundaries.
