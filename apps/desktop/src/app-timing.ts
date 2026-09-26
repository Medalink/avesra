import { invoke, isTauri } from "@tauri-apps/api/core";
import { timedCommands } from "./app-timing-commands";
type Operation = { kind: "command" | "view"; name: string } | { kind: "app_mount" | "ui_ready" };
type Stage = "invoke_round_trip" | "mount_commit" | "next_frame" | "initialize";
type Outcome = "complete" | "failed" | "withdrawn" | "abandoned";
type Observation = { operation: Operation; stage: Stage; outcome: Outcome; duration_us: number };
const queue: Observation[] = [];
let page: string | null = null;
let lost = 0, sending = false;
let registration: "pending" | "available" | "unavailable" = "pending";
export function timingAvailability() { return { registration, pending: queue.length, unreported_loss: lost }; }
let timer: ReturnType<typeof setTimeout> | undefined;
const enabled = isTauri();
if (enabled) void invoke<string>("begin_app_timing").then(value => { page = value; registration = "available"; schedule(); }).catch(() => { registration = "unavailable"; lost += queue.length; queue.length = 0; });
function schedule() {
  if (!page || sending || timer || !queue.length) return;
  timer = setTimeout(() => { timer = undefined; void flush(); }, 1000);
}
async function flush() {
  if (!page || sending || !queue.length) return;
  sending = true;
  const records = queue.splice(0, 32), dropped = Math.min(lost, 1_000_000);
  lost -= dropped;
  try { await invoke("observe_app_timings", { batch: { page, records, lost: dropped } }); }
  catch { lost = Math.min(1_000_000, lost + dropped + records.length); }
  finally { sending = false; schedule(); }
}
export function timing(operation: Operation, stage: Stage) {
  const start = performance.now();
  let settled = false;
  return (outcome: Outcome) => {
    if (settled || !enabled) return;
    settled = true;
    if (registration === "unavailable") { lost = Math.min(1_000_000, lost + 1); return; }
    const duration_us = Math.round((performance.now() - start) * 1000);
    if (!Number.isFinite(duration_us) || duration_us < 0 || duration_us > 600_000_000 || queue.length >= 64) { lost = Math.min(1_000_000, lost + 1); return; }
    queue.push({ operation, stage, outcome, duration_us }); schedule();
  };
}
export function commandTiming(name: string) {
  return timedCommands.has(name) ? timing({ kind: "command", name }, "invoke_round_trip") : (_outcome: Outcome) => {};
}
export function viewTiming(name: string) {
  if (!["setup", "audio", "models", "profiles", "people", "awareness", "memory"].includes(name)) return null;
  return { commit: timing({ kind: "view", name }, "mount_commit"), frame: timing({ kind: "view", name }, "next_frame") };
}
