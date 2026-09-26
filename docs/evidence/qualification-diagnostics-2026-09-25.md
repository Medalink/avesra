# Qualification and diagnostic integration checkpoint

This is a source/build checkpoint, not end-to-end acceptance or a release pass.
Plan001 remains in progress. The owner requested continued implementation of the
whole plan and evidence collected without interrupting their desktop use.

The immutable corrected source archive is
`artifacts/qualification-diagnostics-proof-20260925/source-corrected.tar`, SHA256
`d6fe9a7d7401b429fe4b4dabc7719d0ec59fe04411815df11673bb4266cb8d70`.
It includes the typed planner/directedness checkpoint, native utterance and
held-out qualification work, fixed diagnostic task execution, and the bounded
owned27B reasoning deployment descriptor. It precedes durable qualification,
schema13 facts/routines, native Claude operations and packaging work.

## Corrections found before this checkpoint

- Returning the first completed utterance dropped the activity WebSocket while
  its request was still nonfinal. That cancellation terminates the model child.
  Normal completion now closes the microphone, sends actual contiguous captured
  tail as a bounded final chunk, and awaits correlated terminal/socket retirement
  under the original deadline. It does not append synthetic silence or renew time.
- Missing, stale or unqualified measurements could count toward negative
  qualification quotas. Only complete, revision-bound measurements now count;
  missing evidence cannot establish successful rejection.
- Endpoint identity must come from the actual utterance owner rather than the
  separate labelled recording request. Their correlation is now explicit.
- The retained native no-output proof is checked at gate and durable acceptance.
  The original endpoint clock is retained for analysis/directedness latency.
- Windows compilation found a retired capture enum variant, a large unboxed reply
  variant and a collapsible conditional; these were corrected before the pass.

## Observed checks

`scripts/verify.ps1 -Suite Static` passed on the exact corrected snapshot:
Rust formatting/Clippy and TypeScript/Svelte checks, with zero Svelte errors or
warnings. The production bundled Windows build passed in4m45s.

Retained binaries in that artifact directory:

| Artifact | SHA256 |
| --- | --- |
| `avesra-desktop.exe` | `d094a3297768f02732b0f6d82c36f30e6271f6a29bcbf0b7fd9880ed7e974757` |
| `avesra-native-host.exe` | `6fe042ce28a8c7701e2bf47c1af239986cdad6954f8573fa044def4af875d1b0` |
| Spark `avesra-server` | `d0cfb35616781aa0306bdcde740b7b4910faa2c0b50ee59d0d9f4a8cebe3d59d` |

Spark ARM64 static and release checks passed in the pinned Rust container with
two CPUs and8GiB limit. The server is retained under
`~/.local/share/avesra-build/qualification-diagnostics-proof-20260925/`.
No automated tests or acceptance harnesses ran. These artifacts were not installed
by this checkpoint; build results do not qualify an owner or perform an action.

## Runtime boundary

The installed Cisco CLI's actual read-only `stats` command returned exit0 and
disconnected states within its five-second bound. This identifies a usable
read-only native surface; it is not an accepted Avesra diagnostic task or VPN
connection proof. No connect/disconnect command was invoked.

The new reasoning artifact's66 cached weight shards were checked against its
immutable publisher metadata before any owned engine creation. The existing
Local Studio controller rejected a separate launch with HTTP409: its actual
host metadata reports `unifiedMemory:false` and the existing instance holds its
sole device. Read-only source inspection found a computed GB10 shared-memory
flag omitted from returned GPU metadata. The existing controller, its dirty
source and its model were left unchanged. A separately owned controller now runs
on loopback port18080 with its own data, credentials and512MiB/one-CPU limits.
Its copied GPU metadata fix reports shared memory on the actual GB10. Its source
manifest SHA256 is
`1b2be421a75499d40d72cf30977be65f4c5b4d0c5beb5e0b6bee47f0d4f7b900`.
Neither the controller health nor artifact capture establishes loaded inference.

Two actual owned27B engine launches failed during initialization and were removed
by their deployment owner. Each received36GiB memory, zero container swap and
two-CPU limits before importing the inference runtime. The second container,
`c2b482775b55619dedfd04da01ebd30a4a4c5743b5ecd79620d89a2340af196e`,
exited1 after267seconds. The retained HTTP503 response says engine-core
initialization failed; its truncated traceback does not identify the root cause.
Docker events did not report an OOM event. This is not proof that memory was
sufficient: system swap increased during loading, and usable concurrent capacity
remains unqualified. No further launch should repeat without retaining the actual
engine cause before owner cleanup. The original Local Studio model container was
not replaced or restarted.

