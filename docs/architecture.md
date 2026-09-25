# Architecture

`avesra-contracts` owns serializable typed boundaries without desktop/model dependencies. `avesra-core` owns policy, local state and SQLite persistence. `avesra-server` is the Spark controller executable. `avesra-windows` owns native device integration. The Tauri app uses bundled Svelte/Tailwind resources and typed commands. No privileged remote page is loaded.

One writer owns each SQLite connection behind the desktop's synchronization boundary. Audio callbacks must not acquire that lock, block on network, or run model inference. Local controls invalidate capture/action epochs before asynchronous work. The controller independently validates actions; neither frontend nor model owns authority.

Configuration, capability and readiness are separate. Incomplete components expose unavailable with a reason. UI visual states must be derived from Rust runtime events; graph shapes cannot invent signal levels, speaker identity or task progress.

Initial code checkpoints may implement independent parts of later milestones while affected live integrations remain unavailable. Such code does not complete the milestone or any owner sign-off workflow.

## Native media primitives

Windows `audio::Capture::open` and `Playback::open` are explicit device operations and are not called by current startup/settings code. Gates default disabled. A trusted native state publisher must serialize epoch changes and permissions before enabling. Fixed 320-sample frames are 16kHz mono PCM; queues hold at most64frames. Callbacks perform nonblocking queue access, bounded stale draining, channel conversion, and measured RMS/peak; device errors close the gate. Playback returns actual submitted source frames for future echo-reference processing. Input conversion currently uses linear interpolation and output conversion sample holding; quality is unqualified, and no acoustic echo canceller or safe barge-in is claimed. Consumers must drain/discard queued frames on invalidation, enforce epoch/freshness and30second maximum lifetime, and never persist ambient media. No live stream was opened during the gaming restriction.
