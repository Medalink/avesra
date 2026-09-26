import { command } from "./runtime";

export type SpeakerCandidate = {
  id: string;
  revision: string;
  model_revision: string | null;
  segments: number | null;
  state: string;
  read_error: string | null;
};
export type AvatarParameters = { version: 1; shape: number[]; petals: null[]; ring: string; rotation: number; tempo: null; digest: string };
export type VoiceAvatar = { version: 1; state: "ready_without_portrait" | "unavailable" | "source_unavailable"; parameters: AvatarParameters | null; candidate: { id: string; revision: string } | null; reason: string | null };
export type SpeakerStore = { candidates: SpeakerCandidate[]; storage_directory: string; avatar: VoiceAvatar };
export const unavailableAvatar = (reason = "Voice avatar is unavailable."): VoiceAvatar => ({ version: 1, state: "unavailable", parameters: null, candidate: null, reason });
export function decodeAvatar(value: unknown): VoiceAvatar {
  if (!value || typeof value !== "object" || !("version" in value) || value.version !== 1) return unavailableAvatar("Open the updated app to view your voice avatar.");
  const avatar = value as VoiceAvatar;
  const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
  const reference = avatar.candidate;
  if (!["ready_without_portrait", "unavailable", "source_unavailable"].includes(avatar.state)
    || !(avatar.reason === null || typeof avatar.reason === "string")
    || !(reference === null || (reference && typeof reference.id === "string" && typeof reference.revision === "string" && uuid.test(reference.id) && uuid.test(reference.revision)))) return unavailableAvatar("The voice avatar response is incompatible.");
  if (avatar.state !== "ready_without_portrait") return avatar.parameters === null && reference === null ? avatar : unavailableAvatar("The voice avatar response is incompatible.");
  const p = avatar.parameters;
  if (!p || p.version !== 1 || !Array.isArray(p.shape) || p.shape.length !== 24 || p.shape.some(v => !Number.isInteger(v) || v < 0 || v > 63)
    || !Array.isArray(p.petals) || p.petals.length !== 20 || p.petals.some(v => v !== null) || p.tempo !== null
    || !Number.isInteger(p.rotation) || p.rotation < 0 || p.rotation > 255 || typeof p.ring !== "string" || !/^[0-9a-f]{16}$/.test(p.ring)
    || typeof p.digest !== "string" || !/^[0-9a-f]{64}$/.test(p.digest) || avatar.reason !== null) return unavailableAvatar("The voice avatar parameters are incompatible.");
  return avatar;
}

/** Do not treat an incompatible or incomplete native reply as missing enrollment. */
export async function readSpeakerStore(): Promise<SpeakerStore> {
  const value = await command<SpeakerStore>("speaker_candidates");
  if (!value || typeof value.storage_directory !== "string" || !Array.isArray(value.candidates)
    || value.candidates.some(candidate => !candidate || typeof candidate.id !== "string"
      || typeof candidate.revision !== "string" || typeof candidate.state !== "string"
      || (candidate.model_revision !== null && typeof candidate.model_revision !== "string")
      || (candidate.segments !== null && candidate.segments !== 6)
      || (candidate.read_error !== null && typeof candidate.read_error !== "string"))) {
    throw new Error("Avesra couldn't understand the saved-voice result. Close Avesra and reopen the updated release. Your saved recordings have not been changed.");
  }
  return { ...value, avatar: decodeAvatar(value.avatar) };
}
