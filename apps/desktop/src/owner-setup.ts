/** Presentation only. Native owner/registration/voice authority stays in Rust. */
export const automaticVoiceLimitation = "Automatic owner recognition is still being built. This version cannot listen or respond automatically, even after the setup steps are complete.";
export type RegistrationState = "waiting" | "loading" | "error" | "unregistered" | "different_owner" | "revoked" | "registered" | "unreconciled";
export type RegistrationView = { state: RegistrationState; detail: string };
