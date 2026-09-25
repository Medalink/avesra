<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { command, native, type Runtime } from "./runtime";
  let { id, revision, runtime, blocked, onbusy }: {
    id: string; revision: string; runtime: Runtime | null; blocked: boolean; onbusy: (value: boolean) => void;
  } = $props();
  type Result = { request: string; candidate: string; revision: string; transcript: string; similarity: number | null; held_out_similarities: number[]; seconds: number; clipped_samples: number; total_samples: number };
  type Progress = { request: string; phase: "checking" | "recording" | "processing"; seconds: number };
  let request = $state<string | null>(null);
  let phase = $state<Progress["phase"]>("checking");
  let seconds = $state(0);
  let result = $state<Result | null>(null);
  let error = $state("");
  let ready = $state(false);
  let disposed = false;
  async function cancel() {
    const active = request;
    if (active) await command("cancel_voice_check", { request: active });
  }
  async function record() {
    if (request || blocked || !ready) return;
    const active = crypto.randomUUID();
    request = active; phase = "checking"; seconds = 0; result = null; error = ""; onbusy(true);
    try {
      const value = await command<Result>("check_saved_voice", { request: active, id, revision });
      if (!disposed && request === active && value.request === active && value.candidate === id && value.revision === revision) result = value;
    } catch (e) { if (!disposed && request === active) error = String(e); }
    finally { if (request === active) { request = null; onbusy(false); } }
  }
  onMount(() => {
    let unlisten: (() => void) | undefined;
    if (native) void listen<Progress>("voice-check-progress", event => {
      if (event.payload.request === request && !disposed) { phase = event.payload.phase; seconds = event.payload.seconds; }
    }).then(stop => { if (disposed) stop(); else { unlisten = stop; ready = true; } }).catch(e => { error = String(e); });
    return () => { disposed = true; unlisten?.(); void cancel().catch(() => {}); };
  });
</script>

<div class="flex flex-col gap-2.5 border-t border-white/10 pt-3">
  <span class="text-[12.5px] font-medium">Microphone and transcription check</span>
  <p class="av-hint">Press Record, then read the sentence below. Recording stops automatically after eight seconds.</p>
  <p class="text-[12.5px] text-zinc-100">“Avesra, tell me what I have planned for today and help me choose what to work on next.”</p>
  <div class="flex items-center gap-2">
    <button class="av-btn av-btn-secondary av-btn-sm" disabled={!!request || blocked || !ready || !runtime?.connected || runtime.locked} onclick={record}>Record an eight-second check</button>
    {#if request}<button class="av-btn av-btn-secondary av-btn-sm" onclick={() => cancel().catch(e => error = String(e))}>Cancel</button>{/if}
  </div>
  {#if request}
    <div class="flex flex-col gap-1.5" role="status" aria-live="polite">
      <span class="av-hint">{phase === "checking" ? "Checking Spark speech services · microphone off…" : phase === "recording" ? `Speak now · ${seconds} / 8 seconds` : "Recording complete · Spark is transcribing and comparing your voice…"}</span>
      {#if phase === "recording"}<progress class="h-1.5 w-full accent-[#ac315b]" value={seconds} max={8} aria-label="Recording progress"></progress>{:else}<progress class="h-1.5 w-full accent-[#ac315b]" aria-label="Processing voice check"></progress>{/if}
    </div>
  {/if}
  {#if error}<p class="text-xs text-amber-300" role="alert">{error}</p>{/if}
  {#if result}
    <div class="flex flex-col gap-2 border border-white/10 bg-black/15 p-3" role="status">
      <span class="text-[12.5px] font-medium">What Spark heard</span>
      <p class="text-[12.5px] text-zinc-100">{result.transcript || "No speech was transcribed."}</p>
      <p class="av-hint">{result.transcript && result.similarity !== null ? "Your microphone reached Spark and speech was transcribed. No additional setup step is unlocked by this check." : "Speech could not be fully checked. Check your selected microphone in Audio & Voice, then try speaking again."}</p>
      <details><summary class="av-hint cursor-pointer">Technical comparison details</summary><p class="av-hint mt-2">Voice similarity: {result.similarity === null ? "not enough speech" : result.similarity.toFixed(3)} · saved held-out phrases: {result.held_out_similarities.map(v => v.toFixed(3)).join(", ")}</p><p class="av-hint">{result.seconds} seconds captured · {result.clipped_samples} clipped samples of {result.total_samples.toLocaleString()}. Similarity is a comparison score, not a confidence percentage.</p></details>
    </div>
  {/if}
  <p class="av-hint">This check sends this phrase to your paired Spark and keeps no recording. It does not run the spoken request or enable automatic listening.</p>
</div>
