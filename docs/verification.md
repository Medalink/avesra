# Verification boundaries

The owner instructed this execution to use **specs and code, no tests**. Do not create automated tests, acceptance runners, fixtures or test harnesses. Planned test commands are intentionally unavailable; never report them as passing. Compilation/type/static results and direct runtime/UI observations must be reported separately.

M0 [current discovery](evidence/m0-preflight.md) identifies verified prerequisites and scoped unavailable integrations. M1 build/static commands will fail if expected components or tools are missing. Source availability or a listening port alone does not establish feature readiness.

Live owner workflows OW1–OW3, voice recognition, real browser/native prompt access, resource/latency targets, acceleration/Gaming transitions and eight-hour soak remain unverified until directly demonstrated through Avesra. Manual agent actions cannot substitute for the shipped assistant. This execution cannot claim the original test-based release acceptance gates.

## Source checkpoint, 2026-09-25 UTC

`pnpm -r check` passed: extension TypeScript and Svelte (0 errors, 0 warnings). `pnpm -r build` bundled desktop and MV3 resources. `cargo check --workspace` and `cargo clippy --workspace --offline -- -D warnings` passed on Windows, including Tauri and the native host. The lockfile was refreshed after adding the desktop storage worker dependency; subsequent scripted gates use `--locked`.

The first frontend type check exposed unsupported TypeScript 7/Svelte-check compatibility; TypeScript 6.0.3 was selected and checks passed. Initial Rust compilation found an unsupported unsigned SQLite timestamp binding and missing Windows icon; checked timestamp conversion and the Avesra icon fixed those compile errors. These are build observations, not automated tests.

Implemented checkpoint: bounded protocol schema, pure policy/state primitives, transactional SQLite settings/task skeleton, native audio-device enumeration, persistent Tauri local controls/tray, six-section settings and approved overlay signal shader port, MV3 connection UI, fail-closed native status host, loopback server health. Authenticated transport, actual inference/media, owner identity, tool execution and workflow control are not yet implemented. Native host registration is intentionally absent until exact extension-origin setup. No lane is reported ready.

Run `pwsh -NoProfile -File scripts/verify.ps1 -Suite Static` for Rust format/clippy and frontend types. `-Suite Build` bundles frontends and release Rust executables; it does not install or sign them. On Spark, `bash scripts/verify.sh static|build` covers contracts/core/server only and expressly excludes unqualified audio runtime images. Original Plan 001's complete Windows/Spark/live gates remain incomplete.

## TLS checkpoint dd8f052

On 2026-09-25 UTC, the Windows Static wrapper passed at `dd8f052`: locked workspace clippy, formatting, extension types, Svelte 0 errors/0 warnings. The earlier complete Windows release build passed in 2m28s before the TLS additions; it is not relabeled as a build of the new transport.

The actual Spark2 ARM64 Static and Build scripts passed for `dd8f052` in an isolated compiler container: static Rust compilation 9.56s, release build 35.53s. Image: `rust@sha256:8fa55b2f3ddf97471ab6a767bfa3f37e6bad0986ba823e75fea57e2a2a5c3073`. Source: `/home/medalink/.local/share/avesra-build/dd8f052`. The container was removed on completion; no GPU was requested. Existing Local Studio and other services were preserved. The initial archive attempts exposed login-shell PATH resetting and Windows line endings in the shell script; non-login execution and the committed `*.sh text eol=lf` attribute resolved those defects.

TLS transport code now includes one-time pairing, server revocation, bounded authenticated WebSocket sessions and per-user DPAPI credential storage. Its current control connection never admits effects or marks owner/voice setup ready. Pairing/live reconnection and owner workflow behavior remain separate, unverified claims until directly observed.

2026-09-24 correction checkpoint: single-job `cargo check -p avesra-contracts -p avesra-core -p avesra-server --locked -j 1` passed; desktop `svelte-check` reported zero errors/warnings. No tests or UI interaction were performed during the gaming restriction. The desktop Rust pending-generation change still requires the independent workspace build. The current Spark certificate covers `spark-c8bb`; Windows does not resolve that name. Configured-IP client support does not make that old certificate valid for the IP; a separate explicitly initialized IP-SAN installation is required before direct-IP qualification.

2026-09-24 direct-IP TLS inspection: new `/home/medalink/.local/share/avesra-controller-ip` has an explicit IP SAN192.168.50.11 (observed with openssl). Public certificate SHA256 `b9bc5476ded049bd40f93a5e0476406ed7ed3789f511367c8c2e966d823da43a`. The old controller directory remains unchanged and its read-only device count was zero. Only Avesra's transient old preflight unit was stopped; `avesra-controller-ip-preflight.service` now serves the new directory using the prior dd8f052 ARM64 binary. Windows `curl --cacert artifacts/spark-ip-public-cert.pem https://192.168.50.11:9474/health` succeeded with voice unavailable/action_execution disabled. No pairing code/device credential was created, no TLS bypass or global DNS edits used, and this is not shipped-client session proof.

Native media source checkpoint: `cargo clippy -p avesra-windows --locked -j1 -- -D warnings` passed. It does not establish capture/playback quality, actual echo cancellation, media transport or owner enrollment. All streams remain uninvoked; no application/window/browser interaction occurred.
