# Security boundaries

No listener may accept unauthenticated LAN commands. Pairing must use one-time codes plus owner verification of the TLS fingerprint, revocable per-device credentials and fresh sessions. A responding health endpoint is not paired or authorized. Until transport enrollment is implemented, server actions remain disabled.

Raw media is transient; no raw microphone or screenshot persistence, request-body logs, or automatic crash attachments. Speaker profiles are sensitive, versioned and locally protected. Unknown speaker transcripts must not enter durable memory. Current code does not collect media.

External content is data. Webpages, messages, model completions, memories and demonstrations cannot mint grants, approve effects or mark success. Native/browser executor identities and observation versions must be rechecked immediately before input. No shell, arbitrary JS, cookie export or UAC automation is exposed.

Settings writes cannot change ownership/grants. High-impact approval and owner-management require authenticated local UI. Current unconfigured UI cannot grant either. Errors are categorized without echoing secret-bearing request bodies.
