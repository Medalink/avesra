# Driver contracts

## Settings deployment metadata

Models & Drivers and Memory > Health inspect ASR, speaker and TTS through a
bounded authenticated `GET /audio-lanes` metadata request. Each lane independently
reports `not_configured`, `unavailable`, `incompatible`, or `observed` with the
validated existing AudioHealth record. Missing configuration is distinct from an
unreachable or incompatible configured service; a failed lane does not erase a
successful peer. The route shares the existing health-admission semaphore and
retains it until all three bounded health calls return. A bounded task owns the
permit and fixed configured clients independently of the HTTP waiter, so a native
four-second request timeout or HTTP disconnect cannot cancel the health calls or
release their admission early. Each existing health exchange has its own
three-second bound; the Python health operation only reads in-memory metadata.
The task is never aborted on waiter loss. It never loads a model,
performs inference, changes a deployment or accepts a caller-selected path.
Paired-device authorization is rechecked before publication. Unsupported hosts
return unavailable rather than fabricated lane metadata.

Native `audio_lane_health` replaces the Settings-only speaker projection. It
retains the current visible/unlocked Settings, connection-generation and capture
epoch checks before/after paired credential loading and the bounded TLS request.
The strict response decoder validates exact lane identity, immutable model
revision shape, known states, supported streaming flags, cancellation contract
and absence of permission authority. UI replies are discarded after navigation,
hide, lock or connection/epoch change. Probe/Refresh status read metadata only;
they never grant readiness or start capture/playback.

Loaded models display **Loaded · unqualified**, their actual artifact revision,
streaming capability and busy state. Before a successful probe, display **Not
probed**, never infer **Not connected** or **Not configured**. Reasoning, vision,
decision and memory have no connected metadata probe in this slice. Reasoning
shows **Not inspected** / **Status not integrated**, reflecting its implemented
native driver without claiming a configured or ready engine. Vision, decision
and memory remain explicitly **Not integrated**.
This projection fixes hard-coded ASR/TTS placeholders; it does not complete voice
qualification, reasoning activation or any owner workflow. Verification is
source/static/build and separately recorded live metadata/UI evidence; no
automated tests or harnesses are introduced.

Logical lanes: activity/transcription, speaker identity, planning, vision, speech synthesis, decision and memory. Each configured deployment references one driver and immutable artifact revision; lanes may share one resident model.

Health states are configured, starting, loading, ready, degraded, unavailable and incompatible. Only a real capability probe can promote a deployment to ready. Pinning a model revision or seeing a catalog entry cannot do so. No automatic cloud fallback or inference outside the selected profile is permitted.

Local Studio remains owner of reasoning/vision engines and downloads. Dedicated ASR/speaker/TTS runtimes use the same bounded, cancellable lane contracts, without changing Local Studio internals. The speaker/ASR/TTS dependency deltas and final ARM64 images are pinned in services/audio/README.md. Speaker, ASR and TTS final supervisors have actual load-only evidence. None has streaming, live-input quality or owner-acceptance evidence, and no lane may report ready solely from a successful artifact load.

Model/license/provenance facts are in `evidence/m0-preflight.md` and the original setup record. Schema-valid model proposals still pass task/actor/target/approval checks and outcome verification.

The earlier Models and Health checkpoint inspected only the paired speaker route.
Its ASR/TTS placeholders were not evidence about configured deployments. The
Settings deployment metadata contract above supersedes that projection while
preserving the native authority and qualification boundaries.
