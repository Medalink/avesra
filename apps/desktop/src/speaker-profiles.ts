import { command } from "./runtime";

export type SpeakerCandidate = {
  id: string;
  revision: string;
  model_revision: string | null;
  segments: number | null;
  state: string;
  read_error: string | null;
};
export type SpeakerStore = { candidates: SpeakerCandidate[]; storage_directory: string };

/** Do not treat an incompatible or incomplete native reply as missing enrollment. */
export async function readSpeakerStore(): Promise<SpeakerStore> {
  const value = await command<SpeakerStore>("speaker_candidates");
  if (!value || typeof value.storage_directory !== "string" || !Array.isArray(value.candidates)
    || value.candidates.some(candidate => !candidate || typeof candidate.id !== "string"
      || typeof candidate.revision !== "string" || typeof candidate.state !== "string"
      || (candidate.segments !== null && candidate.segments !== 6)
      || (candidate.read_error !== null && typeof candidate.read_error !== "string"))) {
    throw new Error("Avesra couldn't understand the saved-voice result. Close Avesra and reopen the updated release. Your saved recordings have not been changed.");
  }
  return value;
}
