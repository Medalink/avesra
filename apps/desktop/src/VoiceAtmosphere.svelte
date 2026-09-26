<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { command, native, type Runtime, type SoundEdit, type SoundPreset, type SoundAmounts } from "./runtime";
  import Icon from "./Icon.svelte";
  let { runtime }: { runtime: Runtime | null } = $props();
  const sound = $derived(runtime?.settings.sound);
  const preset = $derived(sound?.preset ?? "digital");
  const amounts = $derived(sound?.[preset]);
  let error = $state("");
  let channels = $state<number | null>(null);
  let advanced = $state(false);
  let outstanding = 0;
  type AmountField = Extract<SoundEdit, { kind: "amount" }>["field"];
  const pending = new Map<string, SoundEdit>();
  let flushing = false;
  let mounted = true;
  // Keep only the newest slider position for each preset/field. A slow disk
  // write cannot grow an input-event backlog or delay the direct bypass button.
  async function edit(value: SoundEdit) {
    const operation = ++outstanding;
    error = "";
    try { await command<Runtime>("update_sound", { edit: value }); }
    catch (e) { if (operation === outstanding) error = String(e); }
  }
  async function flushAmounts() {
    if (flushing) return;
    flushing = true;
    try {
      while (mounted && pending.size) {
        const [key, value] = pending.entries().next().value!;
        pending.delete(key);
        await edit(value);
        await new Promise(resolve => setTimeout(resolve, 40));
      }
    } finally { flushing = false; }
  }
  function amount(field: AmountField, value: string) {
    pending.set(`${preset}:${field}`, { kind: "amount", preset, field, value: Number(value) * (field === "space" ? 2 : 1) });
    void flushAmounts();
  }
  function displayedAmount(field: AmountField) {
    return (amounts?.[field] ?? 0) / (field === "space" ? 2 : 1);
  }
  function echoDelay(channel: "left" | "right", value: string) {
    pending.set(`${preset}:delay:${channel}`, { kind: "echo_delay", preset, channel, value: Number(value) });
    void flushAmounts();
  }
  function harmonyDepth(value: string) {
    pending.set(`${preset}:harmonizer_depth`, { kind: "harmonizer_depth", preset, value: Number(value) * -2 });
    void flushAmounts();
  }
  function navigatePreset(event: KeyboardEvent) {
    if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)
      || event.altKey || event.ctrlKey || event.metaKey) return;
    const current = event.currentTarget;
    if (!(current instanceof HTMLButtonElement)) return;
    const cards = current.parentElement?.querySelectorAll<HTMLButtonElement>('button[role="radio"]');
    const next = cards && Array.from(cards).find(card => card !== current && !card.disabled);
    if (!next) return;
    event.preventDefault();
    next.focus();
    next.click();
  }
  onMount(() => {
    mounted = true;
    let remove = () => {};
    let observed = false;
    if (native) void (async () => {
      const unlisten = await listen<{ output_channels: number | null }>("media-health", event => {
        observed = true;
        if (mounted) channels = event.payload.output_channels;
      });
      if (!mounted) { unlisten(); return; }
      remove = unlisten;
      const current = await command<number | null>("sound_output_channels");
      if (mounted && !observed) channels = current;
    })().catch(() => { if (mounted) channels = null; });
    return () => { mounted = false; pending.clear(); remove(); };
  });
</script>

