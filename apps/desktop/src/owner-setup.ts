/** Presentation only. Native owner/registration/voice authority stays in Rust. */
import type { Runtime } from "./runtime";

export function personalVoiceView(runtime: Runtime | null) {
  const state = runtime?.personal_voice?.state ?? "unavailable";
  const label = state === "learning" ? "Learning your voice" : state === "listening" ? "Listening" : "Unavailable";
  return {
    label,
    title: state === "learning" ? label : runtime?.voice_ready && state === "listening" ? "Talk to Avesra" : label,
    reason: runtime?.personal_voice?.reason ?? "Waiting for the companion's listening status.",
    active: state !== "unavailable",
  };
}

export type RegistrationState = "waiting" | "loading" | "error" | "unregistered" | "different_owner" | "revoked" | "registered" | "unreconciled";
export type RegistrationView = { state: RegistrationState; detail: string };
