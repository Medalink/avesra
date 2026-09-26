# Bounded diagnostics and VPN observations

## Configured connection contract

The native Settings owner may inspect only the saved Cisco default host name and
address from the bounded global preferences document. Discovery is read-only and
does not grant an action. The visible owner confirms the exact observed host with
Windows verification; a changed selected field invalidates that selection. No
username, password, group, certificate decision or MFA response is imported.
The inspected document is transient and zeroed; only the selected name/address
and their canonical digest persist. Unrelated Cisco preference changes do not
silently change the selected target or unnecessarily invalidate its identity.

The closed accepted request `connect work vpn` selects the one current saved
target. Plan 001 section 3 treats this existing configured connection as a routine
within its explicit grant: no additional per-connection Windows verification is
required. The prior blanket `ConnectVpn.needs_approval` flag conflicted with that
rule and is removed only for this routine operation. Profile/grant management
remains protected, MFA remains with Cisco, and configuration changes still require
exact fresh approval. Dispatch binds the original task, step, target, payload,
action/intent revision, actor, session and deadline. Cancellation, expiry, changed
target or revoked permission prevents dispatch.

Before first write the worker checks signed executable identity, current selected
fields, current authority and actual Cisco state. An already connected different
VPN is left intact. This installed surface has a numeric saved peer with explicit
default TLS port 443; both bare numeric addresses and numeric socket addresses
with port 443 normalize to that same peer. Hostnames, non-default ports and group
URLs remain unsupported until their routing/identity contracts are implemented.
The CLI receives the canonical numeric address as one argument,
with closed stdin, no shell, bounded output, original action budget and no retry.

After the first connect command, killing/reaping vpncli is not proof that the VPN
agent cancelled connection. Authentication, MFA, banners/certificate decisions,
timeout and routing loss remain needs-owner/uncertain observations. Avesra never
answers prompts, records their content, disconnects, changes policy or retries a
possibly issued connect automatically. Fresh actual VPN state is required before
any later explicitly requested attempt. Final connected success requires the
observed server address to match the selected numeric target. Paired Spark
reachability is reported independently; VPN connection
does not prove the control channel survived.

Entry points: native saved-host inspection and protected selection; exact durable
accepted resolver; original action and permission projection; existing
effect worker; fixed Cisco stats/connect; task result projection.
Model-generated scripts, arbitrary host input, credential forwarding, disconnect
and policy changes are outside these entry points. The owner's no-tests override
applies: source/static/build checks precede separately authorized direct proof.

Plan 001 sections 9/12 and C7 govern this adapter. Diagnostics do not accept model
command strings. Supported operations form a fixed native catalog with typed
arguments, fixed output shapes, explicit units, size limits and original deadlines.
An accepted task, current actor/target grants and the ordinary execution ledger
remain prerequisites. Setup observations cannot create an accepted task.

## Shipped source entry points

The existing protected Action permissions panel exposes separate fixed-catalog
read grants. `grant_diagnostic_action` accepts only the two catalog enum choices;
the normal visible/current owner panel and consumed Windows verification apply.
Catalog identity supplies its immutable native target ID; neither model nor UI
supplies arbitrary command text, executable paths, hostnames or authority IDs.
The exact whole accepted requests `check computer performance` and `check vpn
status` (optionally addressed to Avesra) link through the existing durable-turn
resolver and explicit grant. No arbitrary transcript command or setup-run bypass
is added. Unknown text remains on the existing planner/clarification path.

The ordinary effect worker owns the full diagnostic, invoking the existing exact
source/grant/session/deadline callback before observation and during waits. It
stores a bounded typed Diagnostic observation with the original task/action
finalization; no raw process output is retained. Successful diagnostic completion
means the report was collected, including explicit unavailable fields, not that
every counter is available or a network problem was fixed. Actor-filtered recent
tasks project that original report, and the existing panel displays measurements,
units, sample interval and limitations. Cancellation remains independently usable
while a diagnostic blocks the status reader.

