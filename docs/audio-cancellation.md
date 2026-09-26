# Warm audio cancellation

Private activity-stream and TTS-stream cancellation first withdraws the exact
request and signals a per-worker shared cancellation event. The parent retains
its active owner while the real child unwinds. Activity checks between bounded
operations; TTS checks its existing codec/current boundaries and always restores
its temporary generation override and hook. No next inference enters meanwhile.

Only the child can acknowledge retirement: it releases stream PCM/features/cache,
synchronizes its actual CUDA work, and sends a terminal carrying the exact private
owner nonce. An idle activity stream receives an explicit reset command under the
same exclusive ownership. Completed operations racing cancellation are drained
and explicitly reset before acknowledging. The model and selected generated voice
remain resident after this acknowledged retirement. No synthetic final activity
scores, successful synthesis terminal, or playback receipt is produced.

The cooperative wait is bounded to two seconds and the original cancel request
lifetime. Missing/malformed retirement, inference failure, reset failure, or
unresponsive actual work retains the existing kill/reap path. Busy remains true
until retirement or process termination. A caller dropping its socket cannot
clear active ownership. Small bounded completed-request tombstones acknowledge a
cancel arriving after an already completed real terminal without unloading.

A cancellation arriving before its first private request is admitted still returns
unknown_request. The controller conservatively retains its permit through the
original remote lifetime (up to 31 seconds); an immediate resume may report busy.
No absence acknowledgement is inferred from a missing request, and this change
does not add pre-admission cancellation tombstones.

Private health advertises `cooperative_reset_or_terminate` only for activity/TTS;
other lanes retain `terminate_process`. Both are finite recognized strategies.
The owned audio installer enforces this same lane-scoped finite strategy set.
Public stream protocols and authority are unchanged. Controller activity/TTS
permits remain held through the existing cancel acknowledgement or complete
original remote lifetime. Graceful cancellation is not completion or qualified
model readiness; loaded state remains loaded_unqualified.

Entry points: Service.dispatch cancel, child request loop, Driver reset,
StreamingActivity push and streaming_tts current; AudioHealth/admin validation;
ActivityStream/TtsStream Drop retain their actual cancellation owner; the owned
audio installer uses the same health strategy validation. Changes are
source-reviewed/compiled only under the owner's no-tests instruction. Actual
mute/resume and interruption/reply reuse require deployed image verification.
