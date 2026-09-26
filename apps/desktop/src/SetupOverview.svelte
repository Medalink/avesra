<script lang="ts">
  import Icon from "./Icon.svelte";
  import { native, type Runtime, type AudioDevice } from "./runtime";
  import { personalVoiceView } from "./owner-setup";

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

  const voice = $derived(personalVoiceView(runtime));
  let selected = $state("01");
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
      label: "Spark connection",
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
      title: "Speech services",
      label: "Speech services",
      state: "Review status",
      ready: false,
      detail: "Check the services that listen, reason and speak.",
      action: "Review services",
      section: "models",
    },
    {
      number: "03",
      icon: "audio",
      title: "Choose your devices",
      label: "Audio devices",
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
      title: "Your voice",
      label: "Your voice",
      state: voice.label,
      ready:
        !!runtime?.voice_ready &&
        runtime?.personal_voice?.state === "listening",
      detail:
        "Personal voice learning starts automatically when its native requirements are available. No calibration step is required here.",
      action: "Voice status",
      section: "people",
    },
    {
      number: "05",
      icon: "mic",
      title: "Give Avesra a voice",
      label: "Avesra's voice",
      state: "Optional",
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
      label: "Actions & access",
      state: "Optional",
      ready: false,
      detail: "Review observation, app aliases and browser access.",
      action: "Review actions",
      section: "awareness",
    },
  ]);
  const current = $derived(
    steps.find((step) => step.number === selected) ?? steps[0],
  );
  const restrictions = $derived([
    ...(runtime?.locked ? ["Windows locked"] : []),
    ...(runtime?.settings.paused ? ["Assistant paused"] : []),
    ...(runtime?.settings.deafened ? ["Deafened"] : []),
    ...(runtime?.settings.explicit_mute ? ["Microphone muted"] : []),
  ]);
</script>

<section
  class="flex flex-col gap-2.5"
  aria-label="Companion status"
  aria-live="polite"
>
  <div class="flex flex-wrap items-center gap-2">
    <span class="av-kicker">Personal voice</span>
    <span class="av-chip text-amber-200 ring-amber-400/25">{voice.label}</span>
    {#each restrictions as restriction}<span
        class="av-chip text-amber-200 ring-amber-400/25">{restriction}</span
      >{/each}
  </div>
  <p class="text-[12.5px] leading-[18px] text-zinc-400">{voice.reason}</p>
  {#if !runtime}<p class="text-[11.5px] text-zinc-500">
      {native ? "Connecting to companion" : "Browser view · no native runtime"}
    </p>{/if}
</section>

<section
  class="flex min-w-0 overflow-hidden ring-1 ring-inset ring-white/10"
  aria-label="Setup guide"
>
  <nav
    aria-label="Setup topics"
    class="flex w-[200px] shrink-0 flex-col border-r border-white/[0.06] bg-black/20 p-2.5"
  >
    <span class="av-kicker px-2.5 pt-1 pb-2">Setup</span>
    <ol class="m-0 flex list-none flex-col gap-0.5 p-0">
      {#each steps as step}
        <li>
          <button
            type="button"
            class="flex h-9 w-full items-center gap-2.5 px-2.5 text-left text-[13px] outline-offset-[-1px] focus-visible:outline-1 focus-visible:outline-av-400 {current.number ===
            step.number
              ? 'bg-white/[0.08] text-white shadow-[inset_2px_0_0_var(--color-av-500)]'
              : step.ready
                ? 'text-zinc-300 hover:bg-white/[0.04]'
                : 'text-zinc-400 hover:bg-white/[0.04]'}"
            aria-current={current.number === step.number ? "step" : undefined}
            aria-controls="setup-review"
            aria-label={`${step.label}. ${step.state}`}
            onclick={() => (selected = step.number)}
          >
            <span
              class="grid size-5 shrink-0 place-items-center font-mono text-[10.5px] {step.ready
                ? 'bg-av-500/20 text-av-300'
                : current.number === step.number
                  ? 'bg-av-700 text-white'
                  : 'text-zinc-500 ring-1 ring-white/12 ring-inset'}"
              aria-hidden="true"
            >
              {#if step.ready}<Icon name="check" size={11} />{:else}{Number(
                  step.number,
                )}{/if}
            </span>
            <span>{step.label}</span>
          </button>
        </li>
      {/each}
    </ol>
    <div class="mt-auto flex flex-col gap-1 px-2.5 pt-6 pb-1">
      <span class="font-mono text-[10.5px] text-zinc-400"
        >On this PC + your Spark</span
      >
      <span class="text-[10.5px] leading-[14px] text-zinc-400"
        >Review any topic. These are not required steps.</span
      >
    </div>
  </nav>
  <div
    id="setup-review"
    class="flex min-w-0 flex-1 flex-col"
    aria-labelledby="setup-review-title"
  >
    <div class="flex flex-1 flex-col gap-5 px-7 pt-5 pb-4">
      <div class="flex flex-col gap-1.5">
        <span class="av-kicker">Setup guide</span>
        <h2
          id="setup-review-title"
          class="m-0 text-[22px] font-semibold tracking-[-0.01em]"
        >
          {current.title}
        </h2>
      </div>
      <div class="av-card flex flex-col gap-2 px-4 py-3">
        <div class="flex items-center gap-2">
          <span class="text-zinc-400"
            ><Icon name={current.icon} size={16} /></span
          >
          <span class="av-label">{current.state}</span>
        </div>
        <p class="m-0 break-words text-[13.5px] leading-5 text-zinc-400">
          {current.detail}
        </p>
      </div>
      {#if current.number === "03" && devicesError}<p
          class="av-hint break-words text-amber-200"
          role="status"
        >
          {devicesError}
        </p>{/if}
      <button
        type="button"
        class="av-btn av-btn-primary self-start"
        onclick={() => navigate(current.section, current.target)}
      >
        {current.action}
        <Icon name="arrow" size={13} />
      </button>
    </div>
    <div
      class="flex flex-wrap items-center gap-2 border-t border-white/[0.06] px-7 py-3"
    >
      <span class="min-w-0 flex-1 text-[11.5px] leading-[17px] text-zinc-500"
        >Opening a control does not change your permissions.</span
      >
      <button
        type="button"
        class="av-btn av-btn-ghost av-btn-sm"
        onclick={() => navigate("profiles", "browser-setup")}
        >Browser setup <Icon name="arrow" size={12} /></button
      >
    </div>
  </div>
</section>

<p class="text-[11.5px] leading-[17px] text-zinc-500">
  Audio and screenshots are transient. Accepted conversation history stays until
  you delete it. Sending, publishing and permission changes always need your
  approval.
</p>
