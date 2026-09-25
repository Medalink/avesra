# Operations

Avesra is under implementation. See [current M0 evidence](evidence/m0-preflight.md). No installed assistant, owner enrollment or live workflow is claimed.

The only initial server is owner-confirmed SSH alias `spark2`. Preserve Local Studio's authentication and unrelated ComfyUI/Qwen supervisor workloads. Avesra must own only its own controller and narrowly scoped speech services. Never reset shared model metrics.

Secrets belong in owner-protected OS configuration or credential storage, referenced by name. Do not commit them, copy browser cookies, print environment dumps, or include media/biometrics in diagnostics.

Before enabling observation or voice, select actual devices/scopes and complete capability probes and owner enrollment. Missing services remain unavailable. Browser accounts and Claude projects require explicit selection. Authentication/MFA stays with the owner.

The baseline `single-spark` and `gaming` profiles prohibit client model allocation. `accelerated` is opt-in and requires measured device eligibility. A disconnected controller never grants permission to execute delayed actions.

Saved-pairing recovery: Profiles > Manage saved pairing shows the public device UUID, plus a warning that local removal is not server revocation. Revoke through the existing authenticated Spark CLI (`avesra-server revoke <private-directory> <device-uuid>`), then explicitly remove the PC copy. Local removal disconnects and invalidates pending connection work, serializes with pairing/reconnect, and binds removal to the reviewed encrypted file revision. If DPAPI cannot decrypt the record, the UI reports the missing device identity; do not guess a server device UUID. A pairing save failure reports the newly issued public UUID for revocation before retry. The server and PC never automatically replace existing credentials or certificates.
