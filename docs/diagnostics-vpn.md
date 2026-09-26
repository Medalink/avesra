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
DNS, endpoint latency, per-app throughput/throttling and download destination are
not inferred from these observations and remain separate catalog work.

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

The required future catalog scope includes current adapter/link properties, cumulative network
byte counters with a second host-local monotonic sample, CPU/disk pressure and
free space, selected VPN state, and bounded DNS/reachability to the particular
endpoint implicated by the accepted request. It does not scan unrelated hosts,
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