<section class="flex flex-col gap-3">
  <span class="av-kicker">Voice atmosphere</span>
  <div class="av-card flex flex-col gap-4 p-3.5" aria-label="Voice atmosphere">
    <div class="flex items-center justify-between gap-3">
      <div class="flex items-center gap-2.5">
        <span class="shrink-0 text-av-400"><Icon name="atmosphere" size={19} /></span>
        <div class="flex flex-col">
          <span class="text-[13px] font-medium text-zinc-50">Voice character and background sound</span>
          <span class="av-hint">Drag to tune live. Repeat the preview above to compare. Changes save as you go.</span>
        </div>
      </div>
      <span class="flex items-center gap-2">
        <span class="font-mono text-[11px] text-zinc-300">{sound?.enabled ? "On" : "Off"}</span>
        <button type="button" role="switch" class="av-switch group" aria-checked={!!sound?.enabled} aria-label="Voice atmosphere" disabled={!runtime} onclick={() => edit({ kind: "enabled", value: !sound?.enabled })}><span class="av-knob group-aria-checked:translate-x-[18px]"></span></button>
      </span>
    </div>
    <div class="grid grid-cols-2 gap-2" role="radiogroup" aria-label="Voice atmosphere preset">
      {#each ["digital", "human"] as choice}
        <button type="button" role="radio" tabindex={preset === choice ? 0 : -1} onkeydown={navigatePreset} aria-checked={preset === choice} class={`flex cursor-pointer flex-col gap-1 p-3 text-left ring-1 ring-inset transition-colors focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-av-400 ${preset === choice ? "bg-white/[0.06] ring-av-500 shadow-[inset_0_-2px_0_var(--color-av-500)]" : "bg-white/[0.02] ring-white/[0.08] hover:bg-white/[0.04] hover:ring-white/15"}`} disabled={!runtime} onclick={() => edit({ kind: "preset", value: choice as SoundPreset })}>
          <span class="flex items-center justify-between gap-2"><span class="text-[13px] font-medium text-zinc-50">{choice === "digital" ? "Digital" : "Human"}</span><span class="font-mono text-[10px] tracking-[0.1em] text-zinc-400 uppercase">{choice === "digital" ? "Default" : ""}</span></span>
          <span class="av-hint">{choice === "digital" ? "Resonant, synthetic and spacious." : "Warm, close and lightly processed."}</span>
        </button>
      {/each}
    </div>
    <span class="av-hint">{sound?.enabled ? (preset === "digital" ? "A dark Galaxy layer follows the voice and lowers beneath each phrase, then fades out after it." : "A closer presentation of your selected voice. Background starts off.") : "Original voice. Your preset is saved; turn atmosphere on to hear it."}</span>
    <div class="grid grid-cols-2 gap-5">
      {#each [["presence", "Voice presence"], ["background", "Galaxy / background"]] as [field, label]}
        <div class="flex flex-col gap-2">
          <div class="flex items-center justify-between"><label class="av-label" for={`sound-${field}`}>{label}</label><span class="font-mono text-[11.5px] text-zinc-300">{amounts?.[field as AmountField] ?? 0}%</span></div>
          <input id={`sound-${field}`} type="range" min="0" max="100" step="1" class="av-range disabled:cursor-not-allowed disabled:opacity-40" disabled={!runtime || (preset === "human" && field === "presence")} value={amounts?.[field as AmountField] ?? 0} oninput={e => amount(field as AmountField, e.currentTarget.value)} />
        </div>
      {/each}
    </div>
    <div class="flex flex-wrap items-center gap-3">
      <span class="av-hint">Compare during preview</span>
      <div class="av-seg" role="group" aria-label="Compare voice processing">
        <button type="button" class="av-seg-btn" aria-pressed={!sound?.enabled} disabled={!runtime} onclick={() => edit({ kind: "enabled", value: false })}>Original</button>
        <button type="button" class="av-seg-btn" aria-pressed={!!sound?.enabled} disabled={!runtime} onclick={() => edit({ kind: "enabled", value: true })}>Atmosphere</button>
      </div>
      <span class="av-hint">Speech keeps playing.</span>
    </div>
    <button type="button" class="av-btn av-btn-ghost av-btn-sm self-start" aria-expanded={advanced} onclick={() => advanced = !advanced}>{advanced ? "Hide sound controls" : "More sound controls"}</button>
    {#if advanced}
      <div class="av-rise flex flex-col gap-4 border-t border-white/10 pt-3">
        <div class="flex flex-wrap gap-5">
          {#each [["effects", "Voice effects", amounts?.effects], ["background", "Background sound", amounts?.background_enabled]] as [field, label, enabled]}
            <span class="flex items-center gap-2.5">
              <span class="text-[12.5px] text-zinc-100">{label}</span>
              <button type="button" role="switch" class="av-switch group" aria-label={String(label)} aria-checked={!!enabled} disabled={!runtime} onclick={() => edit({ kind: "layer", preset, field: field as "effects" | "background", value: !enabled })}><span class="av-knob group-aria-checked:translate-x-[18px]"></span></button>
            </span>
          {/each}
        </div>
        <div class="grid grid-cols-3 gap-x-5 gap-y-4">
          {#each [["warmth", "Bass / warmth"], ["space", "Reverb"], ["echo", "Echo"], ["texture", "Harmonizer"], ["character", "Compression"]] as [field, label]}
            <div class="flex flex-col gap-2">
              <div class="flex items-center justify-between gap-2"><label class="av-label" for={`sound-${field}`}>{label}</label><span class="font-mono text-[11.5px] text-zinc-300">{displayedAmount(field as AmountField)}%</span></div>
              <input id={`sound-${field}`} type="range" min="0" max="100" step={field === "space" ? 0.5 : 1} class="av-range disabled:cursor-not-allowed disabled:opacity-40" disabled={!runtime || (preset === "human" && (field === "texture" || field === "echo"))} value={displayedAmount(field as AmountField)} oninput={e => amount(field as AmountField, e.currentTarget.value)} />
            </div>
          {/each}
          <div class="flex flex-col gap-2">
            <div class="flex items-center justify-between gap-2"><label class="av-label" for="harmonizer-depth">Harmony pitch</label><span class="font-mono text-[11.5px] text-zinc-300">{(amounts?.harmonizer_depth ?? 1) === 0 ? "Unison" : `${Math.abs(amounts?.harmonizer_depth ?? 1) / 2} semitones ${(amounts?.harmonizer_depth ?? 1) > 0 ? "lower" : "higher"}`}</span></div>
            <input id="harmonizer-depth" type="range" min="-12" max="12" step="0.5" class="av-range disabled:cursor-not-allowed disabled:opacity-40" disabled={!runtime || preset === "human"} value={(amounts?.harmonizer_depth ?? 1) / -2} oninput={e => harmonyDepth(e.currentTarget.value)} />
            <span class="av-hint">Lower ← Unison → Higher</span>
          </div>
        </div>
        <div class="grid grid-cols-3 gap-x-5 gap-y-4">
          <div class="flex flex-col gap-2">
            <label class="av-label" for="background-texture">Background texture</label>
            <div class="relative">
              <select id="background-texture" class="av-input av-select" disabled={!runtime} value={amounts?.background_texture ?? "atmosphere"} onchange={e => edit({ kind: "background_texture", preset, value: e.currentTarget.value as SoundAmounts["background_texture"] })}><option value="atmosphere">Galaxy ambience</option><option value="pink">Soft filtered noise</option><option value="white">White noise</option></select>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" class="pointer-events-none absolute top-1/2 right-2.5 -translate-y-1/2 text-zinc-400" aria-hidden="true"><path d="m6 9 6 6 6-6"></path></svg>
            </div>
          </div>
          {#each ["left", "right"] as channel}
            <div class="flex flex-col gap-2">
              <div class="flex items-center justify-between gap-2"><label class="av-label" for={`echo-delay-${channel}`}>{channel === "left" ? "Left echo spacing" : "Right echo spacing"}</label><span class="font-mono text-[11.5px] text-zinc-300">+{amounts?.[channel === "left" ? "echo_delay_left_ms" : "echo_delay_right_ms"] ?? 0} ms</span></div>
              <input id={`echo-delay-${channel}`} type="range" min="0" max="400" step="5" class="av-range disabled:cursor-not-allowed disabled:opacity-40" disabled={!runtime || preset === "human"} value={amounts?.[channel === "left" ? "echo_delay_left_ms" : "echo_delay_right_ms"] ?? 0} oninput={e => echoDelay(channel as "left" | "right", e.currentTarget.value)} />
            </div>
          {/each}
        </div>
        <span class="av-hint">Echo spacing adds time between repeats; zero keeps the current timing and the main voice stays immediate. {channels === 2 ? "Stereo output: centered voice with a wide atmosphere." : channels ? "Mono presentation on this speaker layout. Spatial processing is unavailable." : "Output layout is checked when you play a preview."} Both presets keep your selected voice.</span>
      </div>
    {/if}
    {#if error}<p class="av-hint text-amber-200" role="alert">{error}</p>{/if}
  </div>
</section>
