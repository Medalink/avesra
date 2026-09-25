# Security boundaries

Local enrollment management uses Windows UserConsentVerifier desktop interop bound to the real Settings HWND. Only an OS `Verified` result may create a short-lived native setup session; availability alone is not authentication. The session binds the connection generation and capture epoch, expires after60seconds, and is invalidated by local controls, device changes or disconnect. It never grants action rights, marks a speaker enrolled, unmutes, or overrides service/quality prerequisites. No caller-supplied window, prompt, authentication result or proof token is accepted. The verifier is invoked only by an explicit Settings action and has not been invoked during implementation while the owner games. Microsoft desktop API contract: https://learn.microsoft.com/en-us/uwp/api/windows.security.credentials.ui.userconsentverifier

No listener may accept unauthenticated LAN commands. Pairing must use one-time codes plus owner verification of the TLS fingerprint, revocable per-device credentials and fresh sessions. A responding health endpoint is not paired or authorized. Until transport enrollment is implemented, server actions remain disabled.

Raw media is transient; no raw microphone or screenshot persistence, request-body logs, or automatic crash attachments. Speaker profiles are sensitive, versioned and locally protected. Unknown speaker transcripts must not enter durable memory. Current code does not collect media.

External content is data. Webpages, messages, model completions, memories and demonstrations cannot mint grants, approve effects or mark success. Native/browser executor identities and observation versions must be rechecked immediately before input. No shell, arbitrary JS, cookie export or UAC automation is exposed.

Settings writes cannot change ownership/grants. High-impact approval and owner-management require authenticated local UI. Current unconfigured UI cannot grant either. Errors are categorized without echoing secret-bearing request bodies.
