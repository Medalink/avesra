<script lang="ts">
  import Icon from "./Icon.svelte";
  import Signal, { type SignalFrame } from "./Signal.svelte";
  import type { Runtime } from "./runtime";
  import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
  import { native, command } from "./runtime";
  let {
    runtime,
    error,
    signal = null,
    control,
    hide,
    drag,
    showSettings,
  }: {
    runtime: Runtime | null;
    error: string;
    signal?: SignalFrame | null;
    control: (value: string) => Promise<void>;
    hide: () => Promise<void>;
    drag: (event: PointerEvent) => Promise<void>;
    showSettings: () => Promise<void>;
  } = $props();
  let expanded = $state(false);
  let soundError = $state("");
  async function toggleAtmosphere() {
    soundError = "";
    try { await command("update_sound", { edit: { kind: "enabled", value: !s?.sound.enabled } }); }
    catch (e) { soundError = String(e); }
  }
  const s = $derived(runtime?.settings);
  async function expand() {
    expanded = !expanded;
    if (native)
      await getCurrentWindow().setSize(
        new LogicalSize(440, expanded ? 370 : 124),
      );
  }
  const status = $derived(runtime?.status ?? "disconnected");
  const speaking = $derived(signal?.kind === "speaking" && runtime?.connected && !runtime.locked && !s?.deafened && !s?.paused);
  const preview = $derived(speaking && signal?.purpose === "preview");
  const activityLabel = $derived(speaking ? (preview ? "Voice preview" : "Replying") : runtime?.reason ?? "Native connection unavailable");
  const statusLabel = $derived(
    (
      {
        muted: "Muted",
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
</script>

<div
  class="overlay flex h-full flex-col overflow-hidden"
  class:bg-[#18181c]={expanded}
>
  <button
    class="flex h-24 w-full shrink-0 items-center px-2 focus-visible:outline-1 focus-visible:outline-av-400"
    aria-expanded={expanded}
    aria-label={`${statusLabel || status}. ${activityLabel}. Expand Avesra`}
    onclick={expand}
  >
    {#if speaking || ["passive", "recognizing", "accepted", "thinking", "speaking", "enrolling"].includes(status)}<span
        class="relative flex h-[88px] w-full items-center"
        ><Signal frame={signal} />{#if !signal}<span
            class="h-px w-full bg-white/15"
          ></span>{/if}</span
      >{:else}<span
        class="flex w-full items-center gap-3 {status === 'disconnected'
          ? 'text-red-500'
          : status === 'paused'
            ? 'text-zinc-300'
            : 'text-amber-400'}"
        ><span
          class="h-px flex-1 {status === 'disconnected'
            ? 'bg-red-500/70'
            : 'border-t border-dashed border-current opacity-60'}"
        ></span><Icon
          name={status === "disconnected"
            ? "close"
            : status === "paused"
              ? "pause"
              : status === "deafened"
                ? "deafen_off"
                : "mic_off"}
          size={17}
        /><span
          class="h-px flex-1 {status === 'disconnected'
            ? 'bg-red-500/70'
            : 'border-t border-dashed border-current opacity-60'}"
        ></span></span
      >{/if}
  </button>
  <div class="flex h-7 shrink-0 items-center gap-1.5 pr-1 pl-2.5">
    <button
      class="reveal text-zinc-500"
      onpointerdown={drag}
      aria-label="Drag overlay">⠿</button
    ><span
      class="text-[11.5px] {status === 'disconnected'
        ? 'text-red-400'
        : 'text-amber-200'}">{speaking ? (s?.explicit_mute ? "Muted" : "") : statusLabel}</span
    ><span class="flex-1"></span>
    {#if runtime?.active_task || speaking}<button
        class="av-iconbtn size-6 text-red-400"
        onclick={() => control("stop")}
        aria-label={speaking ? "Stop output and task" : "Stop task"}><Icon name="stop" size={12} /></button
      >{/if}
    <button
      class="av-iconbtn size-6 {s?.explicit_mute ? 'av-iconbtn-on' : 'reveal'}"
      aria-label={s?.explicit_mute ? "Unmute microphone" : "Mute microphone"}
      aria-pressed={!!s?.explicit_mute}
      disabled={!runtime}
      onclick={() => control(s?.explicit_mute ? "unmute" : "mute")}
      ><Icon name={s?.explicit_mute ? "mic_off" : "mic"} size={13} /></button
    >
    <button
      class="av-iconbtn size-6 {s?.deafened ? 'av-iconbtn-on' : 'reveal'}"
      aria-label={s?.deafened ? "Restore assistant sound" : "Deafen assistant"}
      aria-pressed={!!s?.deafened}
      disabled={!runtime}
      onclick={() => control(s?.deafened ? "undeafen" : "deafen")}
      ><Icon name={s?.deafened ? "deafen_off" : "deafen"} size={13} /></button
    >
    <button type="button" class="av-iconbtn size-6 {s?.sound.enabled ? 'av-iconbtn-on' : 'reveal'}" aria-label={s?.sound.enabled ? "Turn voice atmosphere off" : "Turn voice atmosphere on"} aria-pressed={!!s?.sound.enabled} title={soundError || `Voice atmosphere ${s?.sound.enabled ? "on" : "off"}`} disabled={!runtime} onclick={toggleAtmosphere}><Icon name="atmosphere" size={13} /></button>
    <button
      class="av-iconbtn reveal size-6"
      aria-label="Open settings"
      onclick={showSettings}><Icon name="settings" size={13} /></button
    ><button
      class="av-iconbtn reveal size-6"
      aria-label="Hide to tray"
      onclick={hide}><Icon name="minimize" size={13} /></button
    >
  </div>
  {#if expanded}<div
      class="flex flex-1 flex-col gap-4 border-t border-white/[0.07] px-4 pt-3.5 pb-3.5"
    >
      <div>
        <span class="av-kicker">Avesra</span>
        <p class="mt-1 text-[13px] text-zinc-300">
          {speaking ? (preview ? (s?.explicit_mute ? "Previewing a voice candidate. Microphone muted." : "Previewing a voice candidate.") : s?.explicit_mute ? "Still replying · unmute to talk" : "Replying.") : runtime?.reason ?? "The local companion is unavailable."}
        </p>
      </div>
      <div class="warning">
        {runtime?.microphone_check ? "Checking microphone levels locally. Stop in Audio & Voice or mute to stop." : runtime?.enrollment_capture ? "Recording your explicit enrollment phrase. Mute or cancel in Settings to stop. No screen is being captured." : runtime?.enrolled && runtime.voice_ready ? runtime.reason : "Setup is incomplete. No microphone or screen is being captured."}
      </div>
      {#if error || soundError}<p class="text-xs text-red-300" role="alert">
          {error || soundError}
        </p>{/if}<button
        class="av-btn av-btn-secondary self-start"
        onclick={showSettings}
        >Open settings <Icon name="arrow" size={12} /></button
      >{#if !speaking}<span class="av-hint">Task and learning history is unavailable in this panel.</span>{/if}
    </div>{/if}
</div>
