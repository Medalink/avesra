# Verification boundaries

The owner instructed this execution to use **specs and code, no tests**. Do not create automated tests, acceptance runners, fixtures or test harnesses. Planned test commands are intentionally unavailable; never report them as passing. Compilation/type/static results and direct runtime/UI observations must be reported separately.

M0 [current discovery](evidence/m0-preflight.md) identifies verified prerequisites and scoped unavailable integrations. M1 build/static commands will fail if expected components or tools are missing. Source availability or a listening port alone does not establish feature readiness.

Live owner workflows OW1–OW3, voice recognition, real browser/native prompt access, resource/latency targets, acceleration/Gaming transitions and eight-hour soak remain unverified until directly demonstrated through Avesra. Manual agent actions cannot substitute for the shipped assistant. This execution cannot claim the original test-based release acceptance gates.

## Source checkpoint, 2026-09-25 UTC

`pnpm -r check` passed: extension TypeScript and Svelte (0 errors, 0 warnings). `pnpm -r build` bundled desktop and MV3 resources. `cargo check --workspace` and `cargo clippy --workspace --offline -- -D warnings` passed on Windows, including Tauri and the native host. The lockfile was refreshed after adding the desktop storage worker dependency; subsequent scripted gates use `--locked`.

The first frontend type check exposed unsupported TypeScript 7/Svelte-check compatibility; TypeScript 6.0.3 was selected and checks passed. Initial Rust compilation found an unsupported unsigned SQLite timestamp binding and missing Windows icon; checked timestamp conversion and the Avesra icon fixed those compile errors. These are build observations, not automated tests.

Implemented checkpoint: bounded protocol schema, pure policy/state primitives, transactional SQLite settings/task skeleton, native audio-device enumeration, persistent Tauri local controls/tray, six-section settings and approved overlay signal shader port, MV3 connection UI, fail-closed native status host, loopback server health. Authenticated transport, actual inference/media, owner identity, tool execution and workflow control are not yet implemented. Native host registration is intentionally absent until exact extension-origin setup. No lane is reported ready.

Run `pwsh -NoProfile -File scripts/verify.ps1 -Suite Static` for Rust format/clippy and frontend types. `-Suite Build` bundles frontends and release Rust executables; it does not install or sign them. On Spark, `bash scripts/verify.sh static|build` covers contracts/core/server only and expressly excludes unqualified audio runtime images. Original Plan 001's complete Windows/Spark/live gates remain incomplete.
