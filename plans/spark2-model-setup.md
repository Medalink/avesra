# Spark2 model setup evidence

Date: 2026-09-24. Scope: owner-authorized model provisioning in the existing Local Studio installation, plus planning updates. Avesra application code remains unimplemented. This is a setup record, not proof of the three owner workflows.

## Verified environment

| Item | Observed value |
| --- | --- |
| SSH target | `spark2`, user `medalink`, hostname `spark-c8bb` |
| Platform | Ubuntu 24.04.5 LTS, ARM64, NVIDIA GB10, driver 580.178.04 |
| Capacity at discovery | 121 GiB total RAM, approximately 118 GiB available; 3.3 TiB disk free |
| Local Studio | `/home/medalink/local-studio`, revision `d1abdef`, existing local source patches preserved |
| Controller | User service `local-studio-controller.service`, authenticated API on port 8080; configured inference port 8888 |
| Model root | `/home/medalink/.cache/huggingface/avesra/` |
| Registry | `/home/medalink/.local/share/local-studio/model-index.json` |
| Registry backup | `model-index.pre-avesra-20260924T230555Z.json` in the same directory |
| Existing runtime | ARM64 `vllm/vllm-openai:qwen38-flash-next`, pinned locally by image ID below |
| Runtime versions | vLLM `0.1.dev20073+g8e685d198`, PyTorch `2.13.0+cu130`, Transformers `5.15.1` |
| GPU runtime probe | Container GPU access succeeded; GB10 capability 12.1, CUDA 13.0; sum of 0..1023 returned 523776 |
| Client GPU | RTX 5090, 32,607 MiB total VRAM, driver 616.92; capacity must be rechecked under workload |

Pinned local image ID: `sha256:d464f3b466fa9c45ddbff8a812e80564503b6879a9fd95c1a47514f3f0df5a4a`. This is a local image content ID, not a portable registry pull digest. A reproducible installation on another host still needs a verified registry digest or export/build provenance.

## Models and Local Studio registration

All downloads were submitted through the authenticated Local Studio `POST /studio/downloads` API with exact model revisions and destinations beneath its configured model directory. Recipes were added through `POST /recipes`, not by directly editing its database. Existing API credentials were reused without printing them. The registry backup and registry were restricted to the owner account.

| Lane | Model | Pinned revision | Setup state |
| --- | --- | --- | --- |
| Reasoning/vision comparison | `Qwen/Qwen3.6-35B-A3B-FP8` | `95a723d08a9490559dae23d0cff1d9466213d989` | Download in progress; recipe `avesra-fast` registered, serving unverified |
| Reasoning/vision comparison | `Qwen/Qwen3.8-27B-FP8` | `017b9c7af6b5689d5dd426a76e0bc077eb5ca20a` | Download in progress; recipe `avesra-quality` registered, serving unverified |
| Streaming recognition | `nvidia/nemotron-speech-streaming-en-0.6b` | `ebe59e5a817142986528bbbee5dba8db7b38ed50` | Downloaded; large-file SHA-256 checks passed; serving adapter unverified |
| Reusable voice synthesis | `Qwen/Qwen3-TTS-12Hz-0.6B-Base` | `5d83992436eae1d760afd27aff78a71d676296fc` | Downloaded; large-file SHA-256 checks passed; serving adapter unverified |
| Voice design during setup | `Qwen/Qwen3-TTS-12Hz-1.7B-VoiceDesign` | `5ecdb67327fd37bb2e042aab12ff7391903235d3` | Downloaded; large-file SHA-256 checks passed; serving adapter unverified |
| Speaker embeddings | `speechbrain/spkrec-ecapa-voxceleb` | `0f99f2d0ebe89ac095bcc5903c4dd8f72b367286` | Downloaded; large-file SHA-256 checks passed; serving adapter unverified |

`fast` and `quality` are comparison labels, not benchmark conclusions. Both recipes use 16,384 context, at most two sequences, 0.50 GPU-memory utilization, a 2,048-token batch budget, one image per request, and no video inputs. Remote model code is disabled. The configured Qwen tool parser is `qwen3_coder`, reasoning parser `qwen3`; inference authentication is enabled. These are conservative starting settings, not tuned performance results. Models are alternatives, not a requirement to keep both resident.

The two TTS downloads include their speech tokenizers. The speech-recognition download includes the NeMo and Safetensors forms while excluding the optional GGUF copy and figures. Speaker example recordings were excluded. Approximately 80.45 GB total is requested across all six downloads.

Completed audio/identity artifacts were checked against file sizes and upstream LFS SHA-256 metadata at the pinned revisions. The redacted results are stored on Spark2 at `/home/medalink/.local/share/avesra-setup/artifact-verification.json`. This establishes artifact integrity, not inference correctness, voice accuracy, or end-to-end latency.

## Confirmed limitations and next verification

- The installed Local Studio launcher accepts vLLM, SGLang and exllamav3 recipes. Its current vLLM image advertises the Qwen3.5 architectures used by the reasoning candidates, but not the selected TTS, ECAPA or Nemotron streaming-ASR architectures. Those four model packages are visible in Local Studio's downloaded-model inventory; no fake chat recipes were created for them. The owner chose dedicated local speech/speaker drivers alongside Local Studio, with every lane swappable through Avesra. Those service adapters remain implementation work.
- Local Studio's active-model proxy is a single active-model surface. Simultaneous ASR, TTS, speaker verification and reasoning require an explicit, tested lifecycle/routing arrangement. Avesra must not treat a model swap as concurrent lane serving.
- The existing `qwen38-flash-supervisor.service` was active but held after three launch failures; its original inference container was absent. Its source describes rearming after reboot. It was not reset, stopped, disabled or repointed. Before accepting unattended operation, resolve lifecycle/resource conflicts between that workload and Local Studio-managed Avesra models.
- Existing `comfyui-gguf.service` on port 8189 was preserved. Initial low memory use does not guarantee headroom while an image job runs.
- Local Studio publishes inference ports through Docker; do not assume loopback-only exposure. Existing inference authentication is retained. Reboot recovery, service concurrency, TLS access from Avesra and portable runtime provenance remain preflight work.
- No live model completion, tool-selection, image-grounding, microphone, voice identity, speech playback, client-GPU inference or complete Avesra workflow has been proven by the initial setup record. Download progress must not be reported as a ready assistant.

## Swapping and measurement contract

Plan 001 now requires Spark-only, Accelerated and Gaming profiles. Local Studio remains the supported model lifecycle owner. Eligible client lanes may run on the 5090 when actual end-to-end measurements justify placement and sufficient GPU headroom exists. Gaming must free client inference allocations and preserve task/voice state through a safe transition.

Every pipeline stage must expose duration, queue time, outcome and resource metrics, correlated across the client/controller/drivers/tools. The Performance view reports distributions and errors, not only average token throughput. Benchmark reports compare model/runtime/profile revisions against the same accuracy and real-workflow gates. These are implementation requirements, not features already installed by model provisioning.

References: [Local Studio docs](https://localstudio.ai/docs), [installed project's upstream](https://github.com/sybil-solutions/local-studio), [Qwen3.6 FP8](https://huggingface.co/Qwen/Qwen3.6-35B-A3B-FP8), [Qwen3.8 FP8](https://huggingface.co/Qwen/Qwen3.8-27B-FP8), [Nemotron streaming speech](https://huggingface.co/nvidia/nemotron-speech-streaming-en-0.6b), [Qwen3-TTS](https://github.com/QwenLM/Qwen3-TTS), [SpeechBrain ECAPA](https://huggingface.co/speechbrain/spkrec-ecapa-voxceleb).
