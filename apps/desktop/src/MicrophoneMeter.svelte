<script lang="ts">
  import { onDestroy } from "svelte";
  import { command, native, type Runtime } from "./runtime";
  import Signal, { type SignalFrame } from "./Signal.svelte";
  let { runtime, signal }: { runtime: Runtime | null; signal: SignalFrame | null } = $props();
  let busy = $state(false);
  let error = $state("");
  let disposed = false;
  let ownedEpoch: number | null = null;
  const checking = $derived(!!runtime?.microphone_check);
  const live = $derived(signal?.kind === "human" && signal.captureEpoch === runtime?.capture_epoch);
  const rms = $derived(live ? signal?.rms ?? 0 : 0);
  const peak = $derived(live ? signal?.peak ?? 0 : 0);
  const db = $derived(rms > 0 ? Math.max(-60, 20 * Math.log10(rms)) : -60);
  const blocked = $derived(!native || !runtime?.settings.microphone || runtime.locked || runtime.settings.explicit_mute || runtime.settings.deafened || runtime.settings.paused || runtime.enrollment_capture || (runtime.enrolled && runtime.voice_ready));
  async function stop(epoch: number) {
    await command("stop_microphone_check", { epoch });
  }
  async function toggle() {
    if (busy) return;
    busy = true;
    error = "";
    try {
      if (checking && runtime) {
        await stop(runtime.capture_epoch);
        ownedEpoch = null;
      } else {
        const started = await command<Runtime>("begin_microphone_check");
        ownedEpoch = started.capture_epoch;
        if (disposed) await stop(started.capture_epoch);
      }
    } catch (e) { if (!disposed) error = String(e); }
    finally { if (!disposed) busy = false; }
  }
  onDestroy(() => {
    disposed = true;
    if (ownedEpoch !== null) void stop(ownedEpoch).catch(() => {});
  });
</script>

<div class="flex flex-col gap-1.5">
  <div class="flex items-center gap-2">
    <div class="relative h-6 flex-1" role="meter" aria-label="Microphone input level" aria-valuemin={-60} aria-valuemax={0} aria-valuenow={Math.round(db)} aria-valuetext={live ? `${Math.round(db)} dBFS` : "No current input measurement"}>
      <Signal frame={live ? signal : null} tone="owner" count={40} height={24} />
    </div>
    <span class="caption min-w-14 text-right text-zinc-400">{live ? peak >= 0.99 ? "clipping" : rms <= 0.001 ? "quiet" : `${Math.round(db)} dB` : checking ? "starting…" : runtime?.enrollment_capture ? "recording" : "idle"}</span>
  </div>
  <div class="flex items-center gap-2">
    <button class="av-btn av-btn-secondary av-btn-sm" disabled={busy || (!checking && blocked)} onclick={toggle}>{busy ? "Please wait…" : checking ? "Stop check" : "Check microphone"}</button>
    {#if checking}<span class="av-hint" role="status">Listening locally · stops after 30 seconds</span>{:else if runtime?.settings.explicit_mute}<span class="av-hint">Unmute to check.</span>{/if}
  </div>
  {#if error}<p class="text-xs text-amber-300" role="alert">{error}</p>{/if}
  <span class="av-hint">Checks input level only. No audio is saved or sent.</span>
</div>