Ledger schema 12 advances the compatibility marker for Diagnostic permission and
observation variants, validating the existing layouts without changing tables or
rewriting history. Older readers reject it. Before deployment, use an actual
SQLite backup of each existing ledger; an old executable cannot be used as a
schema downgrade. Source/static checks are separate from actual accepted-request
diagnostic proof and full slow-download explanations.

## Read-only catalog

The first implemented native entries are fixed `HostResources` and
`CiscoVpnStatus`. Host resources samples CPU times and interface byte counters
twice across a measured one-second interval, with a five-second total budget and
at most 64 interfaces. CPU uses the calling primary processor group's
GetSystemTimes scope. Network deltas are per interface, not a sum that double
counts virtual/VPN traffic. Added/changed/reset counters are unavailable. Disk
space is explicitly the Windows system drive's caller-visible quota, not the
download destination or disk utilization. Disk pressure remains unsupported.
DNS configuration and default-route metadata are read locally. The selected-endpoint
path below adds bounded DNS/TCP observations and optional fixed-disk destination
space. Per-app throughput/throttling and disk utilization remain unavailable;
aggregate counters do not establish those facts.

The Cisco reader pins the independently observed Authenticode-valid Cisco Systems
binary, version `5, 1, 20, 333`, 145,968 bytes, SHA-256
`567FF2854D62B1AC24B661774486917201D2112520F30A3A9B33AAA224E1C4E0`.
This provenance was read without executing the binary. A held file handle denies
write/delete during hashing and the child lifetime; another build is unsupported
until reviewed. The only argument is `stats`, with no shell or inherited stdin,
hidden console, fixed working directory, 16 KiB each stdout/stderr and five-second
original budget. Readers drain concurrently and remain joined through actual exit.
Failed termination retains worker ownership until the owned process is reaped.
The five-/thirty-second operation budgets trigger cancellation; they cannot
guarantee retirement if Windows itself cannot kill/reap the process. In that
exceptional case the existing effect worker remains unavailable rather than
detaching or admitting a replacement. Pipe readers use bounded nonblocking
availability polling: inherited writer handles after actual child exit produce
unavailable output, never an invented EOF or an indefinite reader join.
Raw output is transient; only the final recognized state escapes. Any error line,
stderr, nonzero exit, invalid encoding, unknown state or incomplete/oversized
output is unavailable. Connected additionally requires the documented tunnel
information section and one numeric server address. This catalog entry only reads
stats. The separate granted routine uses connect as specified above; disconnect
and automated authentication remain absent.

The native source uses Microsoft's documented [interface table ownership](https://learn.microsoft.com/en-us/windows/win32/api/netioapi/nf-netioapi-getiftable2),
[CPU timing scope](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getsystemtimes)
and [caller-visible disk space](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getdiskfreespaceexw).

The implemented catalog includes current adapter/link properties, cumulative
network byte counters with a second host-local monotonic sample, CPU usage/free
space, selected VPN state, and bounded DNS/TCP observations to the specifically
selected endpoint. Disk utilization remains unavailable. It does not scan unrelated hosts,
download speed-test data, reset networking or invoke model-authored scripts.
Where a required Windows counter is inaccessible or resets between samples,
report unavailable with its fixed reason; never substitute zero.

Counter deltas use the measured local interval. Bytes per second, MB/s (decimal)
and Mbps remain distinct: Mbps = bytes/second * 8 / 1,000,000. Report the exact
interval and counter scope. Aggregate adapter traffic cannot establish one app's
download rate. App throttling, source-server speed and wireless quality remain
hypotheses unless actually observed by an authorized adapter. Active significant
downloads require a separately agreed measurement.

Fixed external executables, where necessary, run without a shell and without a
console window. Validate their installed identity/version before execution,
close unused stdin, drain bounded stdout/stderr concurrently, cap their original
duration and retain the actual process handle through exit/cancellation. Overflow,
deadline, partial output or unsupported format yields unavailable. Kill/reap only
the process owned by that invocation; do not enumerate/terminate unrelated apps.
Do not write raw diagnostic output into telemetry or ordinary logs.

