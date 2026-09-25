# Architecture

`avesra-contracts` owns serializable typed boundaries without desktop/model dependencies. `avesra-core` owns policy, local state and SQLite persistence. `avesra-server` is the Spark controller executable. `avesra-windows` owns native device integration. The Tauri app uses bundled Svelte/Tailwind resources and typed commands. No privileged remote page is loaded.

One writer owns each SQLite connection behind the desktop's synchronization boundary. Audio callbacks must not acquire that lock, block on network, or run model inference. Local controls invalidate capture/action epochs before asynchronous work. The controller independently validates actions; neither frontend nor model owns authority.

Configuration, capability and readiness are separate. Incomplete components expose unavailable with a reason. UI visual states must be derived from Rust runtime events; graph shapes cannot invent signal levels, speaker identity or task progress.

Initial code checkpoints may implement independent parts of later milestones while affected live integrations remain unavailable. Such code does not complete the milestone or any owner sign-off workflow.

## Native media primitives

Windows capture/playback opens are owned by one companion media worker. Startup creates its idle thread; devices remain closed until the native connection, enrollment, service-readiness and local mode gates permit them. Settings never grant readiness. Device/profile changes invalidate capture/action epochs; unrelated preferences do not reopen streams. Each device attempt has separate failure state and an immutable epoch tied to a shared immediate permission gate, so delayed old opens cannot adopt a new device's authority. Fixed 320-sample frames are 16kHz mono PCM; queues hold at most64frames and are disposed on configuration invalidation. Capture frames expire after500ms; overflow closes that attempt. Callbacks perform nonblocking queue access, bounded stale draining, channel conversion, and measured RMS/peak. Only backend-advertised16kHz configurations are supported; Windows playback may use its own advertised PCM conversion. Other rates remain unsupported. No acoustic echo canceller or safe barge-in is claimed. Frames carry the CPAL device capture/playback clock; Instant measures local queue age only. No live stream was opened during the gaming restriction.

The worker publishes measured32-sample waveform events; the overlay accepts only the current capture epoch and increasing sequence while native capture gates allow it, and expires missing data after500ms. Identity is unqualified, so current human frames are neutral background color. Reduced-motion snapshots reset on epoch changes. Output references are drained without claiming echo cancellation. Network forwarding and actual identity/readiness integration remain incomplete; no media can currently become enabled through the setup UI.
