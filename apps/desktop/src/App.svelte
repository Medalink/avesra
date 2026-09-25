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
    if (!runtime || next.revision >= runtime.revision) runtime = next;
  }
  onMount(() => {
    let dispose = () => {};
    let gone = false;
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
        if (gone) {
          stop();
          stopErrors();
          return;
        }
        dispose = () => {
          stop();
          stopErrors();
        };
        acceptSnapshot(await command<Runtime>("runtime_snapshot"));
        devices = await command<AudioDevice[]>("audio_devices");
      } catch (e) {
        error = String(e);
      }
    })();
    return () => {
      gone = true;
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