## Observed Cisco surface

Read-only inventory on 2026-09-25 found Cisco Secure Client AnyConnect VPN
`5.1.20.333`, with `vpncli.exe` at
`C:\\Program Files (x86)\\Cisco\\Cisco Secure Client\\vpncli.exe` and matching
file version `5, 1, 20, 333`. Tailscale `1.102.3` is also installed; its presence
does not establish that it is the work VPN requested by the owner.

Cisco documents the fixed CLI `stats` read and separate connection/disconnection
commands in its [Secure Client 5.1 CLI reference](https://www.cisco.com/c/en/us/td/docs/security/vpn_client/anyconnect/Cisco-Secure-Client-5/admin/guide/cisco-secure-client-admin-guide-new/customize-secure-client-intro/r_use_the_ac_cli_commands.html).
This provides an observed product surface to implement, not permission to connect
to an arbitrary endpoint. An actual hidden `stats` invocation with closed stdin
and a five-second deadline exited 0 without stderr. Its 1,867 stdout bytes were
held transiently; parsed state notifications were Unknown, Disconnected,
Disconnected, with no tunnel-information section. Raw output was not retained.
No connect/disconnect command or credential interaction occurred.

The product reader must distinguish unknown, disconnected, connecting, connected,
reconnecting and unavailable. Require the complete command to exit successfully;
an earlier connected line followed by an error is not proof of final connection.
Unsupported/localized text remains unavailable until its parser is reviewed.
Keep profile/server addresses and other sensitive statistics out of telemetry.
After a requested connection, observe both final VPN state and independently
bounded reachability of the already paired Spark; neither implies the other.

## Connection and changes

The owner selects an existing VPN profile/control surface. A connection routine
uses ordinary typed steps bound to that exact target, never imports credentials
or edits corporate policy. Authentication, MFA, certificate warnings and banners
requiring a human decision produce needs-owner evidence; Avesra must not answer
them or persist their contents. A postcommit incomplete connection remains an
unknown effect in the ledger even when its report identifies an owner step.
This implementation does not open interactive authentication surfaces on behalf
of the owner. The owner continues in Cisco directly.

The native accepted coordinator makes one independent three-second, certificate-
pinned authenticated Status request to its already paired Spark after the VPN
worker returns. The exact actor registration must match. A failed check means
unavailable, not proof that VPN policy caused routing loss. The channel observation
is owner/session scoped, displayed with its actual timestamp for up to one minute,
and is separate from the durable Cisco result. No reconnect or connection replay
occurs on failure. Reopening Settings can show this observation after connectivity
returns; original uncertain action history remains durable.

Any configuration-changing proposal states exact target/parameters, expected
effect, disruption and rollback. Its fresh approval is bound to the actual task,
step, action revision, intent revision and payload through existing policy. An
edit, expiry, revocation or changed session invalidates approval before dispatch.
No blanket diagnostic grant approves writes, UAC, protection changes, network
reset, reboot or VPN-policy changes. Routing loss retains original task/effect
uncertainty and does not retry a possibly completed connection or change.

## Verification boundary

The inventory and direct `stats` observation establish a usable installed read
surface only. The native saved-target/grant/connect source path now exists, but
its successful real connection/MFA remains unexecuted. Grounded slow-download diagnosis, exact
approval handling, successful VPN connection/MFA/routing-loss recovery and A18/A19
remain separately required. Use source review, static/build checks and direct
product observations under the owner's no-tests/no-harness instruction.

Read-only inventory repeated on 2026-09-26 confirmed the same size/version/hash
and valid signature. The 746-byte global preferences document has one nonempty
default name and one numeric IPv4 peer with explicit port 443. No actual values
or other preference fields were printed or retained. This inspection did not
invoke connect, authentication, routing changes or credential operations.


## Observed already-satisfied result (schema 20)

An exact granted VPN request whose fresh native stats already show the selected
peer connected completes with `already_satisfied`, without issuing a connect,
consuming a write boundary, or asking for input. It is distinct from a successful
connection change. The original current authority, cancellation and deadline are
checked again inside finalization's transaction. Only the exact validated VPN
observation can establish this result; a model, generic adapter result, missing
observation or reconciliation cannot create it. Task state is succeeded, while
its immutable outcome and event retain the no-change distinction. It does not
become verified-action memory or an action-completion notification. Legacy rows
retain their original outcomes; schema 20 prevents older readers opening the new
variant. Package schema declarations must match the actual binary.

## Bounded slow-download extension contract

The existing granted Computer performance scope supports `diagnose download`
without additional setup. Native local counters, default routes and IPv4 DNS
configuration are observed automatically. Only selected-endpoint probes need an
actual endpoint; application, displayed rate/unit, throttle and destination drive
are optional owner-reported context. These are reported
context, not measured throughput or diagnosis. Native observations bind the exact
context revision and accepted task. Only the selected endpoint is queried; no
unrelated DNS cache names, browsing history, credentials or request URLs are
collected. Interface deltas remain interface traffic, never per-app speed.
Endpoint DNS, route and connection evidence report measured results or fixed
unavailable reasons separately from hypotheses. Windows counters cannot prove a
source server or app throttle caused slow throughput.

The only proposed configuration action in this slice is flushing the Windows DNS
resolver cache after a fresh measured DNS failure for that exact endpoint. DNS
failure does not establish that the cache is stale and a flush is not promised to
fix it. A proposal discloses the system-wide cache scope and expected temporary
DNS lookup cost. A fresh exact owner approval binds original diagnostic evidence,
action/intent revision, actor, session, endpoint/context and fixed payload before
any write. Changed, expired or cancelled evidence/approval cannot execute. There
is no arbitrary command, DNS-server change, network reset, proxy or VPN-policy
mutation, automated elevation or retry. Actual flush and any postcondition are
separate observations; no call is made during source/static development.

Entry-point inventory: the existing native Settings diagnostic-context setup,
accepted-turn resolver and effect worker are the intended observation path;
protected exact approval is the only configuration path. Model proposals and
frontend observations cannot supply measured evidence. Existing host/VPN read
catalog entries remain unchanged. Source progress must not be described as a
completed download diagnosis or approved fix until those entry points are wired
and their actual result is observed. No automated tests are created under the
owner's explicit override; independent source review and build checks apply.


### Concrete download entry points and current evidence boundary

Optional Settings endpoint scope stores one selected endpoint (ASCII hostname
or numeric address) and port80/443. Application label, destination drive and
reported rate/unit/throttle are optional and remain unavailable when omitted. The protected read grant is distinct from the
optional protected DNS-cache proposal scope. The exact whole requests are
`diagnose download` and, after a measured same-context DNS failure,
`clear download dns cache`. Fixed VPN, volume, app-open and diagnostic commands accept
terminal `.`, `!` or `?` sentence punctuation. No embedded clauses are removed;
quoted prompt text and browser URLs are not normalized by this rule. Only the
whole app-open request gets terminal sentence punctuation removed; embedded
punctuation/clauses remain part of the exact alias and cannot select another app.

The retained worker performs selected A and AAAA queries using Microsoft's
[asynchronous DNS API](https://learn.microsoft.com/en-us/windows/win32/api/windns/nf-windns-dnsqueryex),
keeping query/cancellation buffers until the actual completion callback after a
cancellation request. There is a five-second DNS budget within a ten-second
observation budget. A DNS-stage timeout is reported explicitly while prior
resource observations are retained if actual action authority is still current.
TCP stage failures/timeouts similarly remain unavailable measurements. Neither
can enable a flush. If Windows cannot complete cancellation, the actual worker
remains occupied; buffers are not freed or a replacement admitted. At most four
returned non-loopback endpoints receive route lookup and a500ms TCP connection
attempt with no application payload. TCP timing does not establish TLS, source
throughput or a cause of slowness. Interface rates and primary-group CPU scope
retain the existing one-second measured interval; disk utilization remains
explicitly unsupported and destination disk quota is reported separately.

A DNS-cache proposal requires both address-family queries to return an actual
negative/server-failure/refusal/no-record result, no resolved endpoints, and a
successful diagnostic finalization no older than120seconds in the same actor,
device, session and action epoch. Unavailable/cancelled queries cannot propose a
flush. Revoking/changing the diagnosis scope invalidates the evidence. The later
accepted request creates AwaitingApproval with the fixed payload and evidence
identity; the original native coordinator waits while Settings displays the
exact revision. Approval does not renew either the action deadline or the
conservative30second accepted-coordinator lifetime. Caller loss, task cancel,
lock/disconnect or expiry prevents dispatch; stale pending approvals cannot be
recovered from history. No approval is inferred from the spoken request alone.

The fixed write is the documented Microsoft
[`ipconfig /flushdns`](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/ipconfig).
The installed binary was inspected read-only on2026-09-26: version10.0.26100.1,
61440bytes, Authenticode Valid/Microsoft Windows, SHA256
`8a013c65ff778cf8341cd0b3404f6e323c5ff91b5059ac47f3bfe2cd6208f6a1`.
A read handle denies replacement through actual child retirement. No shell,
stdin, output capture, elevation, credentials or extra arguments are used.
The original authority is checked before its single commit boundary; timeout
or loss after that boundary remains an unknown effect. Exit-zero records command
completion only, never a repaired download or successful subsequent lookup.
An OS failure to kill/reap retains the worker rather than claiming retirement.

No DNS query, connection attempt, cache flush or VPN mutation was performed as
part of this source implementation. Native accepted diagnosis/approval and its
negative authority cases still require direct owner-authorized runtime proof.


Local configuration additions use bounded GetIpForwardTable2 default routes and
GetNetworkParams IPv4 DNS-server count; host/domain/scope strings in that API
buffer are not retained or displayed. Route metrics are not combined interface
metrics, and IPv4 DNS configuration is not a claim about IPv6 resolvers. Missing
endpoint context falls back to the current HostResources grant; it never fabricates
an endpoint or runs a probe. Missing app/rate/throttle/drive does not block this
local diagnosis. The optional selected-endpoint form is not an activation step.

Optional destination-space observations are limited to a Windows fixed local disk;
removable, mapped-network and unknown drive types are explicitly unsupported.
An internal route/TCP stage timeout preserves collected local facts and any route
already observed, provided the original action authority remains current.

## Grounded spoken diagnostic result

A successful native resolution of a C7 accepted task may issue one opaque
observation-reply continuation. Reading historical task rows cannot create it.
The continuation retains the original task, action revision, native accepted
source and monotonic budget; the native coordinator binds the current actor
registration and original withdrawal signal before dispatch. After actual
finalization, the same Store worker derives a bounded spoken explanation from
that exact immutable finalization and typed observation, inside the reply
transaction. No frontend text, model response or copied report is accepted.

The reply receives NativeObservation provenance (dispatch and action revision),
a fresh durable monotonic reply ordinal, and the original accepted-turn context.
It is published at most once via the existing normal reply owner and streamed
TTS path. It cannot revive an expired or cancelled action, renew its lifetime,
change its outcome or become another effect. Failures and unknown outcomes are
spoken only if the original live owner still allows output; otherwise their
existing task report remains the durable result. Historical observation replies
are not retrieved as current diagnostic evidence in later model prompts.

Host-only download diagnosis summarizes CPU and per-interface observations,
local DNS/default-route presence and system-drive free space. It explicitly says
that the selected download endpoint, app rate, throttle and destination are
unknown and asks for the endpoint only when endpoint-specific evidence would
help. A selected-endpoint report distinguishes DNS resolution, transport timing,
and local counters: none alone establishes download throughput or stale cache.
A completed flush is described solely as command completion, with remeasurement
needed. VPN output distinguishes already connected, newly observed target match,
owner authentication, other connection and unresolved state; it never claims
Spark reachability from VPN state alone.
