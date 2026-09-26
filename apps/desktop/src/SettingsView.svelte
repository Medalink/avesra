<script lang="ts">
  import { viewTiming } from "./app-timing";
  import Performance from "./Performance.svelte";
  import Notifications from "./Notifications.svelte";
  import { onMount, tick } from "svelte";
  import Icon from "./Icon.svelte";
  import SelectFrame from "./SelectFrame.svelte";
  import SetupOverview from "./SetupOverview.svelte";
  import { personalVoiceView } from "./owner-setup";
  import { sparkConnection } from "./runtime";
  import Pairing from "./Pairing.svelte";
  import VoiceDesigner from "./VoiceDesigner.svelte";
  import VoiceAtmosphere from "./VoiceAtmosphere.svelte";
  import ShortcutSettings from "./ShortcutSettings.svelte";
  import EnrollmentView from "./EnrollmentView.svelte";
  import SetupLock from "./SetupLock.svelte";
  import MicrophoneMeter from "./MicrophoneMeter.svelte";
  import AppCatalog from "./AppCatalog.svelte";
  import ActionTasks from "./ActionTasks.svelte";
  import PrivateMemory from "./PrivateMemory.svelte";
  import HistoryPanel from "./HistoryPanel.svelte";
  import BrowserSetup from "./BrowserSetup.svelte";
  import type { SignalFrame } from "./Signal.svelte";
  import type { Runtime, Settings, AudioDevice } from "./runtime";
  import { command, native } from "./runtime";
  let {
    runtime,
    signal,
    devices,
    devicesLoading,
    devicesError,
    refreshDevices,
    error,
    notice,
    saving,
    control,
    update,
    hide,
    drag,
  }: {
    runtime: Runtime | null;
    signal: SignalFrame | null;
    devices: AudioDevice[];
    devicesLoading: boolean;
    devicesError: string;
    refreshDevices: () => Promise<void>;
    error: string;
    notice: string;
    saving: boolean;
    control: (value: string) => Promise<void>;
    update: (patch: Partial<Settings>) => Promise<void>;
    hide: () => Promise<void>;
    drag: (event: PointerEvent) => Promise<void>;
  } = $props();
  const voice = $derived(personalVoiceView(runtime));
  const spark = $derived(sparkConnection(runtime));
  function restoredSection() {
    try {
      const saved = localStorage.getItem("avesra.settings.section");
      return saved && ["audio", "models", "profiles", "people", "awareness", "memory"].includes(saved) ? saved : "audio";
    } catch { return "audio"; }
  }
  let section = $state(restoredSection());
  let content: HTMLElement;
  let heading: HTMLHeadingElement;
  let navigationTiming = 0;
  async function navigate(next: string, target?: string) {
    const serial = ++navigationTiming;
    const observed = viewTiming(next);
    section = next;
    try { localStorage.setItem("avesra.settings.section", next); } catch { /* Navigation remains usable if persistence is unavailable. */ }
    notice = "";
    await tick();
    observed?.commit(serial === navigationTiming ? "complete" : "abandoned");
    requestAnimationFrame(() => observed?.frame(serial === navigationTiming ? "complete" : "abandoned"));
    heading?.focus({ preventScroll: true });
    content?.scrollTo({ top: 0 });
    if (target) document.getElementById(target)?.scrollIntoView({ block: "start" });
  }
  let tab = $state("Memory");
  type AudioHealth = {state: string; model_revision: string; busy: boolean; streaming: boolean};
  type LaneObservation = {state: "not_configured" | "unavailable" | "incompatible"} | {state: "observed"; health: AudioHealth};
  type AudioLaneHealth = {version: 1; asr: LaneObservation; speaker: LaneObservation; tts: LaneObservation};
  let audioHealth = $state<AudioLaneHealth | null>(null);
  type ReasoningHealth = {state: "loaded_unqualified"; artifact_revision: string; engine_incarnation: string; quality_fingerprint_available: boolean};
  let reasoningHealth = $state<ReasoningHealth | null>(null);
  let reasoningError = $state("");
  let healthError = $state("");
  let probing = $state(false);
  let refreshHealth = $state<() => void>(() => {});
  const healthActive = $derived(section === "models" || (section === "memory" && tab === "Health"));
  function laneStatus(value: LaneObservation | undefined) {
    if (!value) return "Not probed";
    if (value.state === "not_configured") return "Not configured";
    if (value.state === "incompatible") return "Incompatible";
    if (value.state === "unavailable") return "Unavailable";
    if (value.state !== "observed") return "Unavailable";
    return value.health.state === "loaded_unqualified" ? "Loaded · unqualified" : value.health.state === "loading" ? "Loading" : value.health.state === "termination_pending" ? "Stopping" : "Unavailable";
  }
  const speakerStatus = $derived(laneStatus(audioHealth?.speaker));
  const healthContext = $derived(JSON.stringify({
    active: healthActive,
    connected: runtime?.connected ?? false,
    locked: runtime?.locked ?? true,
    epoch: runtime?.capture_epoch ?? 0,
    action: runtime?.action_epoch ?? 0,
    microphone: runtime?.settings.microphone ?? null,
  }));
  // Actual command ownership survives reactive context replacement. Native
  // commands may still be finishing after their publication context changes.
  let healthMounted = false;
  let healthPending = false;
  let healthQueued = false;
  let healthGeneration = 0;
  let healthEligible = false;
  function clearHealth() {
    audioHealth = null; reasoningHealth = null;
    healthError = ""; reasoningError = "";
  }
  async function probeHealth() {
    if (!healthMounted || !healthEligible || !native || document.visibilityState !== "visible") return;
    if (healthPending) { healthQueued = true; return; }
    healthPending = true; healthQueued = false; probing = true;
    clearHealth();
    const generation = healthGeneration;
    try {
      const [audio, reasoning] = await Promise.allSettled([command<AudioLaneHealth>("audio_lane_health"), command<ReasoningHealth>("reasoning_health")]);
      if (healthMounted && generation === healthGeneration && healthEligible && document.visibilityState === "visible") {
        if (audio.status === "fulfilled") audioHealth = audio.value; else healthError = String(audio.reason);
        if (reasoning.status === "fulfilled") reasoningHealth = reasoning.value; else reasoningError = String(reasoning.reason);
      }
    } finally {
      healthPending = false;
      if (healthMounted) probing = false;
      const queued = healthQueued;
      healthQueued = false;
      if (queued) void probeHealth();
    }
  }
  $effect(() => {
    const context = JSON.parse(healthContext) as {active:boolean;connected:boolean;locked:boolean;epoch:number};
    healthGeneration += 1;
    healthEligible = context.active && context.connected && !context.locked && context.epoch > 0;
    healthQueued = false;
    clearHealth();
    void probeHealth();
  });
  onMount(() => {
    healthMounted = true;
    refreshHealth = () => { void probeHealth(); };
    void probeHealth();
    const interval = setInterval(() => { void probeHealth(); }, 15000);
    const visibility = () => {
      navigationTiming++;
      healthGeneration += 1; healthQueued = false; clearHealth();
      void probeHealth();
    };
    document.addEventListener("visibilitychange", visibility);
    return () => {
      navigationTiming++;
      healthMounted = false; healthGeneration += 1; healthQueued = false;
      clearInterval(interval); document.removeEventListener("visibilitychange", visibility);
    };
  });
  const sections = [
    [
      "audio",
      "Audio & Voice",
      "Devices, how Avesra listens, shortcuts, its voice, and the small chimes it uses.",
    ],
    [
      "models",
      "Models & Drivers",
      "What runs each part of Avesra and where. Everything is local unless marked Cloud — Jev.",
    ],
    [
      "profiles",
      "Profiles & Machines",
      "Where each lane runs, resource budgets, and the machines Avesra is paired with.",
    ],
    [
      "people",
      "People & Voice ID",
      "Save your owner account and voice. See exactly what is complete and what is still needed.",
    ],
    [
      "awareness",
      "Observation & Actions",
      "What Avesra may see, what it has learned, and what always needs your OK.",
    ],
    [
      "memory",
      "Memory & Diagnostics",
      "What Avesra remembers, your request history, and how its machines and models are doing.",
    ],
  ];
  const lanes = [
    [
      "Speech recognition",
      "Transcribes speech into words.",
      "Probe the paired Spark for configured speech recognition metadata.",
    ],
    [
      "Voice identity",
      "Matches enrolled voices before accepting a request.",
      "Owner enrollment and speaker service are required.",
    ],
    [
      "Conversation & planning",
      "Plans accepted requests and responds.",
      "Inspect the paired controlled reasoning deployment without running inference. Loaded status does not grant voice or action permission.",
    ],
    [
      "Screen understanding",
      "Interprets selected, fresh screen observations.",
      "The screen-understanding driver is not integrated. External model availability is not inspected here.",
    ],
    [
      "Voice synthesis",
      "Speaks with your chosen generated voice.",
      "Select a voice after the speech service is ready.",
    ],
    [
      "Decision evaluation",
      "Chooses between typed, permitted options.",
      "The local decision driver is not integrated.",
    ],
    [
      "Memory processing",
      "Proposes sourced facts and routines.",
      "The background memory driver is not integrated.",
    ],
  ];
  const laneKeys = ["asr", "speaker", null, null, "tts", null, null] as const;
  const knownModels: Record<string, string> = {
    ebe59e5a817142986528bbbee5dba8db7b38ed50: "Nemotron Speech",
    "0f99f2d0ebe89ac095bcc5903c4dd8f72b367286": "ECAPA-TDNN",
    "5d83992436eae1d760afd27aff78a71d676296fc": "Qwen3-TTS Base",
  };
  const laneCards = $derived(lanes.map((lane, index) => {
    const key = laneKeys[index];
    const observation = key ? audioHealth?.[key] : undefined;
    const health = observation?.state === "observed" ? observation.health : null;
    if (index === 2) return {
      title: lane[0], description: lane[1], supported: true,
      status: reasoningHealth ? "Loaded · unqualified" : reasoningError ? "Unavailable · unverified" : "Not probed",
      driver: reasoningHealth ? "Controlled reasoning driver" : "Not verified",
      model: reasoningHealth ? "Observed configured artifact" : "Not verified",
      revision: reasoningHealth?.artifact_revision ?? "",
      machine: reasoningHealth ? "Paired Spark" : "Not verified",
      detail: reasoningHealth ? `${reasoningHealth.artifact_revision} · engine ${reasoningHealth.engine_incarnation} · ${reasoningHealth.quality_fingerprint_available ? "complete quality fingerprint observed" : "quality fingerprint unavailable"} · qualification required` : reasoningError || lane[2],
    };
    return {
      title: lane[0], description: lane[1], supported: !!key,
      status: key ? laneStatus(observation) : index === 2 ? "Not inspected" : "Not integrated",
      driver: health ? "Dedicated audio service" : observation?.state === "not_configured" ? "Not configured" : key ? "Not verified" : index === 2 ? "Status not integrated" : "Not integrated",
      model: health ? knownModels[health.model_revision] ?? "Configured audio model" : "Not verified",
      revision: health?.model_revision ?? "",
      machine: health ? "Paired Spark" : "Not verified",
      detail: health ? `${health.model_revision} · ${health.streaming ? "streaming advertised" : "batch only"} · ${health.busy ? "busy" : "idle"} · qualification required`
        : observation?.state === "not_configured" ? "No deployment is configured for this lane on the paired controller."
        : observation?.state === "incompatible" ? "Configured service returned incompatible metadata."
        : observation?.state === "unavailable" ? "Configured service is unavailable; probe again after checking its deployment."
        : lane[2],
    };
  }));
  const meta = $derived(section === "setup" ? ["setup", "Get started", "Your setup, one step at a time."] : sections.find((s) => s[0] === section)!);
  const s = $derived(runtime?.settings);
  const interfaceScales = [100, 110, 125, 150, 175];
