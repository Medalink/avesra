<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { command, native, type Runtime } from "./runtime";
  import { ensureManagementVerification } from "./setup";
  import type { RegistrationView } from "./owner-setup";
  let { runtime, ownerReady, parentBusy, onstate, onbusy }: {
    runtime: Runtime | null; ownerReady: boolean; parentBusy: boolean;
    onstate: (value: RegistrationView) => void; onbusy: (value: boolean) => void;
  } = $props();
  type Binding = { device: string; actor: string; owner_revision: string; registration_revision: string; registered_by: string; revoked: boolean };
  type Status = { state: "unregistered" | "different_owner" | "revoked" | "registered" | "unreconciled"; intent: "saved" | "missing" | "unavailable"; binding: Binding | null };
  let status = $state<Status | null>(null), busy = $state(false), error = $state("");
  let operation = $state<"refresh" | "register" | "revoke" | null>(null);
  let progress = $state("");
  let lifetimeReady = $state(false), pageVisible = $state(false), refreshNeeded = $state(true);
  let mounted = false, generation = 0, context = "";
  let captureContext: number | undefined;
  const enabled = $derived(native && lifetimeReady && pageVisible && ownerReady && !!runtime?.connected && !runtime.locked && !runtime.settings.paused);
  const waiting = $derived(!ownerReady ? "Create your owner account first." : !runtime?.connected ? "Connect your Spark to check this step." : runtime.locked ? "Unlock Windows to continue." : runtime.settings.paused ? "Resume Avesra to check this step." : !pageVisible ? "Open Settings to check registration automatically." : "Waiting for the current setup action to finish.");
  const detail = $derived(error ? `Couldn't check owner registration. ${error}` : !enabled ? waiting : refreshNeeded && parentBusy && !busy ? "Waiting for the current recording or setup action to finish. Registration will refresh automatically." : busy || refreshNeeded ? "Checking whether your owner is already registered…" : status?.state === "registered" ? "Your Spark remembers this owner. This step is complete." : status?.state === "unregistered" ? "Register your owner account on this Spark. Windows will ask you to verify if needed." : status?.state === "revoked" ? "This owner's registration was revoked. Recovery for a revoked registration is not available in this build." : status?.state === "different_owner" ? "This pairing belongs to a different owner. Check that you chose the intended Spark in Profiles & Machines. This page cannot replace that owner." : "Spark has this owner, but its record does not match the registration saved on this PC. This build cannot repair that mismatch automatically. Re-recording your voice will not fix it.");
  const label = $derived(!enabled ? "Waiting" : busy || refreshNeeded ? "Checking…" : error ? "Check failed" : status?.state === "registered" ? "Complete" : status?.state === "unregistered" ? "Action needed" : "Needs attention");
  function invalidate() { generation++; status = null; error = ""; refreshNeeded = true; }
  $effect(() => {
    const next = `${ownerReady}:${runtime?.connected}:${runtime?.locked}:${runtime?.settings.paused}:${runtime?.action_epoch}`;
    if (next !== context || ((!busy || operation === "refresh") && captureContext !== runtime?.capture_epoch)) { context = next; invalidate(); }
    captureContext = runtime?.capture_epoch;
  });
  $effect(() => {
    if (enabled && !busy && !parentBusy && refreshNeeded) untrack(() => void operate("refresh"));
  });
  $effect(() => { onstate({ state: !enabled ? "waiting" : busy || refreshNeeded ? "loading" : error ? "error" : status?.state ?? "error", detail }); });
  async function operate(kind: "refresh" | "register" | "revoke") {
    if (!enabled || busy || parentBusy) return;
    const revision = status?.binding?.registration_revision;
    if (kind === "revoke" && (!revision || status?.state === "different_owner" || status?.binding?.revoked)) return;
    if (kind === "register" && (status?.state !== "unregistered" || status.intent === "unavailable")) return;
    const current = ++generation;
    refreshNeeded = false; busy = true; operation = kind; error = ""; onbusy(true);
    try {
      if (kind !== "refresh") await ensureManagementVerification(() => mounted && current === generation && enabled, message => progress = message);
      if (!mounted || current !== generation || !enabled) return;
      progress = kind === "refresh" ? "Checking Spark registration…" : kind === "register" ? "Registering your owner…" : "Removing owner registration…";
      const next = await command<Status>(kind === "refresh" ? "actor_registration_status" : kind === "register" ? "register_owner_with_spark" : "revoke_owner_registration", kind === "revoke" ? {registrationRevision: revision} : undefined);
      if (mounted && current === generation) status = next;
    } catch (e) { if (mounted && current === generation) { status = null; error = String(e); } }
    finally { busy = false; operation = null; progress = ""; onbusy(false); }
  }
  onMount(() => {
    mounted = true;
    let visibilityVersion = 0;
    const cleanup: (() => void)[] = [];
    const retain = (stop: () => void) => { if (mounted) cleanup.push(stop); else stop(); };
    if (native) void (async () => {
      const window = getCurrentWindow();
      retain(await listen("settings-hidden", () => { visibilityVersion++; pageVisible = false; invalidate(); }));
      retain(await window.onFocusChanged(event => { if (event.payload && mounted) { visibilityVersion++; pageVisible = true; } }));
      const readVersion = visibilityVersion;
      const visible = await window.isVisible();
      if (mounted) { if (readVersion === visibilityVersion) pageVisible = visible; lifetimeReady = true; }
    })().catch(() => { if (mounted) { error = "Reopen this page to check registration."; refreshNeeded = false; } });
    return () => { mounted = false; generation++; cleanup.forEach(stop => stop()); };
  });
</script>

<div class="flex flex-col gap-2 border-t border-white/10 py-3">
  <div class="flex items-center gap-3"><span class="font-mono text-xs text-zinc-400">02</span><span class="min-w-0 flex-1 text-[13px] font-medium text-zinc-100">Register owner on Spark</span><span class="av-chip {status?.state === 'registered' && !busy && !refreshNeeded ? 'text-emerald-300 ring-emerald-400/30' : 'text-zinc-300 ring-white/15'}">{label}</span></div>
  <p class="av-hint ml-7" role="status">{progress || detail}</p>
  {#if enabled && !busy && !refreshNeeded && status?.state !== "registered"}
    <div class="ml-7 flex flex-wrap gap-2">
      {#if status?.state === "unregistered" && status.intent !== "unavailable"}<button class="av-btn av-btn-primary av-btn-sm" disabled={parentBusy} onclick={() => operate("register")}>Register owner on Spark</button>{:else if error || !status || status.intent === "unavailable"}<button class="av-btn av-btn-secondary av-btn-sm" disabled={parentBusy} onclick={() => operate("refresh")}>Retry registration check</button>{/if}
    </div>
  {/if}
  {#if status?.binding && !status.binding.revoked && status.state !== "different_owner" && !busy && !refreshNeeded}
    <details class="ml-7"><summary class="av-hint cursor-pointer">Registration options</summary><div class="mt-2 flex flex-wrap gap-2"><button class="av-btn av-btn-ghost av-btn-sm" disabled={parentBusy} onclick={() => operate("refresh")}>Check again</button><button class="av-btn av-btn-ghost av-btn-sm" disabled={parentBusy} onclick={() => operate("revoke")}>Remove registration…</button></div><p class="av-hint mt-2">Removing this registration requires Windows verification and cannot be undone here.</p></details>
  {/if}
  {#if status?.intent === "unavailable"}<p class="av-hint ml-7 text-amber-200">The local registration record could not be read. Registration changes are blocked; use Retry registration check.</p>{/if}
</div>
