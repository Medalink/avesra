# Exact-container reasoning adapter

Actual controlled-load, warmup and production-budget greeting evidence is recorded
in [the 2026-09-26 recovery report](../../docs/evidence/reasoning-recovery-2026-09-26.md).
The observed cold setup required a separate44.952second warmup; the subsequent
normal30second helper completed its greeting in3.327seconds. Perform the distinct
bounded operator warmup before offering a fresh cold instance to normal requests;
never turn a timed-out request into an automatic retry or relax its deadline.

## Separate bounded text-only instance

`Dockerfile.owned` and `bounded_start.py` define a separate image derived from the
inspected immutable vLLM image. It starts no model until that exact container has
cgroup-v2 memory capped at at most36GiB, swap disabled, CPU quota at at most2 cores,
and at least38GiB host MemAvailable. It waits at most60seconds, then refuses if
limits/headroom are absent. The wait uses Python standard-library metadata only;
it imports neither torch nor vLLM. This allows Local Studio's existing named
instance launch to own lifecycle while its exact newly returned container receives
resource limits before any model allocation. Do not change limits on another
instance or stop the existing Local Studio engine.

The fixed owned instance serves the pinned Qwen3.8-27B-FP8 artifact
`017b9c7af6b5689d5dd426a76e0bc077eb5ca20a`, using4096 context, one sequence,
512 batched tokens, explicit768MiB KV budget, eager execution and language-model-only.
The retained initialization failure on2026-09-25 measured a0.57GiB minimum KV
allocation for this4096-token context; the earlier256MiB recipe could load weights
but could not initialize the engine. The correction changes only the fixed KV
allocation inside the unchanged36GiB/zero-swap/two-CPU admission limits. It needs
a new immutable guard image and image-bound capture; previous captures remain
historical evidence and are not silently rebound.
Its28.75GiB cached weight package alone is not a complete memory estimate. Actual
load success, unchanged resource limits, reviewed image/artifact capture and model
incarnation must be observed before qualification. This recipe does not provide
vision capability. The actually built derived image is
`sha256:d7c6827ade3234f66b365a492ff81e927ed64fc1c73f180aaefab69ebf8c2dd0`;
the guard SHA-256 is`209e4905b1176aab52ca35cb2b8af04e8f053628c03e1090b2645aa6f8316d19`.
Use `engine.profile: "owned_27b"`, that image, artifact revision above, and
`expected.recipe`/`expected.model: "avesra-owned-27b"`, port8000. The configured
instance name selects only the separately owned Local Studio record. The original
configuration below defaults to `legacy_35b` when profile is absent; no existing
deployment is silently changed. The profile's exact descriptor is in`profiles.py`.

Local Studio records the created container before awaiting health; its launch
HTTP call does not return until healthy. Keep that call owned while inspecting
`/compute/instances` for the exact new name and Docker reference. Verify the
container ID and Local Studio name/nonce labels, then set only that container's
limits while its startup guard waits. Do not wait for the launch response before
setting limits. A failed/missing resource gate exits that new instance and must
not trigger a retry or changes to unrelated workloads. Build success does not
prove loaded weights, request completion or calibrated classifier quality.

The separate instance's private configuration uses these exact fields (the
credential file stays private and is read by the adapter):

```json
{
  "version": 2,
  "controller": "http://127.0.0.1:8080/",
  "expected": {"instance":"avesra-reasoning","recipe":"avesra-owned-27b","model":"avesra-owned-27b","port":8000},
  "artifact_revision":"017b9c7af6b5689d5dd426a76e0bc077eb5ca20a",
  "credential_file":"/home/medalink/.config/local-studio/controller.env",
  "engine": {
    "profile":"owned_27b",
    "image":"sha256:d7c6827ade3234f66b365a492ff81e927ed64fc1c73f180aaefab69ebf8c2dd0",
    "model_directory":"/home/medalink/.cache/huggingface/avesra/Qwen3.8-27B-FP8"
  }
}
```

