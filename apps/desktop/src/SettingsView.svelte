<script lang="ts">
  import Icon from "./Icon.svelte";
  import Pairing from "./Pairing.svelte";
  import SetupLock from "./SetupLock.svelte";
  import type { Runtime, Settings, AudioDevice } from "./runtime";
  let {
    runtime,
    devices,
    error,
    notice,
    saving,
    control,
    update,
    hide,
    drag,
  }: {
    runtime: Runtime | null;
    devices: AudioDevice[];
    error: string;
    notice: string;
    saving: boolean;
    control: (value: string) => Promise<void>;
    update: (patch: Partial<Settings>) => Promise<void>;
    hide: () => Promise<void>;
    drag: (event: PointerEvent) => Promise<void>;
  } = $props();
  let section = $state("audio");
  let tab = $state("Memory");
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
      "Who Avesra recognizes and what each person may ask it to do.",
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
      "Streaming ASR service is not configured.",
    ],
    [
      "Voice identity",
      "Matches enrolled voices before accepting a request.",
      "Owner enrollment and speaker service are required.",
    ],
    [
      "Conversation & planning",
      "Plans accepted requests and responds.",
      "Pair Spark and probe a reasoning deployment.",
    ],
    [
      "Screen understanding",
      "Interprets selected, fresh screen observations.",
      "Pair Spark and probe a vision deployment.",
    ],
    [
      "Voice synthesis",
      "Speaks with your chosen generated voice.",
      "Select a voice after the speech service is ready.",
    ],
    [
      "Decision evaluation",
      "Chooses between typed, permitted options.",
      "Local decision driver is not connected.",
    ],
    [
      "Memory processing",
      "Proposes sourced facts and routines.",
      "Background memory driver is not connected.",
    ],
  ];
  const meta = $derived(sections.find((s) => s[0] === section)!);
  const s = $derived(runtime?.settings);
</script>

<div
  class="flex h-full flex-col overflow-hidden bg-[#1f1f23] ring-1 ring-white/10"