Three idle Avesra service containers (voice designer, batch activity and candidate
streaming activity) were restarted after actual health reported `busy:false`,
releasing their model loads. ASR, speaker, TTS, generated assets and settings were
preserved. Free host memory after this was about45GiB; this is one setup observation,
not a runtime capacity guarantee. Activity must be explicitly reloaded before
normal voice can be ready.

## Subsequent frozen source checks

Durable qualification and schema13 memory integration passed Windows static and
production build checks. Source archive
`artifacts/durable-qualification-proof-20260925/source-verified.tar` has SHA256
`f4834441d57712d8db79685e00f1286124ec460ec4beda64fd0b314e025685c5`.
Its retained desktop SHA256 is
`294d7dc0a51cfe8ff58fdb9533b9e432350987c3b5e8f1f239afb0ec60c6bebd`;
native-host SHA256 is
`6e99831b9757a1dd9ded60f853ac5fbcaea3d2f10ed06b9911cd5e3d9ef7751a`.
The production build took3m41s. Matching ARM64 static and production builds
also passed, with one CPU and4GiB limits; ARM release compilation took54.03s.
The retained server SHA256 is
`953b93cb9512096a5c19fcd2cd7f3daa07dc304cea5069f083b606392f265ec9`.
These binaries are retained, not installed.

The later Claude draft/output-overlap/initial packaging source archive,
`artifacts/owner-workflows-proof-20260925/source.tar`, SHA256
`6809cdca31eb519a338abcf05c9a7c29e9e62de76febf4d06673ff5855e0fa33`,
passed Windows static checks. Review still found navigation timing and project
association defects in the Claude implementation, and shutdown journal recovery
needed a correction. This snapshot is not a release candidate or workflow pass.

Use [the background evidence procedure](../background-evidence.md) for subsequent
real application media: separate non-input desktop, exact production artifact,
real WebView frames and silent native postmix capture. Never substitute a rendered
mockup or service-only recording for an accepted owner workflow.

## Retained startup cause and corrected owned load

A third owned launch retained the engine exception before owner cleanup:
4096 tokens required0.57GiB of KV cache, while the configured allocation was
0.25GiB. The captured log is
`artifacts/owned-reasoning-image-20260925/startup-retained.log`, SHA256
`953722f9bdfac10e19f28f2e8b46b0621df7d776faeda8e0413f8e4c13b7c388`.
The correction reserves768MiB within the unchanged36GiB memory, zero cgroup swap
and two-CPU limits. It does not raise those limits or restart the original model.

The newly captured immutable image is
`sha256:d7c6827ade3234f66b365a492ff81e927ed64fc1c73f180aaefab69ebf8c2dd0`.
The exact new container
`dcdb60728b6fadbc43eb288870caa637b6588df2c5631015d2d903894a29e721`
completed startup at04:34:48UTC and returned health HTTP200 at04:34:50UTC.
The source helper still returned `engine_identity` on its subsequent real
read-only inspection. Health is therefore not adapter qualification, inference,
or a working conversation. The original Local Studio container's identity and
start time remained unchanged at this checkpoint. GPU unified-memory allocations
are not all represented by the container's cgroup peak; do not use that peak as
proof of full-system capacity or lack of swap pressure.

Only Avesra's streaming activity service was explicitly reloaded after its idle
release. Its real health returned loaded-unqualified, streaming true, busy false
and zero inferences. The production controller's activity configuration and
desktop/server binaries had not yet been updated.

## Integrated source checkpoint

`artifacts/integrated-workflows-proof-20260925/source-corrected.tar`, SHA256
`a8585929ef73b5132f7e3f5aa2ae0fc9df28d70a9337b735f4e73c496da9af7c`,
passed Windows Static with zero Svelte errors/warnings. It includes the corrected
repeatable effect-authority checks, schema16 browser consumer, retained
notification batches, measured quiet handoff, memory retention, current reasoning
Models inspection, and fingerprint-bound qualification restoration. The earlier
`source.tar` in that directory predates review corrections and is not the source
for this result. Production builds and actual installation remain separate.

The owner subsequently prioritized normal conversation. At that point Spark
still answered ICMP and accepted TCP22, but did not send an SSH banner within
five seconds. No reboot or unrelated service restart was used to mask the failure.
Normal listening remains unavailable pending actual adapter and owner qualification;
the previously recorded editable voice previews are a distinct completed path.
