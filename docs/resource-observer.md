# Host resource observations

This observer samples every five seconds on separate low-priority native and
controller owners. It never authorizes work, changes a deployment, or runs on the
audio callback/voice path. Settings reads cached observations, not hardware.
Sampler/collector failure leaves voice available and readings unavailable.

Windows measures this native process's CPU time and working set, physical host
RAM/available RAM, and host uptime. CPU is normalized by actual logical processor
count and the same process's monotonic interval. WebView child processes and any
other worker are excluded: this is not a whole-companion budget or gaming impact
measurement. Process creation time is observed; a Windows boot UUID is unavailable.

Linux reads bounded own-process `/proc` counters, actual boot UUID, host memory,
and only its current cgroup-v2 memory/CPU limits and counters. Host available RAM
is unified-memory headroom on Spark, not a reservation or a model working set.
Unlimited cgroup limits are explicit. A fixed absolute `nvidia-smi` invocation
may read up to four GPU rows; unsupported fields remain unavailable independently.
Its one-second deadline requests kill, then the same owner waits for actual child
retirement before another probe. It never scans Docker, models or other processes.

First CPU samples are warmup, decreasing counters/changed process or cgroup are
reset, and intervals outside one to fifteen seconds are gaps. No rate is computed
across those boundaries or across hosts. Every field carries a fixed numeric
metric and either a bounded integer value or an enum-only unavailability reason.
No paths, device serials, arbitrary command output or errors are retained.

The existing nonblocking telemetry writer persists at most 8192 resource samples
per host (about eleven hours at five seconds), under the configured one-to-seven
day age ceiling. Capacity can evict earlier than age. Fixed daily rollups retain
thirty days with an8192-row bound. Current inspection/export returns the newest
720 detailed samples with explicit truncation and all bounded rollups. Loss and
eviction totals are visible. Resource data is host/process-scoped, separate from
accepted-turn traces; selecting a turn does not attribute host usage to that turn.
Access/export retains current native owner/device and paired registration checks.

Trace query/snapshot version2 requires the resource snapshot; older peers fail
explicitly without a fake empty result. Telemetry database schema4 adds separate
resource tables while preserving previous trace rows; old collectors refuse it
nonfatally. Action storage and voice protocols are unchanged. Export keeps typed
resource observations alongside OTLP spans without pretending resource samples
are accepted spans. A missing boot identity or stale sample is explicitly shown.

No PC GPU, game frame-time, capture cost, per-model GPU allocation, swap latency,
engine queue or profile-transition benchmark is implied. Actual runtime/export
and overhead evidence remain separate from source/static verification.
