<script lang="ts">
  import Icon from "./Icon.svelte";
  import { native, type Runtime, type AudioDevice } from "./runtime";

  let {
    runtime,
    devices,
    devicesLoading,
    devicesError,
    navigate,
  }: {
    runtime: Runtime | null;
    devices: AudioDevice[];
    devicesLoading: boolean;
    devicesError: string;
    navigate: (section: string, target?: string) => void;
  } = $props();

  const connected = $derived(runtime?.connected === true);
  const input = $derived(
    devices.find(
      (d) => d.direction === "input" && d.id === runtime?.settings.microphone,
    ),
  );
  const output = $derived(
    devices.find(
      (d) => d.direction === "output" && d.id === runtime?.settings.speaker,
    ),
  );
  const audioSelected = $derived(
    !!input && !!output && !devicesLoading && !devicesError,
  );
  const steps = $derived([
    {
      number: "01",
      icon: "profiles",
      title: "Connect your Spark",
      state: connected ? "Complete" : "Setup needed",
      ready: connected,
      detail: connected
        ? "Your encrypted connection is active."
        : "Pair this PC with your local assistant.",
      action: connected ? "Manage connection" : "Pair Spark",
      section: "profiles",
      target: "spark-pairing",
    },
    {
      number: "02",
      icon: "models",
      title: "Prepare the models",
      state: runtime?.voice_ready ? "Voice ready" : "Not ready",
      ready: !!runtime?.voice_ready,
      detail: "Check the services that listen, reason and speak.",
      action: "Review services",
      section: "models",
    },
    {
      number: "03",
      icon: "audio",
      title: "Choose your devices",
      state: devicesLoading
        ? "Checking"
        : devicesError
          ? "Unavailable"
          : audioSelected
            ? "Selected"
            : "Selection needed",
      ready: audioSelected,
      detail: audioSelected
        ? `${input?.name} · ${output?.name}`
        : "Choose a microphone and speakers for this PC.",
      action: "Set up audio",
      section: "audio",
    },
    {
      number: "04",
      icon: "people",
      title: "Make it yours",
      state: runtime?.enrolled ? "Enrolled" : "Setup needed",
      ready: !!runtime?.enrolled,
      detail: "Verify the owner and enroll your voice.",
      action: "Set up voice identity",
      section: "people",
    },
    {
      number: "05",
      icon: "mic",
      title: "Give Avesra a voice",
      state: "Review needed",
      ready: false,
      detail: "Create, preview and select a generated voice.",
      action: "Open voice designer",
      section: "audio",
      target: "voice-designer",
    },
    {
      number: "06",
      icon: "awareness",
      title: "Choose what it can do",
      state: "Review needed",
      ready: false,
      detail: "Review observation, app aliases and browser access.",
      action: "Review actions",
      section: "awareness",
    },
  ]);
  const restrictions = $derived([
    ...(runtime?.locked ? ["Windows locked"] : []),
    ...(runtime?.settings.paused ? ["Assistant paused"] : []),
    ...(runtime?.settings.deafened ? ["Deafened"] : []),
    ...(runtime?.settings.explicit_mute ? ["Microphone muted"] : []),
  ]);
</script>

<section class="setup-intro" aria-label="Companion status">
  <div class="flex items-start gap-4">
    <span
      class="grid size-11 shrink-0 place-items-center bg-av-500/10 text-av-400 ring-1 ring-av-500/25 ring-inset"
      ><Icon name="audio" size={25} /></span
    >
    <div class="min-w-0 flex-1">
      <span class="av-kicker">Your local assistant</span>
      <h2 class="mt-1 text-[20px] font-medium tracking-[-0.035em]">
        Let's get Avesra ready.
      </h2>
      <p class="mt-2 text-[12px] leading-[18px] text-zinc-400">
        Connect your Spark, choose your devices and make Avesra yours.
      </p>
    </div>
  </div>
  <div
    class="mt-4 flex flex-wrap items-center gap-2 border-t border-white/[0.08] pt-3"
    aria-live="polite"
  >
    <span class="av-chip bg-white/[0.04] text-zinc-300 ring-white/10"
      >{runtime
        ? "Companion running"
        : native
          ? "Connecting to companion"
          : "Browser view · no native runtime"}</span
    >
    <span class="av-chip text-amber-200 ring-amber-400/25"
      >{runtime?.voice_ready && runtime.enrolled
        ? "Voice configured"
        : "Voice setup needed"}</span
    >
    {#each restrictions as restriction}<span class="av-chip text-amber-200 ring-amber-400/25">{restriction}</span>{/each}
  </div>
</section>

<section class="section" aria-label="Setup steps">
  <div class="flex items-center justify-between">
    <span class="av-kicker">Set up your companion</span><span
      class="caption text-zinc-500">On this PC + your Spark</span
    >
  </div>
  <div class="grid grid-cols-2 gap-2.5">
    {#each steps as step}
      <button
        class="setup-step group"
        onclick={() => navigate(step.section, step.target)}
        aria-label={`${step.action}. ${step.state}`}
      >
        <span class="flex items-center gap-2">
          <span class="caption text-zinc-500">{step.number}</span>
          <span class="text-zinc-400"><Icon name={step.icon} size={15} /></span>
          <strong class="min-w-0 flex-1 text-[12.5px] font-medium text-zinc-100"
            >{step.title}</strong
          >
          {#if step.ready}<span class="text-zinc-300" title={step.state}
              ><Icon name="check" size={14} /></span
            >{/if}
        </span>
        <span class="line-clamp-2 text-[11.5px] leading-[17px] text-zinc-400"
          >{step.detail}</span
        >
        <span class="mt-auto flex items-center justify-between gap-2 pt-1">
          <span
            class="text-[11.5px] font-medium text-av-300 group-hover:text-av-200"
            >{step.action} <span aria-hidden="true">↗</span></span
          >
          <span class="font-mono text-[9.5px] text-zinc-500">{step.state}</span>
        </span>
      </button>
    {/each}
  </div>
</section>

<div class="flex items-start gap-3 border-t border-white/[0.06] pt-3">
  <span class="mt-0.5 text-zinc-500"><Icon name="awareness" size={15} /></span>
  <p class="flex-1 text-[11.5px] leading-[17px] text-zinc-500">
    Audio and screenshots are transient. Accepted conversation history stays
    until you delete it. Sending, publishing and permission changes always need
    your approval.
  </p>
  <button
    class="av-btn av-btn-ghost av-btn-sm"
    onclick={() => navigate("profiles", "browser-setup")}
    >Browser setup <Icon name="arrow" size={12} /></button
  >
</div>

<style>
  .setup-intro {
    padding: 14px;
    background: linear-gradient(115deg, #e0115f09, transparent 65%), #ffffff03;
    border: 1px solid #ffffff14;
  }
  .setup-step {
    display: flex;
    flex-direction: column;
    gap: 7px;
    min-width: 0;
    padding: 12px;
    text-align: left;
    background: #ffffff03;
    border: 1px solid #ffffff14;
    transition:
      background 120ms,
      border-color 120ms;
  }
  .setup-step:hover {
    background: #ffffff07;
    border-color: #ffffff29;
  }
  .setup-step:focus-visible {
    outline: 1px solid var(--color-av-400);
    outline-offset: 2px;
  }
</style>
