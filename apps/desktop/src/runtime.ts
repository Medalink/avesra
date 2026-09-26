import { invoke, isTauri } from "@tauri-apps/api/core";
export type ShortcutAction = "mute" | "deafen" | "overlay";
export type Chord = { control: boolean; alt: boolean; shift: boolean; key: number };
export type SoundPreset = "digital" | "human";
export type SoundAmounts = {
  effects: boolean;
  background_enabled: boolean;
  character: number;
  background: number;
  warmth: number;
  space: number;
  texture: number;
  harmonizer_depth: number;
  presence: number;
  echo: number;
  echo_delay_left_ms: number;
  echo_delay_right_ms: number;
  background_texture: "atmosphere" | "pink" | "white";
};
export type SoundSettings = {
  revision: number;
  enabled: boolean;
  preset: SoundPreset;
  digital: SoundAmounts;
  human: SoundAmounts;
};
export type SoundEdit =
  | { kind: "enabled"; value: boolean }
  | { kind: "preset"; value: SoundPreset }
  | { kind: "volume"; value: number }
  | { kind: "amount"; preset: SoundPreset; field: "character" | "background" | "warmth" | "space" | "texture" | "presence" | "echo"; value: number }
  | { kind: "layer"; preset: SoundPreset; field: "effects" | "background"; value: boolean }
  | { kind: "echo_delay"; preset: SoundPreset; channel: "left" | "right"; value: number }
  | { kind: "harmonizer_depth"; preset: SoundPreset; value: number }
  | { kind: "background_texture"; preset: SoundPreset; value: SoundAmounts["background_texture"] };
export type Settings = {
  owner_name: { actor: string; name: string } | null;
  sound: SoundSettings;
  shortcuts: Record<ShortcutAction, Chord | null>;
  audio_device_schema: number;
  microphone: string | null;
  speaker: string | null;
  profile: "single-spark" | "accelerated" | "gaming";
  always_on_top: boolean;
  interface_scale: number;
  learning_chime: boolean;
  action_chime: boolean;
  chime_volume: number;
  learning_chime_volume: number | null;
  action_chime_volume: number | null;
  speech_volume: number;
  speech_rate: number;
  explicit_mute: boolean;
  deafened: boolean;
  paused: boolean;
};
export type Runtime = {
  revision: number;
  settings: Settings;
  capture_epoch: number;
  playback_epoch: number;
  action_epoch: number;
  connected: boolean;
  connection_phase: "disconnected" | "connecting" | "connected" | "error";
  connection_error: string | null;
  enrolled: boolean;
  voice_ready: boolean;
  enrollment_capture: boolean;
  microphone_check: boolean;
  capture_error: string | null;
  locked: boolean;
  active_task: boolean;
  status: string;
  reason: string;
};
export type AudioDevice = {
  id: string;
  name: string;
  direction: "input" | "output";
  is_default: boolean;
};
export const native = isTauri();
export function sparkConnection(runtime: Runtime | null) {
  const phase = runtime?.connection_phase ?? "disconnected";
  return {
    connected: phase === "connected",
    connecting: phase === "connecting",
    label: phase === "error" ? "connection failed" : phase,
    detail: runtime?.connection_error ?? (phase === "connecting" ? "Connecting securely to your saved Spark…" : "Open Spark connection settings"),
  };
}
export async function command<T>(
  name: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!native) throw new Error("Open the Windows app to use local controls.");
  return invoke<T>(name, args);
}
