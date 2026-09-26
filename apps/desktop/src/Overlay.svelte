<script lang="ts">
  import Icon from "./Icon.svelte";
  import Signal, { type SignalFrame } from "./Signal.svelte";
  import type { Runtime } from "./runtime";
  let {
    runtime,
    error,
    signal = null,
    expanded = $bindable(false),
    control,
    hide,
    drag,
    showSettings,
  }: {
    runtime: Runtime | null;
    error: string;
    signal?: SignalFrame | null;
    expanded?: boolean;
    control: (value: string) => Promise<void>;
    hide: () => Promise<void>;
    drag: (event: PointerEvent) => Promise<void>;
    showSettings: () => Promise<void>;
  } = $props();
  const s = $derived(runtime?.settings);
  const status = $derived(runtime?.status ?? "disconnected");
  const speaking = $derived(signal?.kind === "speaking" && runtime?.connected && !runtime.locked && !s?.deafened && !s?.paused);
  const preview = $derived(speaking && signal?.purpose === "preview");
  const reveal = $derived(expanded ? "" : "reveal");
  const activityLabel = $derived(speaking ? (preview ? "Voice preview" : signal?.purpose === "greeting" ? "Saying hello" : "Replying") : runtime?.reason ?? "Native connection unavailable");
  const statusLabel = $derived(
    (
      {
        muted: "Mic muted",
        deafened: "Deafened",
        paused: "Paused",
        disconnected: "Disconnected",
        unavailable: "Setup needed",
        working: "Working",
        enrolling: "Recording enrollment",
        checking: "Checking microphone",
      } as Record<string, string>
    )[status] ?? "",
  );
  const visibleStatus = $derived(speaking ? (s?.explicit_mute ? "Mic muted" : "") : statusLabel);
  const statusTone = $derived(status === "disconnected" ? "text-red-400" : status === "working" ? "text-av-300" : status === "paused" ? "text-zinc-100" : "text-amber-300");
  const statusDot = $derived(status === "disconnected" ? "bg-red-500" : status === "working" ? "bg-av-500" : status === "paused" ? "bg-zinc-400" : "bg-amber-400");
  let pausePending = $state(false);
  async function togglePause() {
    if (pausePending || !runtime || runtime.locked) return;
    pausePending = true;
    try { await control(runtime.settings.paused ? "resume" : "pause"); }
    finally { pausePending = false; }
  }

</script>

<div
  class="overlay relative flex h-full w-full flex-col overflow-hidden text-zinc-100 ring-1 transition-[background-color,box-shadow] duration-200 {expanded
    ? 'bg-[#1f1f23]/95 ring-white/10 backdrop-blur-md shadow-[0_16px_40px_-12px_rgba(0,0,0,0.85)]'
    : 'bg-transparent ring-transparent hover:bg-black/30 hover:ring-white/10 focus-within:ring-white/10'}"