</script>

<div
  class="flex h-full flex-col overflow-hidden bg-[#1f1f23] ring-1 ring-white/10"
>
  <header
    class="flex h-11 shrink-0 items-center gap-2.5 border-b border-white/[0.06] pr-2 pl-4"
  >
    <span class="text-av-400"><Icon name="brand" /></span><button
      class="flex-1 self-stretch text-left text-[13px] font-medium text-zinc-100"
      onpointerdown={drag}
      aria-label="Drag settings window">Avesra Settings</button
    >
    <button
      type="button"
      class="av-chip transition-colors hover:bg-white/10 focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-av-400 {runtime?.connected ? 'bg-white/[0.04] text-zinc-300 ring-white/10' : 'bg-red-400/5 text-red-300 ring-red-400/25'}"
      title={spark.detail}
      aria-label={`Spark ${spark.label}. ${spark.detail}`} aria-live="polite" aria-busy={spark.connecting}
      onclick={() => navigate("profiles", "spark-pairing")}
      >Spark · {spark.label}<Icon name="arrow" size={10} /></button
    ><span class="av-chip bg-white/[0.04] text-zinc-300 ring-white/10"
      >Profile · {s?.profile === "gaming" ? "Gaming" : "Single Spark"}</span
    >
    <button class="av-iconbtn size-7" onclick={hide} aria-label="Close settings"
      ><Icon name="close" size={14} /></button
    >
  </header>
  <div class="flex min-h-0 flex-1">
    <nav
      class="flex w-[200px] shrink-0 flex-col gap-0.5 border-r border-white/[0.06] bg-black/20 p-2.5"
      aria-label="Settings sections"
    >
      {#each sections as item}<button
          class="av-nav-btn"
          aria-current={section === item[0] ? "page" : undefined}
          onclick={() => navigate(item[0])}
          ><span class="grid size-5 place-items-center"
            ><Icon name={item[0]} /></span
          ><span class="flex-1">{item[1]}</span></button
        >{/each}
      <span class="flex-1"></span>
      <div class="flex flex-col gap-0.5 px-2.5 pb-1">
        <span class="font-mono text-[10.5px] text-zinc-400">Avesra 0.1 · development</span><span
          class="text-[10.5px] leading-[14px] text-zinc-400"
          >{native ? runtime ? "Native Windows companion" : "Connecting to companion…" : "Browser view · controls unavailable"}</span
        >
      </div>
    </nav>
    <main bind:this={content} class="av-scroll min-w-0 flex-1 overflow-y-auto">
      <div class="flex flex-col {section === 'setup' ? 'gap-4' : 'gap-6'} px-6 pt-5 pb-8">
        <div class="flex flex-col gap-1">
          <h1 bind:this={heading} tabindex="-1" class="text-[17px] font-semibold tracking-[-0.01em] text-zinc-50 outline-none">
            {meta[1]}
          </h1>
          {#if section !== "setup"}<p class="text-[12.5px] leading-[18px] text-zinc-400">{meta[2]}</p>{/if}
        </div>
        {#if error}<div
            class="border border-red-400/30 bg-red-500/5 p-3 text-[12px] text-red-300"
            role="alert"
          >
            {error}
          </div>{/if}
        {#if notice}<div class="text-[11.5px] text-zinc-400" role="status">
            {notice}
          </div>{/if}
        {#if section === "setup"}
          <SetupOverview {runtime} {devices} {devicesLoading} {devicesError} {navigate} />
        {:else if section === "audio"}
          <section class="section">
            <div class="flex items-center justify-between"><span class="av-kicker">Devices</span><button class="av-btn av-btn-secondary av-btn-sm" disabled={!native || devicesLoading} onclick={refreshDevices}>{devicesLoading ? "Refreshing…" : "Refresh devices"}</button></div>
            {#if devicesError}<div class="warning" role="alert">{devicesError}</div>{/if}
            <div class="grid grid-cols-2 gap-4">
              <div class="flex flex-col gap-1.5">
                <label class="av-label" for="microphone">Microphone</label
                ><SelectFrame><select
                  id="microphone"
                  class="av-input av-select"
                  value={s?.microphone ?? ""}
                  disabled={!s || saving || devicesLoading || !!devicesError}
                  onchange={(e) =>
                    update({ microphone: e.currentTarget.value || null })}
                  ><option value="">Select microphone</option>
                  {#if s?.microphone && !devices.some(d => d.direction === "input" && d.id === s.microphone)}<option value={s.microphone} disabled>Selected microphone unavailable</option>{/if}
                  {#each devices.filter((d) => d.direction === "input") as device}<option
                      value={device.id}
                      >{device.name}{device.is_default
                        ? " · default"
                        : ""}</option
                    >{/each}</select
                ></SelectFrame>
                <MicrophoneMeter {runtime} {signal} />
              </div>
              <div class="flex flex-col gap-1.5">
                <label class="av-label" for="speaker">Speakers</label><SelectFrame><select
                  id="speaker"
                  class="av-input av-select"
                  value={s?.speaker ?? ""}
                  disabled={!s || saving || devicesLoading || !!devicesError}
                  onchange={(e) =>
                    update({ speaker: e.currentTarget.value || null })}
                  ><option value="">Select output</option>
                  {#if s?.speaker && !devices.some(d => d.direction === "output" && d.id === s.speaker)}<option value={s.speaker} disabled>Selected output unavailable</option>{/if}
                  {#each devices.filter((d) => d.direction === "output") as device}<option
                      value={device.id}
                      >{device.name}{device.is_default
                        ? " · default"
                        : ""}</option
                    >{/each}</select
                ></SelectFrame><span class="av-hint"
                  >Replies and voice previews use this output. Your selected Avesra voice is kept.</span
                >
              </div>
            </div>
          </section>
          <section class="section">
            <span class="av-kicker">How Avesra listens</span>
            <div class="av-card flex flex-col gap-2 p-3" role="status">
              <span class="text-[13px] font-medium">{voice.label}</span>
              <p class="av-hint">{runtime?.enrollment_capture ? "Recording an optional voice sample. Mute or cancel recording to stop." : voice.reason}</p>
              <p class="av-hint">Talk naturally. No wake word or calibration session is required. Mute or pause whenever you want quiet.</p>
            </div>
            <div class="row">
              <div>
                <h2>Microphone mute</h2>
                <p class="av-hint">
                  Local control stays available when Spark is disconnected.
                </p>
              </div>
              <button
                class="av-btn av-btn-secondary"
                disabled={!runtime}
                onclick={() => control(s?.explicit_mute ? "unmute" : "mute")}
                >{s?.explicit_mute ? "Unmute" : "Mute"}</button
              >
            </div>
          </section>
          <ShortcutSettings {runtime} />
          <div id="voice-designer"><VoiceDesigner {runtime} /></div>
          <VoiceAtmosphere {runtime} />
          <section class="section">
            <span class="av-kicker">Learning & action chimes</span
            >{#each [["learning_chime", "Learning chime", "After a useful memory is committed."], ["action_chime", "Action chime", "After an action outcome is verified."]] as item}<div
                class="row"
              >
                <div>
                  <h2>{item[1]}</h2>
                  <p class="av-hint">{item[2]}</p>
                </div>
                <button
                  role="switch"
                  aria-label={item[1]}
                  aria-checked={!!s?.[
                    item[0] as "learning_chime" | "action_chime"
                  ]}
                  class="av-switch"
                  disabled={!s || saving}
                  onclick={() =>
                    update({
                      [item[0]]:
                        !s?.[item[0] as "learning_chime" | "action_chime"],
                    })}
                  ><span
                    class="av-knob"
                    style:transform={s?.[
                      item[0] as "learning_chime" | "action_chime"
                    ]
                      ? "translateX(18px)"
                      : "translateX(3px)"}
                  ></span></button
                >
              </div>{/each}
            {#each ["learning_chime_volume", "action_chime_volume"] as volume}
            <div class="flex items-center gap-3 px-3.5 py-2.5">
              <label class="av-label w-[120px]" for={volume}
                >{volume === "learning_chime_volume" ? "Learning volume" : "Action volume"}</label
              >
              <input
                id={volume}
                type="range"
                min="0"
                max="100"
                step="1"
                class="av-range flex-1"
                value={s?.[volume as "learning_chime_volume" | "action_chime_volume"] ?? s?.chime_volume ?? 15}
                disabled={!s || saving}
                onchange={(e) =>
                  update({ [volume]: Number(e.currentTarget.value) })}
              />
              <span
                class="w-10 text-right font-mono text-[11.5px] text-zinc-300"
                >{s?.[volume as "learning_chime_volume" | "action_chime_volume"] ?? s?.chime_volume ?? 15}%</span
              >
            </div>
            {/each}
            <p class="av-hint">
              Only committed events can sound. Learning batches coalesce at most once per minute; active speech, recording, pause and Deafen take precedence.
            </p>
          </section>
          <Notifications {runtime} />
        {:else if section === "models"}
          <div class="warning">
            {voice.label}: {voice.reason}
          </div>
          <div class="flex items-center gap-3">
            <p class="av-hint flex-1" role="status">{probing ? "Inspecting configured audio and reasoning services…" : healthError || "Audio and controlled reasoning metadata comes from the paired Spark. Probe reads status without running inference."}</p>
            <button class="av-btn av-btn-secondary av-btn-sm" disabled={!native || !runtime?.connected || runtime?.locked || probing} onclick={refreshHealth}>Refresh status</button>
          </div>
          <div class="flex flex-col gap-2">
            {#each laneCards as lane}<div class="av-card p-3">
                <div class="flex items-start gap-3">
                  <div class="flex-1">
                    <h2>{lane.title}</h2>
                    <p class="av-hint mt-0.5">{lane.description}</p>
                  </div>
                  <span
                    class="av-chip bg-amber-400/10 text-amber-200 ring-amber-400/25"
                    >{lane.status}</span
                  >
                </div>
                <div class="mt-3 grid grid-cols-[minmax(0,0.9fr)_minmax(0,1.4fr)_minmax(0,0.9fr)_auto] items-end gap-3">
                  <div class="flex min-w-0 flex-col gap-0.5">
                    <span class="text-[10.5px] text-zinc-400">Driver</span>
                    <span class="truncate text-[12.5px] text-zinc-200">{lane.driver}</span>
                  </div>
                  <div class="flex min-w-0 flex-col gap-0.5">
                    <span class="text-[10.5px] text-zinc-400">Model · limits</span>
                    <span class="truncate text-[12.5px] text-zinc-200" title={lane.revision}>{lane.model}</span>
                  </div>
                  <div class="flex min-w-0 flex-col gap-0.5">
                    <span class="text-[10.5px] text-zinc-400">Machine</span>
                    <span class="truncate text-[12.5px] text-zinc-200">{lane.machine}</span>
                  </div>
                  <button class="av-btn av-btn-ghost av-btn-sm" disabled={!lane.supported || !native || !runtime?.connected || runtime?.locked || probing} onclick={refreshHealth}>Probe</button>
                </div>
                <p class="mt-1.5 break-words font-mono text-[10.5px] text-zinc-500">{lane.detail}</p>
              </div>{/each}
          </div>
        {:else if section === "profiles"}
          <section class="section">
            <span class="av-kicker">Profiles</span>
            <div class="grid grid-cols-3 gap-2">
              {#each [["single-spark", "Single Spark", "All inference on Spark."], ["accelerated", "Accelerated", "Qualified client lanes."], ["gaming", "Gaming", "Spark inference; passive app learning deferred."]] as profile}<button
                  class="flex flex-col gap-2 p-3 text-left ring-1 ring-inset disabled:opacity-45 {s?.profile ===
                  profile[0]
                    ? 'bg-white/[0.06] ring-av-500 shadow-[inset_0_-2px_0_var(--color-av-500)]'
                    : 'bg-white/[0.02] ring-white/10'}"
                  disabled={!s || saving || profile[0] === "accelerated"}
                  aria-pressed={s?.profile === profile[0]}
                  onclick={() =>
                    update({ profile: profile[0] as Settings["profile"] })}
                  ><strong class="text-[12.5px] font-medium"
                    >{profile[1]}</strong
                  ><span class="av-hint">{profile[2]}</span></button
                >{/each}
            </div>
            <p class="av-hint">
              Single Spark and Gaming preserve your current conversation. Gaming
              stops optional passive app observation; explicit teaching remains available.
              Accelerated is unavailable until client capability and performance
              are verified.
            </p>
          </section>
          <section class="section">
            <span class="av-kicker">Machines</span>
            <div id="spark-pairing"><Pairing {runtime} /></div>
            <button type="button" class="av-btn av-btn-ghost av-btn-sm self-start" onclick={() => navigate("setup")}>Open setup guide</button>
            <div id="browser-setup"><BrowserSetup {runtime} /></div>
          </section>
          <section class="section">
            <span class="av-kicker">Display</span>
            <div class="row">
              <div>
                <h2>Interface size</h2>
                <p class="av-hint">
                  Scales text and controls in every Avesra window on this PC.
                </p>
              </div>
              <div class="av-seg" role="group" aria-label="Interface size">
                {#each interfaceScales as scale}<button
                    type="button"
                    class="av-seg-btn"
                    aria-pressed={s?.interface_scale === scale}
                    disabled={!s || saving}
                    onclick={() => update({ interface_scale: scale })}
                    >{scale}%</button
                  >{/each}
              </div>
            </div>
          </section>
          <section class="section">
            <span class="av-kicker">Overlay</span>
            <div class="row">
              <div>
                <h2>Always on top</h2>
                <p class="av-hint">
                  Keep the compact overlay above other windows.
                </p>
              </div>
              <button
                role="switch"
                aria-label="Always on top"
                aria-checked={!!s?.always_on_top}
                class="av-switch"
                disabled={!s || saving}
                onclick={() => update({ always_on_top: !s?.always_on_top })}
                ><span
                  class="av-knob"
                  style:transform={s?.always_on_top
                    ? "translateX(18px)"
                    : "translateX(3px)"}
                ></span></button
              >
            </div>
          </section>
        {:else if section === "people"}
          <SetupLock {runtime} />
          <EnrollmentView {runtime} {navigate} {control} />
          <section class="flex flex-col gap-2.5">
            <div class="flex items-center justify-between">
              <span class="av-kicker">Other people</span>
              <button type="button" class="av-btn av-btn-ghost av-btn-sm" disabled title="Additional-person enrollment is not available in this build">Enroll a person</button>
            </div>
            <div class="av-card flex flex-col gap-2.5 p-3.5">
              <span class="text-[13px] font-medium text-zinc-50">Additional people are not available yet</span>
              <p class="av-hint">Personal conversation uses the current Windows owner's voice association. Additional-person enrollment and permissions are not available in this build.</p>
            </div>
          </section>
          <section class="flex flex-col gap-2">
            <span class="av-kicker">Rules that don’t change</span>
            <div class="av-card flex flex-col divide-y divide-white/[0.06]">
              <div class="px-3.5 py-2.5 text-[12.5px] text-zinc-300">For a new Personal voice, the first accepted natural utterance is treated as the Windows owner's provisional voice. After that initial association, mismatching voices do not update it.</div>
              <div class="px-3.5 py-2.5 text-[12.5px] text-zinc-300">A voice match never grants permissions or changes ownership. Protected management requires Windows verification.</div>
              <div class="px-3.5 py-2.5 text-[12.5px] text-zinc-300">Voice ID is not secure authentication. Recordings, synthesis or illness can fool it; a voice association is not identity verification.</div>
              <div class="px-3.5 py-2.5 text-[12.5px] text-zinc-300">Raw enrollment audio stays in memory. Saved voice features are protected on this PC; selected advanced profiles are not retuned by Personal learning.</div>
            </div>
          </section>
        {:else if section === "awareness"}
          <section class="section">
            <span class="av-kicker">Observation scope</span>
            <div class="row">
              <div>
                <h2>Screen awareness</h2>
                <p class="av-hint">
                  No displays or applications are being captured.
                </p>
              </div>
              <span class="av-chip text-amber-200 ring-amber-400/30">Off</span>
            </div>
            <div class="empty">
              <h2>Choose what Avesra may see</h2>
              <p class="av-hint">
                Observation becomes available after authenticated setup.
                Passwords, secure desktops and excluded apps stay outside the
                selected scope.
              </p>
            </div>
          </section>
          <section class="section">
            <span class="av-kicker">Actions</span>
            <div class="row">
              <div>
                <h2>Pause assistant</h2>
                <p class="av-hint">
                  Stop listening, observation and new actions.
                </p>
              </div>
              <button
                class="av-btn av-btn-secondary"
                disabled={!runtime}
                onclick={() => control(s?.paused ? "resume" : "pause")}
                >{s?.paused ? "Resume" : "Pause"}</button
              >
            </div>
            <div class="line"></div>
            <h2>Always ask first</h2>
            <p class="av-hint">
              Send, publish, delete, spend, change permissions or change
              system/network settings.
            </p>
          </section>
          <AppCatalog {runtime} />
          <ActionTasks {runtime} />
          <section class="section"><span class="av-kicker">Routines</span><p class="av-hint">Save and inspect sourced one-step routines in Memory.</p></section>
        {:else if section === "memory"}
          <div
            class="av-seg self-start"
            role="tablist"
            aria-label="Memory and diagnostics"
          >
            {#each ["Memory", "History", "Performance", "Health"] as t}<button
                class="av-seg-btn"
                role="tab"
                aria-selected={tab === t}
                data-selected={tab === t}
                onclick={() => (tab = t)}>{t}</button
              >{/each}
          </div>
          {#if tab === "Memory"}<PrivateMemory {runtime} />{:else if tab === "History"}<HistoryPanel {runtime} />{:else if tab === "Performance"}<Performance {runtime} />{:else}<div class="av-card p-4">
              <div class="row">
                <h2>Local companion</h2>
                <span class="av-chip text-zinc-300 ring-white/15"
                  >{runtime ? "Running" : "Unavailable"}</span
                >
              </div>
              <div class="line my-3"></div>
              <div class="row">
                <span>Voice pipeline</span><span class="text-amber-200"
                  >{runtime?.voice_ready ? "Ready" : "Setup incomplete"}</span
                >
              </div>
              <div class="line my-3"></div>
              <div class="row"><span>Speaker service</span><span class="text-amber-200">{speakerStatus}</span></div>
              <div class="row"><span>Speech recognition</span><span class="text-amber-200">{laneStatus(audioHealth?.asr)}</span></div>
              <div class="row"><span>Voice synthesis</span><span class="text-amber-200">{laneStatus(audioHealth?.tts)}</span></div>
              <p class="av-hint mt-3" role="status">{healthError || runtime?.reason || "Local runtime unavailable."}</p>
            </div>
            <p class="av-hint">
              This view reports connected runtime state. Model provisioning is
              not assistant readiness.
            </p>{/if}
        {/if}
      </div>
    </main>
  </div>
</div>
