# Transient reasoner-backed directedness observations

The private reasoning driver also supports a separate pre-acceptance classifier.
It never constructs a planner claim, accepted turn, conversation history, grant or
effect. Native captured/ASR text alone supplies the bounded transient input; no
WebView command accepts an arbitrary transcript. A paired authenticated request
binds exact request/utterance, device/session, capture/action epochs, microphone
and optional native actor/grant context. The server checks the live capture/action
context and device activity at admission and publication. Context mismatch,
withdrawal, missing qualification or uncertain inference fails closed.

The model returns only `request`, `follow_up`, `rejected` or `unknown`, with no
confidence, approval or reasoning prose. It must abstain when text alone cannot
distinguish speech addressed to the assistant from another person, quoted speech
or contextual fragments. This component cannot detect physical speaker identity,
audio replay or guarantee directedness. A FollowUp still requires the native
gate's separately maintained same-context follow-up window. Real calibration and
held-out trials of the complete gate must qualify these observations before any
automatic acceptance; matching a model/category label is insufficient.

The classifier shares the planner driver's single actual model owner, original
30-second budget, exact artifact/process/tokenizer admission, non-retried send,
terminal-plus-EOF drain and durable uncertainty. It uses its own fixed prompt and
typed result parser, not a fabricated accepted planner request. Caller loss stops
publication while actual work retains its permit/drain ownership. No input text,
model reasoning or categories are stored in the model-job ledger.

Read-only binding inspection uses the same controlled-load observer and exact
tokenizer without generation or a transcript probe. The reported adapter revision
is SHA-256 of the fixed classifier policy revision, observed artifact revision and
actual engine incarnation. Every actual classification re-observes those live
bindings. A process restart always requires fresh controlled-load verification;
neither saved evidence nor a fingerprint bypasses it.

Wire version2 additionally reports an optional reusable quality fingerprint. It
hashes the actual immutable image, entrypoint/arguments and capacity, normalized
Docker environment (only the reviewed pure-auth `VLLM_API_KEY` value is excluded),
the complete captured model-package filename/content-SHA256 roster, reviewed
serving-source hashes, and actual GPU UUID/name/compute capability, driver version,
loaded CUDA runtime content hash and installed CUDA/PyTorch/vLLM versions. It also
hashes the classifier policy and complete canonical classifier request body,
replacing only the transcript with a fixed placeholder. No transcript, credential,
environment value or hardware identifier is exposed in the wire fingerprint.
Container IDs, process IDs, capture timestamps/inodes and authentication-key
rotation do not change that quality algorithm identity.

Missing or invalid GPU/runtime metadata yields no reusable fingerprint; existing
strict incarnation binding remains required. Only equality of complete nonempty
fingerprints may allow protected durable calibration evidence to be rebound to a
freshly observed controlled incarnation. Artifact revision alone cannot authorize
reuse. Per-request revision and current context still bind the live observation;
fingerprints grant no acceptance, permission, model-job retirement or readiness.
Metadata availability is neither successful inference nor qualification.

Entry points: paired POST `/voice-directedness` for typed `inspect` or `classify`,
native `connection::directedness` helpers only, and the private reasoning driver.
There is no action/accepted-input path from the route, no saved transcript, no
automatic model load and no downgrade. Native calibration and live capture own
their exact current-context callback and consume the observed revision/category.
