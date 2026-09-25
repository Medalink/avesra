<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import {
    getCurrentWindow,
    Window,
    LogicalSize,
  } from "@tauri-apps/api/window";
  import SettingsView from "./SettingsView.svelte";
  import Overlay from "./Overlay.svelte";
  import { type SignalFrame } from "./Signal.svelte";
  let signal = $state<SignalFrame | null>(null);
  let signalSequence = 0;
  let signalReceivedAt = 0;
  import {
    command,
    native,
    type Runtime,
    type AudioDevice,
    type Settings,
  } from "./runtime";
  let runtime = $state<Runtime | null>(null);
  let devices = $state<AudioDevice[]>([]);
  let error = $state("");
  let notice = $state("");
  let saving = $state(false);

  let expanded = $state(false);

  const settingsWindow =
    new URLSearchParams(location.search).get("window") === "settings";
  const s = $derived(runtime?.settings);
  async function control(value: string) {
    error = "";
    try {
      acceptSnapshot(
        await command<Runtime>("local_control", { control: value }),
      );
    } catch (e) {
      error = String(e);
    }
  }
  async function update(patch: Partial<Settings>) {
    if (!s || saving) return;
    saving = true;
    error = "";
    notice = "";
    try {
      acceptSnapshot(
        await command<Runtime>("save_settings", {
          settings: { ...s, ...patch },
        }),
      );
      notice = "Preferences saved on this PC.";
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }
  async function hide() {
    if (native) await getCurrentWindow().hide();
  }
  async function showSettings() {
    if (native) {
      const w = await Window.getByLabel("settings");
      await w?.show();
      await w?.setFocus();
    }
  }
  async function expand() {
    expanded = !expanded;
    if (native)
      await getCurrentWindow().setSize(
        new LogicalSize(440, expanded ? 370 : 124),
      );
  }
  async function drag(event: PointerEvent) {
    if (native && event.button === 0) await getCurrentWindow().startDragging();
  }
  function acceptSnapshot(next: Runtime) {
    if (runtime && next.revision < runtime.revision) return;
    if (!runtime || next.capture_epoch !== runtime.capture_epoch) {
      signal = null;
      signalSequence = 0;
    }
    runtime = next;
    if (!captureAllowed()) signal = null;
  }
  function captureAllowed() {
    return runtime?.connected && runtime.enrolled && runtime.voice_ready &&
      !runtime.locked && !runtime.settings.explicit_mute &&
      !runtime.settings.deafened && !runtime.settings.paused;
  }
  function acceptSignal(next: SignalFrame | null) {
    if (!next) { signal = null; return; }
    if (!captureAllowed() || next.captureEpoch !== runtime?.capture_epoch ||
      !Number.isSafeInteger(next.sequence) || next.sequence <= signalSequence ||
      next.kind !== "human" || next.source !== "background" ||
      !Number.isFinite(next.capturedAt) || next.capturedAt < 0 ||
      !Array.isArray(next.samples) || next.samples.length !== 32 ||
      next.samples.some(value => !Number.isFinite(value) || Math.abs(value) > 1)) return;
    signalSequence = next.sequence;
    signalReceivedAt = performance.now();
    signal = next;
  }
  onMount(() => {
    let dispose = () => {};
    let gone = false;
    const expiry = setInterval(() => {
      if (signal && performance.now() - signalReceivedAt > 500) signal = null;
    }, 100);
    (async () => {
      try {
        if (!native) {
          error = "Browser preview — native connection unavailable.";
          return;
        }
        const stop = await listen<Runtime>("runtime-state", (e) =>
          acceptSnapshot(e.payload),
        );
        const stopErrors = await listen<string>("runtime-error", (e) => {
          error = e.payload;
        });
        const stopSignal = await listen<SignalFrame | null>("signal-frame", e => acceptSignal(e.payload));
        if (gone) {
          stop();
          stopErrors();
          stopSignal();
          return;
        }
        dispose = () => {
          stop();
          stopErrors();
          stopSignal();
        };
        acceptSnapshot(await command<Runtime>("runtime_snapshot"));
        devices = await command<AudioDevice[]>("audio_devices");
      } catch (e) {
        error = String(e);
      }
    })();
    return () => {
      gone = true;
      clearInterval(expiry);
      dispose();
    };
  });
</script>

{#if settingsWindow}<SettingsView
    {runtime}
    {devices}
    {error}
    {notice}
    {saving}
    {control}
    {update}
    {hide}
    {drag}
  />{:else}<Overlay
    {runtime}
    {error}
    {signal}
    {control}
    {hide}
    {drag}
    {showSettings}
  />{/if}
