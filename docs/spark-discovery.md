# Discover and connect a Spark

The default companion pairing flow is Find my Spark, a bounded local-network discovery request followed by explicit selection of a discovered server. The address, public certificate and fingerprint are filled by native discovery, not copied by the user. Advanced retains the existing manually verified certificate and one-time-code flow.

## Discovery v1

The configured Avesra server listens on UDP 9476 and responds to a versioned request with a fresh UUID nonce. Replies repeat that nonce and contain only product, discovery/control versions, a bounded display name, the public certificate and whether easy pairing is open. The client derives the HTTPS address from the actual IPv4 sender and the fixed control port 9474; the reply cannot redirect it to another host, path or port. No code, private key, credential, account data or model content is advertised.

The client sends a broadcast and probes explicitly supplied local host hints (initially the configured Spark2 address), never sweeps a subnet. A scan lasts three seconds, accepts at most 16 distinct endpoint/certificate candidates and 128 replies, and retains native candidates for 60 seconds. Only native cached candidate IDs may start easy pairing. Stale scans, new scans and withdrawn settings panels invalidate selection. Unsupported discovery/control versions and malformed, oversized or nonlocal replies are ignored. Discovery itself does not establish trusted identity or readiness.

## Easy pairing and trust

On a new server with no previously issued device credentials, default serve opens a five-minute, one-device easy-pairing window. `serve <directory> --pairing` explicitly opens that window for an existing server; `--manual-pairing` keeps it closed even on a new installation. A monotonic deadline and a mutex serialize consumption. Restarting an already paired server never reopens it automatically. A successful advanced-code pairing also closes the easy window. Failure after credential issuance is never automatically retried. Advanced pairing codes are not replaced or exposed by easy pairing.

The device card shows the actual network address and offers Use this Spark only when its advertisement reports an open pairing window. Selecting it is trust on first use of that advertised certificate, suitable for a trusted local network; the UI says to choose a recognized device. Manual independent fingerprint verification remains available under Advanced. The native client validates the certificate and hostname/IP through normal TLS and pins that certificate in the existing protected pairing record. It never disables TLS verification, overwrites an existing saved pairing, silently trusts a replacement certificate or sends credentials over discovery. Proxies and redirects are disabled for discovery pairing. Certificates must cover the discovery sender IP.

`POST /pair-local` accepts an empty JSON object over TLS and issues one credential only while the easy window is open. There is no unauthenticated way to reopen it. The existing server auth store, protected Windows persistence and authenticated control connection are reused. Pairing still grants no owner identity, microphone activation or action authority. Service readiness remains separate from connection status.

## Entry points and verification boundary

| Entry point | Responsibility |
| --- | --- |
| Server serve / discovery responder | Public metadata, bounded replies, first-install or explicit timed pairing window |
| `/pair-local`, existing `/pair` | Serialize device issuance and close easy pairing on success |
| Desktop discovery commands | Bounded scan, native candidate retention and stale/withdrawn rejection |
| Existing TLS pairing and persistence | Certificate validation, exact endpoint, bounded credential reply, no overwrite |
| Pairing UI | Scan/results/empty/error states, explicit candidate selection, Advanced fallback |

No automated tests or fixtures under the owner override. Use static/build checks and direct native discovery observations; a found device is not evidence of a working assistant. Changing source does not deploy the updated server to Spark.
