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

## Pairing and control transport implementation

The server's opt-in `init <directory> <dns-name>` creates a private installation directory, a self-signed TLS certificate for the selected DNS name, and the authentication database. `pair-code <directory>` writes a one-time random 256-bit pairing code to an owner-only local file, valid for five minutes and at most five incorrect attempts. Codes are read by the owner over the authenticated server administration channel; they are not advertised by a LAN endpoint. The certificate SHA-256 fingerprint is public and must be checked on the local PC setup screen before pairing. No TLS verification bypass is permitted.

`serve <directory>` binds TLS port 9474. `/pair` exchanges a valid one-time code for a fresh device UUID and bearer credential. The durable server database stores only the credential hash. `/control` requires that credential in the Authorization header before WebSocket upgrade, and a new server-selected session UUID on each connection. The first message must be Hello; its current local epochs become that connection's baseline. Later sequence/epoch checks apply. Local mode changes may only advance epochs, never reduce them. Action messages are rejected until owner enrollment and execution are implemented. Pairing alone never creates owner privileges or enables microphone capture.

Revocation is available through the authenticated local server CLI, takes effect on each control message, and closes idle sockets on the next bounded heartbeat interval. Socket frame/message caps are 65,536 bytes, handshake deadline ten seconds, and idle deadline thirty seconds. A new connection cannot resume or replay old queued actions. Authentication/pairing errors contain no supplied secret, and no request bodies enter logs.

Typed action arguments and action/accepted-intent revisions are immutable authorization inputs. Exact argument membership in a controller-derived accepted intent is required. An approval additionally binds the full typed payload and action revision; a changed prompt, amount, proposal digest, target or configuration cannot reuse it. Consequential operations reference an immutable owner-visible proposal plus revision and SHA-256 digest; an executor must resolve and verify that proposal before using it. These primitives remain disconnected from actual desktop effects.

VPN connection changes network state and requires exact local approval. Navigation arguments must be canonical HTTPS URLs without embedded credentials or whitespace/control characters. Read-page targets must be canonical HTTPS origins only. Cancel targets cannot be nil; Hello carries at most the seven known lanes without duplicates, including the initial handshake. Existing application databases with no version marker, multiple version rows, or unsupported versions are rejected before initialization/recovery writes.

The configured Spark endpoint may use an owner-selected DNS name or IP address on HTTPS port 9474; its verified certificate must include the same DNS/IP subject alternative name. There is no global DNS override or certificate bypass. The setup default reflects the currently discovered Spark address, but the saved endpoint is explicit and editable before pairing. A disconnect invalidates pending pair/load generations as well as active sockets, so a delayed result cannot reconnect silently.
