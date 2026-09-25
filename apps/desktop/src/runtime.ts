import { invoke, isTauri } from "@tauri-apps/api/core";
export type ShortcutAction = "mute" | "deafen" | "overlay";
export type Chord = { control: boolean; alt: boolean; shift: boolean; key: number };
export type Settings = {
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
  enrolled: boolean;
  voice_ready: boolean;
  enrollment_capture: boolean;
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
export async function command<T>(
  name: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!native) throw new Error("Open the Windows app to use local controls.");
  return invoke<T>(name, args);
}