After exclusive artifact capture, the authenticated Local Studio
`POST /compute/launch` request uses the following fixed body. Port8000 must be
free; a different automatically selected port will be refused by this profile.
Keep the launch request owned while reading the exact new instance reference,
then apply `docker update --memory 36g --memory-swap 36g --cpus 2` to that verified
new container ID within the guard's60second startup window. This is an explicit
operator procedure; the runtime reasoning adapter never performs lifecycle work.

```json
{
  "name":"avesra-reasoning",
  "engine":"vllm",
  "modelPath":"/home/medalink/.cache/huggingface/avesra/Qwen3.8-27B-FP8",
  "recipeId":"avesra-owned-27b",
  "deviceCount":1,
  "servedModelName":"avesra-owned-27b",
  "dockerImage":"sha256:d7c6827ade3234f66b365a492ff81e927ed64fc1c73f180aaefab69ebf8c2dd0",
  "options":{"maxContextLength":4096,"memoryFraction":0.25,"maxConcurrentRequests":1,"reasoningParser":"qwen3"},
  "extraArgs":["--max-num-batched-tokens","512","--kv-cache-memory-bytes","805306368","--enforce-eager","--language-model-only"]
}
```

Inspect exact container command/entrypoint, labels, limits, read-only mount and
Local Studio record against the profile. A successful launch is still separate
from a successful adapter inspection and real terminal-bound inference.

These fixed helpers are bundled into the Rust server; no separately installed
Python service is started. They use standard-library Python and the existing
Docker/Local Studio owner. They never stop, start, swap or download a model.

Put `reasoning.json` in the existing private Avesra server directory, mode0600.
The example names the existing instance; a separately managed instance name may
be used when Local Studio has created that instance with the same reviewed recipe
and serving arguments. Merely editing a name cannot qualify an existing load.

```json
{
  "version": 2,
  "controller": "http://127.0.0.1:8080/",
  "expected": {"instance":"llm","recipe":"avesra-fast","model":"avesra-fast","port":8888},
  "artifact_revision":"95a723d08a9490559dae23d0cff1d9466213d989",
  "credential_file":"/home/medalink/.config/local-studio/controller.env",
  "engine": {
    "image":"sha256:d464f3b466fa9c45ddbff8a812e80564503b6879a9fd95c1a47514f3f0df5a4a",
    "model_directory":"/home/medalink/.cache/huggingface/avesra/Qwen3.6-35B-A3B-FP8"
  }
}
```

The existing credential file supplies both `LOCAL_STUDIO_API_KEY` and
`INFERENCE_API_KEY`. Neither value belongs in configuration examples, output or
arguments. Runtime helpers receive credentials through bounded stdin only.

Before a selected instance has started, run the explicit producer:

```sh
python3 services/reasoning/capture.py /private/avesra/reasoning.json
```

The selected container must be absent/stopped. The producer verifies every model
package file against immutable upstream metadata and checks the bytes remain
unchanged. It creates `reasoning-artifact.json` exclusively, mode0600, in the
configuration directory. It does not overwrite earlier evidence. Large artifact
hashing is an operator setup operation, outside the interactive request deadline.
Local Studio must subsequently create/start the selected engine. Its creation
and start must both follow the capture on the same boot, and all captured file
identities must remain unchanged. A stopped container restarted in place does
not qualify. Do not stop unrelated work to satisfy this procedure.

Only the reviewed immutable image, exact vLLM arguments, read-only package mount,
unchanged reviewed serving sources, live API/EngineCore process identities,
live model metadata and exact loaded tokenizer budget may qualify. Runtime
inspection does not perform generation. Generation uses the same container and
process identity, no custom stops, one choice and a512-token output reserve.
The owned stream must reach its valid terminal and EOF before its actual durable
job can retire. Failure leaves uncertainty blocking new work.

Configuration and a successful build are not live proof. The current existing
engine predates this capture process and cannot be retroactively qualified.
Controlled-load qualification, accepted-request generation, cancellation/drain
behavior, response latency and downstream speech remain runtime gates. No helper
or model was activated by adding these source files.
