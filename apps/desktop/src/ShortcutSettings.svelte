<script lang="ts">
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { command, native, type Chord, type Runtime, type ShortcutAction } from "./runtime";
  let { runtime }: { runtime: Runtime | null } = $props();
  type Status = { action: ShortcutAction; binding: Chord | null; state: "registered" | "disabled" | "conflict" | "unavailable" };
  const rows: { action: ShortcutAction; label: string; description: string }[] = [
    { action: "mute", label: "Microphone mute", description: "Toggle microphone listening." },
    { action: "deafen", label: "Deafen", description: "Toggle listening and assistant audio." },
    { action: "overlay", label: "Show overlay", description: "Bring Avesra's overlay into view." },
  ];
  let statuses = $state<Status[]>([]);
  let recording = $state<ShortcutAction | null>(null);
  let busy = $state(false);
  let listenerReady = $state(false);
  let error = $state("");
  let errorAction = $state<ShortcutAction | null>(null);
  let editor = $state<HTMLButtonElement>();
  let token: string | null = null;
  let generation = 0;
  let disposed = false;
  let refreshing = false;
  let statusGeneration = 0;
  let early: { id: string; chord: Chord } | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;
  function keys(chord: Chord | null | undefined) {
    if (!chord) return ["Unassigned"];
    return [chord.control ? "Ctrl" : "", chord.alt ? "Alt" : "", chord.shift ? "Shift" : "", chord.key >= 112 ? `F${chord.key - 111}` : String.fromCharCode(chord.key)].filter(Boolean);
  }
  async function refresh() {
    if (refreshing || disposed || !native || document.visibilityState !== "visible") return;
    refreshing = true;
    const mine = statusGeneration;
    try { const value = await command<Status[]>("shortcut_status"); if (!disposed && mine === statusGeneration) statuses = value; }
    catch (e) { if (!disposed && mine === statusGeneration) { statuses = []; error = String(e); } }
    finally { refreshing = false; }
  }
  function cancel() {
    generation++; recording = null; clearTimeout(timer); early = null;
    const id = token; token = null;
    if (id) void command("end_shortcut_recording", { id }).catch(() => {});
  }
  async function start(action: ShortcutAction) {
    cancel(); const mine = generation; error = ""; errorAction = action; busy = true;
    try {
      const id = await command<string>("begin_shortcut_recording");
      if (disposed || mine !== generation) { void command("end_shortcut_recording", { id }).catch(() => {}); return; }
      token = id; recording = action;
      timer = setTimeout(cancel, 10_000);
      await tick(); if (mine === generation) editor?.focus();
    } catch (e) { if (!disposed && mine === generation) error = String(e); }
    finally { busy = false; }
    const captured = early as { id: string; chord: Chord } | null; early = null;
    if (!disposed && mine === generation && captured?.id === token) void save(captured.chord);
  }
  async function save(binding: Chord | null) {
    const action = recording; if (!action || busy) return;
    cancel(); statusGeneration++; busy = true; error = ""; errorAction = action;
    try { await command("set_shortcut", { action, binding }); }
    catch (e) { if (!disposed) error = String(e); }
    finally { statusGeneration++; busy = false; await refresh(); }
  }
  function capture(event: KeyboardEvent) {
    if (!recording || busy) return;
    event.preventDefault(); event.stopPropagation();
    if (event.repeat) return;
    if (event.key === "Escape") { cancel(); return; }
    if (event.key === "Backspace" && !event.ctrlKey && !event.altKey && !event.shiftKey) { void save(null); return; }
    if (["Control", "Alt", "Shift"].includes(event.key)) return;
    // event.key represents the layout's logical Win32 key; event.code is physical.
    const key = /^[a-z0-9]$/i.test(event.key) ? event.key.toUpperCase().charCodeAt(0) : /^F([1-9]|10|11)$/.test(event.key) ? 111 + Number(event.key.slice(1)) : 0;
    if (!key || event.metaKey || event.getModifierState("AltGraph") || (!event.ctrlKey && !event.altKey)) {
      error = "Use Ctrl or Alt with A–Z, 0–9 or F1–F11. This key or layout combination is unsupported."; return;
    }
    void save({ control: event.ctrlKey, alt: event.altKey, shift: event.shiftKey, key });
  }
  $effect(() => { if (runtime?.locked) cancel(); });
  onMount(() => {
    let unlisten: (() => void) | undefined;
    void listen<{ id: string; chord: Chord }>("shortcut-recorded", event => { if (event.payload.id === token && !busy) void save(event.payload.chord); else if (busy && !recording) early = event.payload; }).then(stop => { if (disposed) stop(); else { unlisten = stop; listenerReady = true; } }).catch(e => { if (!disposed) error = String(e); });
    const blur = () => cancel();
    window.addEventListener("blur", blur);
    document.addEventListener("visibilitychange", blur);
    void refresh(); const poll = setInterval(() => { void refresh(); }, 2000);
    return () => { disposed = true; cancel(); clearInterval(poll); unlisten?.(); window.removeEventListener("blur", blur); document.removeEventListener("visibilitychange", blur); };
  });
</script>

<section class="flex flex-col gap-2">
  <span class="av-kicker">Shortcuts</span>
  <div class="av-card flex flex-col divide-y divide-white/[0.06]">
    {#each rows as row}
      {@const status = statuses.find(value => value.action === row.action)}
      <div class="flex flex-col gap-1 px-3.5 py-2.5">
        <div class="flex items-center gap-3">
          <div class="flex min-w-0 flex-1 flex-col"><span class="text-[12.5px] text-zinc-100">{row.label}</span><span class="av-hint">{row.description} {status?.state === "registered" ? "" : status?.state === "conflict" ? "Already in use by another app." : status?.state === "disabled" ? "Unassigned." : "Registration unavailable."}</span></div>
          {#if recording === row.action}
            <button bind:this={editor} type="button" class="inline-flex h-8 cursor-text items-center gap-2 bg-av-500/10 px-3 font-mono text-[11.5px] text-av-200 ring-1 ring-av-500 focus:outline-none" onkeydown={capture} aria-label="Press the new shortcut, Escape to cancel, or Backspace to remove"><span class="av-caret inline-block h-3 w-[2px] bg-av-500"></span>Press keys · Esc cancels</button>
            <button class="av-btn av-btn-ghost av-btn-sm" onclick={cancel}>Cancel</button>
          {:else}
            <span class="flex items-center gap-1">{#each keys(status?.binding ?? runtime?.settings.shortcuts[row.action]) as key}<span class="av-kbd">{key}</span>{/each}</span>
            <button class="av-btn av-btn-secondary av-btn-sm" disabled={!native || !listenerReady || !runtime || runtime.locked || busy || !!recording} onclick={() => start(row.action)}>Change</button>
          {/if}
        </div>
        {#if error && errorAction === row.action}<span class="text-[11.5px] text-red-400" role="alert">{error}</span>{/if}
      </div>
    {/each}
    <div class="flex items-center gap-3 px-3.5 py-2.5"><div class="flex min-w-0 flex-1 flex-col"><span class="text-[12.5px] text-zinc-100">Push to talk</span><span class="av-hint">Unavailable until voice capture is qualified.</span></div><span class="av-kbd">Unassigned</span><button class="av-btn av-btn-secondary av-btn-sm" disabled>Change</button></div>
  </div>
  {#if error && !errorAction}<p class="av-hint text-red-400" role="alert">{error}</p>{/if}
</section>