>
  <button
    class="flex h-24 w-full shrink-0 items-center px-2 focus-visible:outline-1 focus-visible:outline-av-400"
    title={expanded ? "Collapse" : "Show details"}
    aria-expanded={expanded}
    aria-label={`${statusLabel || status}. ${activityLabel}.${status === "working" && !speaking ? " Step progress unavailable." : ""} ${expanded ? "Collapse" : "Expand"} Avesra`}
    onclick={() => expanded = !expanded}
  >
    {#if speaking || ["passive", "recognizing", "accepted", "thinking", "speaking", "enrolling"].includes(status)}<span
        class="relative flex h-[88px] w-full items-center"
        ><Signal frame={signal} height={84} /></span
      >{:else if status === "working"}<span class="flex w-full items-center gap-1" aria-hidden="true">
        {#each [0, 1, 2, 3, 4] as segment (segment)}
          <span class="relative h-1 flex-1 overflow-hidden bg-white/10"><span class="av-shimmer absolute inset-0 bg-av-500/10"></span></span>
        {/each}
      </span>{:else if status === "paused"}<span class="flex w-full items-center gap-3 text-zinc-300" aria-hidden="true">
        <span class="av-hatch h-[3px] flex-1"></span>
        <svg width="13" height="13" viewBox="0 0 24 24" fill="currentColor"><rect x="5" y="4" width="5" height="16"></rect><rect x="14" y="4" width="5" height="16"></rect></svg>
        <span class="av-hatch h-[3px] flex-1"></span>
      </span>{:else}<span
        class="flex w-full items-center {status === 'disconnected' ? 'gap-2.5' : 'gap-3'} {status === 'disconnected'
          ? 'text-red-500'
          : 'text-amber-400'}"
        ><span
          class="h-px flex-1 {status === 'disconnected'
            ? 'bg-red-500/70'
            : 'border-t border-dashed border-current opacity-60'}"
        ></span>{#if status === "disconnected"}<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="square" aria-hidden="true"><path d="M6 6l12 12M18 6 6 18"></path></svg>{:else}<Icon name={status === "deafened" ? "deafen_off" : "mic_off"} size={17} />{/if}<span
          class="h-px flex-1 {status === 'disconnected'
            ? 'bg-red-500/70'
            : 'border-t border-dashed border-current opacity-60'}"
        ></span></span
      >{/if}
  </button>
  <div class="flex h-7 shrink-0 items-center gap-1.5 pr-1 pl-2.5">
    <button
      type="button"
      class="{reveal} grid h-6 w-2.5 shrink-0 cursor-grab place-items-center text-zinc-500 active:cursor-grabbing"
      onpointerdown={drag}
      aria-label="Drag overlay"
      title="Drag overlay"><svg width="8" height="12" viewBox="0 0 8 12" fill="currentColor" aria-hidden="true"><rect x="0.5" y="0.5" width="2" height="2"></rect><rect x="5" y="0.5" width="2" height="2"></rect><rect x="0.5" y="5" width="2" height="2"></rect><rect x="5" y="5" width="2" height="2"></rect><rect x="0.5" y="9.5" width="2" height="2"></rect><rect x="5" y="9.5" width="2" height="2"></rect></svg></button>
    {#if visibleStatus}
      <span class="size-1.5 shrink-0 {statusDot}" aria-hidden="true"></span>
      <span class="av-shadow-text shrink-0 font-mono text-[10.5px] font-medium tracking-[0.12em] uppercase {statusTone}" aria-live="polite">{visibleStatus}</span>
      {#if !expanded}<span class="av-shadow-text min-w-0 flex-1 truncate text-[11.5px] text-zinc-300" title={activityLabel}>{activityLabel}</span>{:else}<span class="flex-1"></span>{/if}
    {:else}
      <span class="sr-only" aria-live="polite">{activityLabel}</span><span class="flex-1"></span>
    {/if}
    {#if runtime?.active_task || speaking}<button
        class="av-iconbtn size-6 text-red-500 ring-1 ring-red-500/60 ring-inset hover:bg-red-600 hover:text-white"
        onclick={() => control("stop")}
        aria-label={speaking ? "Stop output and task" : "Stop task"} title={speaking ? "Stop output and task" : "Stop task"}><svg width="8" height="8" viewBox="0 0 10 10" fill="currentColor" aria-hidden="true"><rect x="0.5" y="0.5" width="9" height="9"></rect></svg></button
      >{/if}
    <button
      class="av-iconbtn size-6 {s?.explicit_mute ? 'av-iconbtn-on' : reveal}"
      aria-label={s?.explicit_mute ? "Unmute microphone" : "Mute microphone"}
      aria-pressed={!!s?.explicit_mute}
      disabled={!runtime}
      onclick={() => control(s?.explicit_mute ? "unmute" : "mute")}
      ><Icon name={s?.explicit_mute ? "mic_off" : "mic"} size={13} /></button
    >
    <button
      class="av-iconbtn size-6 {s?.deafened ? 'av-iconbtn-on' : reveal}"
      aria-label={s?.deafened ? "Restore assistant sound" : "Deafen assistant"}
      aria-pressed={!!s?.deafened}
      disabled={!runtime}
      onclick={() => control(s?.deafened ? "undeafen" : "deafen")}
      ><Icon name={s?.deafened ? "deafen_off" : "deafen"} size={13} /></button
    >
    <button
      class="av-iconbtn {reveal} size-6"
      aria-label="Open settings"
      onclick={showSettings}><Icon name="settings" size={13} /></button
    ><button
      class="av-iconbtn {reveal} size-6"
      aria-label="Hide to tray"
      onclick={hide}><Icon name="minimize" size={13} /></button
    >
  </div>
  {#if expanded}<div
      class="av-scroll flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto border-t border-white/[0.07] px-4 pt-3.5 pb-3.5"
    >
      <div>
        <span class="av-kicker">Avesra</span>
        {#if speaking || runtime?.microphone_check || runtime?.enrollment_capture}<p class="mt-1 text-[13px] text-zinc-300">
          {speaking ? (preview ? (s?.explicit_mute ? "Previewing a voice candidate. Microphone muted." : "Previewing a voice candidate.") : s?.explicit_mute ? "Still replying · unmute to talk" : "Replying.") : runtime?.reason ?? "The local companion is unavailable."}
        </p>{/if}
      </div>
      {#if status === "disconnected"}
        <div class="flex items-start gap-2.5 bg-red-500/[0.08] px-3 py-2.5 ring-1 ring-red-500/25 ring-inset" role="status">
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="mt-0.5 shrink-0 text-red-400" aria-hidden="true"><path d="M9 17H7A5 5 0 0 1 7 7"></path><path d="M15 7h2a5 5 0 0 1 4 8"></path><path d="M3 3l18 18"></path></svg>
          <div class="flex min-w-0 flex-1 flex-col gap-0.5">
            <span class="text-[12.5px] font-medium text-red-200">Connection unavailable</span>
            <span class="text-[12px] leading-[17px] text-red-200/80">{runtime?.reason ?? "The local companion is unavailable."}</span>
          </div>
        </div>
      {:else if status === "paused"}
        <div class="av-hatch flex items-start gap-2.5 px-3 py-2.5 ring-1 ring-zinc-300/25 ring-inset" role="status">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" class="mt-0.5 shrink-0 text-zinc-200" aria-hidden="true"><rect x="5" y="4" width="5" height="16"></rect><rect x="14" y="4" width="5" height="16"></rect></svg>
          <span class="text-[12px] leading-[17px] text-zinc-200">{runtime?.reason ?? "Assistant paused."}</span>
        </div>
      {:else}<div class="warning">
        {runtime?.microphone_check ? "Checking microphone levels locally. Stop in Audio & Voice or mute to stop." : runtime?.enrollment_capture ? "Recording your explicit enrollment phrase. Mute or cancel in Settings to stop. No screen is being captured." : runtime?.reason ?? "Current assistant status is unavailable."}
      </div>{/if}
      {#if error}<p class="text-xs text-red-300" role="alert">
          {error}
        </p>{/if}<button
        class="av-btn av-btn-secondary self-start"
        onclick={showSettings}
        >Open settings <Icon name="arrow" size={12} /></button
      >{#if !speaking}<span class="av-hint">Task and learning history is unavailable in this panel.</span>{/if}
    </div>
    <footer class="flex shrink-0 items-center gap-1 border-t border-white/[0.07] px-2 py-2">
      <button type="button" class="av-btn av-btn-ghost" onclick={() => expanded = false}><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m6 15 6-6 6 6"></path></svg>Collapse</button>
      <button type="button" class="av-btn av-btn-ghost" disabled={!runtime || runtime.locked || pausePending} onclick={togglePause}>
        {#if s?.paused}<svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M7 4.5v15l12-7.5z"></path></svg>{:else}<svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><rect x="5" y="4" width="5" height="16" rx="1.5"></rect><rect x="14" y="4" width="5" height="16" rx="1.5"></rect></svg>{/if}
        {s?.paused ? "Resume" : "Pause assistant"}
      </button>
    </footer>{/if}
</div>