>
  <header
    class="flex h-11 shrink-0 items-center gap-2.5 border-b border-white/[0.06] pr-2 pl-4"
  >
    <span class="text-av-400"><Icon name="audio" /></span><button
      class="flex-1 self-stretch text-left text-[13px] font-medium"
      onpointerdown={drag}
      aria-label="Drag settings window">Avesra Settings</button
    >
    <span class="av-chip bg-amber-400/10 text-amber-200 ring-amber-400/25"
      >Spark · {runtime?.connected ? "connected" : "disconnected"}</span
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
          onclick={() => {
            section = item[0];
            notice = "";
          }}
          ><span class="grid size-5 place-items-center"
            ><Icon name={item[0]} /></span
          ><span class="flex-1">{item[1]}</span></button
        >{/each}
      <span class="flex-1"></span>
      <div class="flex flex-col gap-0.5 px-2.5 pb-1">
        <span class="caption text-zinc-400">Avesra 0.1 · development</span><span
          class="text-[10.5px] leading-[14px] text-zinc-400"
          >Setup is incomplete.</span
        >
      </div>
    </nav>
    <main class="av-scroll min-w-0 flex-1 overflow-y-auto">
      <div class="flex flex-col gap-6 px-6 pt-5 pb-8">
        <div class="flex flex-col gap-1">
          <h1 class="text-[17px] font-semibold tracking-[-0.01em] text-zinc-50">
            {meta[1]}
          </h1>
          <p class="text-[12.5px] leading-[18px] text-zinc-400">{meta[2]}</p>
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
        {#if section === "audio"}
          <section class="section">
            <span class="av-kicker">Devices</span>
            <div class="grid grid-cols-2 gap-4">
              <div class="flex flex-col gap-1.5">
                <label class="av-label" for="microphone">Microphone</label
                ><select
                  id="microphone"
                  class="av-input"
                  value={s?.microphone ?? ""}
                  disabled={!s || saving}
                  onchange={(e) =>
                    update({ microphone: e.currentTarget.value || null })}
                  ><option value="">Select microphone</option
                  >{#each devices.filter((d) => d.direction === "input") as device}<option
                      value={device.name}
                      >{device.name}{device.is_default
                        ? " · default"
                        : ""}</option
                    >{/each}</select
                >
                <div class="flex items-center gap-2">
                  <div
                    class="flex flex-1 gap-0.5"
                    aria-label="Input level unavailable"
                  >
                    {#each Array(24) as _}<span class="h-2 flex-1 bg-white/10"
                      ></span>{/each}
                  </div>
                  <span class="caption text-zinc-500">off</span>
                </div>
              </div>
              <div class="flex flex-col gap-1.5">
                <label class="av-label" for="speaker">Speakers</label><select
                  id="speaker"
                  class="av-input"
                  value={s?.speaker ?? ""}
                  disabled={!s || saving}
                  onchange={(e) =>
                    update({ speaker: e.currentTarget.value || null })}
                  ><option value="">Select output</option
                  >{#each devices.filter((d) => d.direction === "output") as device}<option
                      value={device.name}
                      >{device.name}{device.is_default
                        ? " · default"
                        : ""}</option
                    >{/each}</select
                ><span class="av-hint"
                  >Playback is unavailable until voice setup.</span
                >
              </div>
            </div>
          </section>
          <section class="section">
            <span class="av-kicker">How Avesra listens</span>
            <div class="grid grid-cols-3 gap-2">
              {#each [["Continuous", "No wake word after enrollment"], ["Wake phrase", "Optional listening mode"], ["Push to talk", "Hold a keyboard shortcut"]] as mode, i}<div
                  class="flex flex-col gap-1 p-3 ring-1 ring-inset {i === 0
                    ? 'bg-white/[0.06] ring-av-500 shadow-[inset_0_-2px_0_var(--color-av-500)]'
                    : 'bg-white/[0.02] ring-white/10'}"
                >
                  <span class="text-[12.5px] font-medium">{mode[0]}</span><span
                    class="av-hint">{mode[1]}</span
                  >
                </div>{/each}
            </div>
            <div class="warning">
              Listening is off. Pair Spark, prepare the speech services and
              enroll your voice to begin.
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
          <section class="section">
            <span class="av-kicker">Shortcuts</span>
            {#each [["Microphone mute", "Ctrl + Shift + M"], ["Deafen", "Ctrl + Shift + D"], ["Show overlay", "Ctrl + Shift + A"], ["Push to talk", "Ctrl + Space"]] as shortcut}
              <div class="row">
                <span class="av-label">{shortcut[0]}</span><span
                  class="av-kbd opacity-50">{shortcut[1]}</span
                >
              </div>
            {/each}
            <p class="av-hint">
              Suggested shortcuts. Global registration is not available yet; use
              the overlay and tray controls.
            </p>
          </section>
          <section class="section">
            <span class="av-kicker">Avesra's voice</span>
            <div class="av-card p-3">
              <div class="row">
                <div>
                  <h2>No voice selected</h2>
                  <p class="av-hint mt-1">
                    Create and preview a voice when synthesis is ready.
                  </p>
                </div>
                <span class="av-chip text-amber-200 ring-amber-400/30"
                  >Unavailable</span
                >
              </div>
            </div>
            <div class="grid grid-cols-2 gap-4">
              <label class="flex flex-col gap-2"
                ><span class="av-label"
                  >Pace <span class="caption text-zinc-400"
                    >{s?.speech_rate ?? 100}%</span
                  ></span
                ><input
                  aria-label="Speech pace"
                  type="range"
                  min="50"
                  max="200"
                  value={s?.speech_rate ?? 100}
                  disabled={!s || saving}
                  onchange={(e) =>
                    update({ speech_rate: Number(e.currentTarget.value) })}
                /></label
              ><label class="flex flex-col gap-2"
                ><span class="av-label"
                  >Voice volume <span class="caption text-zinc-400"
                    >{s?.speech_volume ?? 80}%</span
                  ></span
                ><input
                  aria-label="Voice volume"
                  type="range"
                  min="0"
                  max="100"
                  value={s?.speech_volume ?? 80}
                  disabled={!s || saving}
                  onchange={(e) =>
                    update({ speech_volume: Number(e.currentTarget.value) })}
                /></label
              >
            </div>
          </section>
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
                      ? "translateX(19px)"
                      : "translateX(3px)"}
                  ></span></button
                >
              </div>{/each}
            <div class="flex items-center gap-3 px-3.5 py-2.5">
              <label class="av-label w-[120px]" for="chimevol"
                >Chime volume</label
              >
              <input
                id="chimevol"
                type="range"
                min="0"
                max="100"
                step="1"
                class="av-range flex-1"
                value={s?.chime_volume ?? 15}
                disabled={!s || saving}
                onchange={(e) =>
                  update({ chime_volume: Number(e.currentTarget.value) })}
              />
              <span
                class="w-10 text-right font-mono text-[11.5px] text-zinc-300"
                >{s?.chime_volume ?? 15}%</span
              >
            </div>
            <p class="av-hint">
              Preferences are saved; chime playback is not yet available.
            </p>
          </section>
        {:else if section === "models"}
          <div class="warning">
            No inference deployment is paired. Model downloads alone do not
            establish readiness.
          </div>
          <div class="flex flex-col gap-2">
            {#each lanes as lane}<div class="av-card p-3">
                <div class="flex items-start gap-3">
                  <div class="flex-1">
                    <h2>{lane[0]}</h2>
                    <p class="av-hint mt-0.5">{lane[1]}</p>
                  </div>
                  <span
                    class="av-chip bg-amber-400/10 text-amber-200 ring-amber-400/25"
                    >Unavailable</span
                  >
                </div>
                <div class="mt-3 grid grid-cols-3 gap-3 text-[12px]">
                  <div>
                    <span class="av-hint">Driver</span>
                    <p>Not connected</p>
                  </div>
                  <div>
                    <span class="av-hint">Model</span>
                    <p>Not configured</p>
                  </div>
                  <div>
                    <span class="av-hint">Machine</span>
                    <p>Single Spark</p>
                  </div>
                </div>
                <p class="mt-2 text-[11px] text-zinc-500">{lane[2]}</p>
              </div>{/each}
          </div>
        {:else if section === "profiles"}
          <section class="section">
            <span class="av-kicker">Profiles</span>
            <div class="grid grid-cols-3 gap-2">
              {#each [["single-spark", "Single Spark", "All inference on Spark."], ["accelerated", "Accelerated", "Qualified client lanes."], ["gaming", "Gaming", "No client inference."]] as profile}<button
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
              Accelerated is unavailable until client capability and performance
              are verified.
            </p>
          </section>
          <section class="section">
            <span class="av-kicker">Machines</span>
            <Pairing connected={!!runtime?.connected} />
            <div class="av-card p-4">
              <div class="row">
                <div>
                  <h2>This PC</h2>
                  <p class="av-hint mt-1">
                    Audio, local controls and desktop tools
                  </p>
                </div>
                <span class="av-chip text-zinc-300 ring-white/15">Local</span>
              </div>
              <p class="av-hint mt-3">Client inference is disabled.</p>
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
                    ? "translateX(19px)"
                    : "translateX(3px)"}
                ></span></button
              >
            </div>
          </section>
        {:else if section === "people"}
          <SetupLock {runtime} />
          <section class="section">
            <span class="av-kicker">Owner</span>
            <div class="empty">
              <div class="row">
                <h2>No owner enrolled</h2>
                <span class="av-chip text-amber-200 ring-amber-400/30"
                  >Setup required</span
                >
              </div>
              <p class="av-hint">
                Pair Spark and prepare the speaker identity service before
                enrolling. Enrollment uses prompted and held-out speech.
              </p>
              <button class="av-btn av-btn-primary self-start" disabled
                >Enroll owner</button
              >
              <p class="av-hint">
                Enrollment is unavailable until identity and local
                authentication are ready.
              </p>
            </div>
          </section>
          <section class="section">
            <span class="av-kicker">Other people</span>
            <p class="av-hint">
              No additional people are enrolled. Only the authenticated owner
              can add people or change permissions.
            </p>
          </section>
          <div class="warning">
            A voice match alone cannot change ownership or approve high-impact
            actions.
          </div>
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
          <section class="section">
            <span class="av-kicker">Learned aliases & routines</span>
            <p class="av-hint">No aliases or routines have been learned.</p>
          </section>
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
          {#if tab === "Memory"}<div class="empty">
              <h2>No memories yet</h2>
              <p class="av-hint">
                Useful facts and routines will appear with their source and
                date. Accepted history is retained until you delete it; raw
                audio and screenshots are transient.
              </p>
            </div>{:else if tab === "History"}<div class="empty">
              <h2>No accepted requests</h2>
              <p class="av-hint">
                History begins after owner setup. Unknown voices are not saved
                as conversations.
              </p>
            </div>{:else if tab === "Performance"}<div
              class="grid grid-cols-3 gap-3"
            >
              {#each ["Median", "p95", "p99"] as metric}<div
                  class="av-card p-4"
                >
                  <span class="av-kicker">{metric}</span>
                  <p class="mt-2 font-mono text-xl text-zinc-400">—</p>
                  <p class="av-hint mt-1">No measured requests</p>
                </div>{/each}
            </div>
            <div class="empty">
              <h2>Measurements unavailable</h2>
              <p class="av-hint">
                Per-stage latency, queue time, errors and sample counts will
                appear after the pipeline is connected. No performance result is
                simulated.
              </p>
            </div>{:else}<div class="av-card p-4">
              <div class="row">
                <h2>Local companion</h2>
                <span class="av-chip text-zinc-300 ring-white/15"
                  >{runtime ? "Running" : "Unavailable"}</span
                >
              </div>
              <div class="line my-3"></div>
              <div class="row">
                <span>Spark transport</span><span class="text-amber-200"
                  >{runtime?.connected
                    ? "Connected · owner setup required"
                    : "Disconnected"}</span
                >
              </div>
              <div class="line my-3"></div>
              <div class="row">
                <span>Voice pipeline</span><span class="text-amber-200"
                  >Unavailable</span
                >
              </div>
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
