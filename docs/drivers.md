# Driver contracts

Logical lanes: activity/transcription, speaker identity, planning, vision, speech synthesis, decision and memory. Each configured deployment references one driver and immutable artifact revision; lanes may share one resident model.

Health states are configured, starting, loading, ready, degraded, unavailable and incompatible. Only a real capability probe can promote a deployment to ready. Pinning a model revision or seeing a catalog entry cannot do so. No automatic cloud fallback or inference outside the selected profile is permitted.

Local Studio remains owner of reasoning/vision engines and downloads. Dedicated ASR/speaker/TTS runtimes use the same bounded, cancellable lane contracts, without changing Local Studio internals. The speaker/ASR/TTS dependency deltas and final ARM64 images are pinned in services/audio/README.md. Speaker and ASR supervisors have actual load-only evidence; TTS final-image startup remains pending. None has streaming, live-input quality or owner-acceptance evidence, and no lane may report ready solely from a successful artifact load.

Model/license/provenance facts are in `evidence/m0-preflight.md` and the original setup record. Schema-valid model proposals still pass task/actor/target/approval checks and outcome verification.
