<script lang="ts">
  import { timing } from "./app-timing";
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import {
    getCurrentWindow,
    LogicalSize,
  } from "@tauri-apps/api/window";
  import SettingsView from "./SettingsView.svelte";
  import Overlay from "./Overlay.svelte";
  import { type SignalFrame } from "./Signal.svelte";
  let signal = $state<SignalFrame | null>(null);
  import { PlaybackSignal } from "./playback-signal";
  const playback = new PlaybackSignal();
  let outputSignal = $state<SignalFrame | null>(null);
  let signalSequence = 0;
  let signalReceivedAt = 0;
  import {
    command,
    native,
    type Runtime,
    type AudioDevice,
  } from "./runtime";
  let runtime = $state<Runtime | null>(null);
  let devices = $state<AudioDevice[]>([]);
  let devicesLoading = $state(false);
  let devicesError = $state("");
  async function refreshDevices() {
    if (!native || devicesLoading) return;
    devicesLoading = true;
    devicesError = "";
    devices = [];
    try {
      devices = await command<AudioDevice[]>("audio_devices");
    } catch (e) {
      devicesError = `Unable to refresh audio devices: ${String(e)}`;
    } finally {
      devicesLoading = false;
    }
  }
  let error = $state("");
  let notice = $state("");

  let expanded = $state(false);

  const settingsWindow =
    new URLSearchParams(location.search).get("window") === "settings";
  const s = $derived(runtime?.settings);
  const interfaceScale = $derived(s?.interface_scale);
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
  async function hide() {
    if (native) {
      try { await command("hide_window"); } catch (e) { error = String(e); }
    }
  }
  async function showSettings() {
    if (native) {
      try { await command("show_settings"); } catch (e) { error = String(e); }
    }
  }
  // Both webviews are zoomed natively to the interface size; the overlay window
  // keeps its reference geometry (440 × 124, or 370 tall expanded) times that zoom.
  // Startup sizing happens natively; this follows later setting and expand changes.
  $effect(() => {
    if (interfaceScale === undefined || !native || settingsWindow) return;
    const zoom = interfaceScale / 100;
    let current = true;
    void getCurrentWindow().setSize(
      new LogicalSize(440 * zoom, (expanded ? 370 : 124) * zoom),
    ).catch(e => { if (current) error = `Unable to resize the overlay: ${String(e)}`; });
    return () => { current = false; };
  });
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
    playback.context(next.playback_epoch, next.connected && !next.locked &&
      !next.settings.deafened && !next.settings.paused && next.settings.speaker !== null,
      next.enrolled && next.voice_ready);
    playback.expire(performance.now());
    outputSignal = playback.frame;
    if (!captureAllowed()) signal = null;
  }
  function captureAllowed() {
    return runtime && ((settingsWindow && runtime.microphone_check) ||
      (runtime.connected && ((runtime.enrolled && runtime.voice_ready) || runtime.enrollment_capture))) &&
      !runtime.locked && !runtime.settings.explicit_mute &&
      !runtime.settings.deafened && !runtime.settings.paused;
  }
  function acceptSignal(next: SignalFrame | null) {
    if (!next) { signal = null; return; }
    if (!captureAllowed() || next.captureEpoch !== runtime?.capture_epoch ||
      !Number.isSafeInteger(next.sequence) || next.sequence <= signalSequence ||
      next.kind !== "human" || next.source !== "background" ||
      !Number.isFinite(next.capturedAt) || next.capturedAt < 0 ||
      typeof next.rms !== "number" || !Number.isFinite(next.rms) || next.rms < 0 || next.rms > 1 ||
      typeof next.peak !== "number" || !Number.isFinite(next.peak) || next.peak < 0 || next.peak > 1 ||
      !Array.isArray(next.samples) || next.samples.length !== 32 ||
      next.samples.some(value => !Number.isFinite(value) || Math.abs(value) > 1)) return;
    signalSequence = next.sequence;
    signalReceivedAt = performance.now();
    signal = next;
  }
  onMount(() => {
    const readyTiming = timing({ kind: "ui_ready" }, "initialize");
    let dispose = () => {};
    let gone = false;
    let syncing = false;
    let clockGeneration = 0;
    async function syncClock() {
      if (gone || syncing || document.hidden || !native) return;
      syncing = true;
      const generation = clockGeneration;
      const start = performance.now();
      try {
        const value = await command<unknown>("playback_signal_clock");
        if (!gone && !document.hidden && generation === clockGeneration) playback.calibrate(value, start, performance.now());
      } catch {
        if (!gone && generation === clockGeneration) playback.uncalibrated();
      } finally {
        syncing = false;
        if (!gone) { playback.expire(performance.now()); outputSignal = playback.frame; }
      }
    }
    function visibility() {
      clockGeneration++;
      playback.uncalibrated();
      outputSignal = null;
      if (!document.hidden) void syncClock();
    }
    document.addEventListener("visibilitychange", visibility);
    const calibration = setInterval(() => { void syncClock(); }, 10_000);
    const expiry = setInterval(() => {
      playback.expire(performance.now());
      outputSignal = playback.frame;
      if (signal && performance.now() - signalReceivedAt > 500) signal = null;
    }, 50);
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
        const stopOutput = await listen<unknown>("playback-signal", e => {
          playback.accept(e.payload, performance.now(), !document.hidden);
          playback.expire(performance.now());
          outputSignal = playback.frame;
        });
        if (gone) {
          stopOutput();
          stop();
          stopErrors();
          stopSignal();
          return;
        }
        dispose = () => {
          stopOutput();
          stop();
          stopErrors();
          stopSignal();
        };
        acceptSnapshot(await command<Runtime>("runtime_snapshot"));
        await syncClock();
        if (settingsWindow) await refreshDevices();
        readyTiming(gone ? "withdrawn" : "complete");
      } catch (e) {
        readyTiming("failed");
        error = String(e);
      }
    })();
    return () => {
      readyTiming("abandoned");
      gone = true;
      clockGeneration++;
      playback.uncalibrated();
      clearInterval(calibration);
      document.removeEventListener("visibilitychange", visibility);
      clearInterval(expiry);
      dispose();
    };
  });
</script>

{#if settingsWindow}<SettingsView
    {runtime}
    {signal}
    {devices}
    {devicesLoading}
    {devicesError}
    {refreshDevices}
    {error}
    {notice}
    {control}
    accept={acceptSnapshot}
    {hide}
    {drag}
  />{:else}<Overlay
    bind:expanded
    {runtime}
    {error}
    signal={outputSignal ?? signal}
    {control}
    {hide}
    {drag}
    {showSettings}
  />{/if}
