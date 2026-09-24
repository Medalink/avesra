# M0 preflight — 2026-09-24

Status: PARTIAL. Current host inventory is verified. Concurrent audio inference, enrollment, real browser extension access, app prompt control, and VPN routing remain unavailable evidence. This is not a working-assistant or release claim.

Source checkout: `f085d25cc4e8b4ee59da7c6d7e710ce5d9ecc5ae`, isolated branch `codex/avesra-plan-001`. The diff from planning baseline `eb0189f` contains only plans and approved designs; no application code or AGENTS.md was present. The owner authorized execution and subsequently directed **specs and code only, no tests**. Automated tests and acceptance harnesses will not be created or run for this execution; compilation, static checks and direct inspection remain distinct evidence.

## Current discovery

Read-only commands ran on 2026-09-24 at approximately 23:40–23:48 UTC. No other host was contacted. No mailbox, prompt contents, browser cookies or credentials were printed or collected.

| Area | Observed evidence | Limit |
| --- | --- | --- |
| Spark | SSH alias `spark2`, hostname `spark-c8bb`, aarch64, Ubuntu 24.04.5 | Only this host is in scope |
| GPU/runtime | GB10, driver 580.178.04, Docker 29.6.2, NVIDIA Container Toolkit 1.20.1 | GPU utilization was 0%; memory counters return N/A |
| Capacity | 121 GiB RAM, 65 GiB used, 55 GiB available; root 3.2 TiB free | Whole-host snapshot, not reserved capacity or workload benchmark |
| Services | `local-studio-controller`, `comfyui-gguf`, and `qwen38-flash-supervisor` user services active | Service activity does not establish model readiness; ownership conflict on reboot remains unresolved |
| Local Studio | Revision `d1abdef92f382ac647c7ffe6e83fa74959a49f0b`; existing two changed source files and two untracked patches preserved | No source or service modifications |
| Serving | `local-studio-llm` running; ports 8080, 8888 and 8189 listen on all IPv4 interfaces; 8888 also IPv6 | TLS setup still required for the Avesra transport |
| Authentication | Anonymous `/v1/models` returns HTTP 401 on 8080 and 8888; authenticated catalog returns `avesra-fast` and `avesra-quality` | Catalog presence is not an inference capability benchmark |
| Artifact provenance | Existing image local ID `sha256:d464f3b466fa9c45ddbff8a812e80564503b6879a9fd95c1a47514f3f0df5a4a`; Docker RepoDigest `vllm/vllm-openai@sha256:fc120ece0a388cc0aa1caad4a9f1cd92113484ab7ec2fd0efadd62585be05bf8` | Registry digest recovered; fresh pull/reproduction has not run |
| Spark tooling | Python 3.12.3; `command -v` found no cargo/rustc/uv on ordinary SSH PATH | Linux build requires scoped toolchain installation |
| Windows | Windows 11 Pro 10.0.26200, 64-bit | No elevation/configuration changes |
| Build tools | Rust/Cargo 1.97.0; pnpm 11.10.0; Node 24.16.0; uv 0.12.17; VS 18 Community with x64/x86 C++ tools | Tool availability does not prove a Tauri build |
| WebView2 | 153.0.4234.48 | Registry discovery |
| Client GPU | RTX 5090, driver 616.92, 32607 MiB total, 2642 MiB used, 7% utilization | Transient sample; no Avesra inference allocated |
| Audio | Razer Kiyo Pro microphone; Razer Seiren X microphone; Realtek USB2.0 Audio SPDIF output; all endpoint status OK | Owner selection, capture quality and playback unverified |
| Displays | Two 3440×1440 displays; second begins at x=3440; work areas 3440×1392 | Per-monitor DPI/input transforms not yet verified |
| Browser installs | Chrome 153.0.8010.53; Brave 154.1.96.59 | Browser/profile/account selection pending |
| Browser policy | No Chrome/Brave policy roots found under inspected HKCU/HKLM paths | This is not proof of unmanaged installation or native messaging success |
| Claude | Registered Store identity `Claude_pzs8sxrjxfjjc!Claude`, package 2.7032.0.0 | Project/prompt/new-chat control access not established |
| Codex | Running executable beneath local OpenAI Codex installation | Current UIA query yielded no matching main window; not an unsupported-interface conclusion |
| VPN | Cisco Secure Client UI 5.1.20.2493 and running vpnagent | Profile, credentials/MFA and routing behavior remain owner setup |

Credential reference only: Local Studio API and inference keys reside in `/home/medalink/.config/local-studio/controller.env`, permission mode 0600. No values are included here.

Local model README license metadata identifies Apache-2.0 for both Qwen reasoning packages, both Qwen TTS packages and SpeechBrain ECAPA; Nemotron identifies the NVIDIA Open Model License. Exact revisions and historical artifact hash verification are in `plans/spark2-model-setup.md`; that earlier inference evidence is not relabeled as a current run.

## Scoped dependencies

1. Dedicated streaming ASR/speaker/TTS runtimes are not serving. Pin compatible ARM64/GB10 dependencies and directly inspect concurrent readiness/latency before enabling live voice. No speech accuracy, underrun or p95 result is claimed.
2. Browser native messaging remains unimplemented/unproven. The no-tests instruction excludes the planned fixture test suite; real setup and direct inspection remain required before claiming browser control.
3. Owner-selected browser/profile/account, microphone/output, Claude IRIS mapping, generated voice and VPN profile/MFA are pending input. Keep these unconfigured in code; no guesses or mailbox inspection.
4. The existing Qwen supervisor and ComfyUI are unrelated workloads. Do not stop/reset/repoint them; deployment contention and reboot ownership need resolution before unattended operation.
5. M1 contracts, safety state machines, build scaffolding and honest unavailable-state UI can proceed independently under plan section 18. M0 as a whole remains partial.

Commands used: Git status/revision/diff, PowerShell `Get-Command`, `Get-CimInstance`, `Get-PnpDevice`, `Get-StartApps`, narrowly filtered process paths, version resources, WebView/policy registry reads, screen geometry, read-only UIA control-type counts; SSH `hostname`, `uname`, OS release, `nvidia-smi`, `free`, `df`, Docker version/ps/image-inspect, service state, `ss`, tool discovery and bounded authenticated catalog reads. No tests ran.
