<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { command, native, type Runtime } from "./runtime";
  let { runtime, ownerReady, parentBusy }: { runtime: Runtime | null; ownerReady: boolean; parentBusy: boolean } = $props();
  type Binding = { device: string; actor: string; owner_revision: string; registration_revision: string; registered_by: string; revoked: boolean };
  type Status = { state: "unregistered" | "different_owner" | "revoked" | "registered" | "unreconciled"; intent: "saved" | "missing" | "unavailable"; binding: Binding | null };
  let status = $state<Status | null>(null), busy = $state(false), error = $state("");
  let lifetimeReady = $state(false);
  let mounted = false, generation = 0, context = "";
  const enabled = $derived(native && lifetimeReady && ownerReady && !!runtime?.connected && !runtime.locked && !runtime.settings.paused);
  const label = $derived(!status ? "Not inspected" : status.state === "registered" ? "Registered" : status.state === "unregistered" ? "Not registered" : status.state === "revoked" ? "Revoked" : status.state === "different_owner" ? "Different owner" : "Needs review");
  function invalidate() { generation++; status = null; error = ""; }
  $effect(() => {
    const next = `${ownerReady}:${runtime?.connected}:${runtime?.locked}:${runtime?.action_epoch}:${runtime?.capture_epoch}`;
    if (next !== context) { context = next; invalidate(); }
  });
  async function operate(kind: "refresh" | "register" | "revoke") {
    if (!enabled || busy || parentBusy) return;
    const revision = status?.binding?.registration_revision;
    if (kind === "revoke" && (!revision || status?.state === "different_owner" || status?.binding?.revoked)) return;
    if (kind === "register" && (status?.state !== "unregistered" || status.intent === "unavailable")) return;
    const current = ++generation;
    busy = true; error = "";
    try {
      const next = await command<Status>(kind === "refresh" ? "actor_registration_status" : kind === "register" ? "register_owner_with_spark" : "revoke_owner_registration", kind === "revoke" ? {registrationRevision: revision} : undefined);
      if (mounted && current === generation) status = next;
    } catch (e) {
      if (mounted && current === generation) { status = null; error = String(e); }
    } finally { busy = false; }
  }
  onMount(() => {
    mounted = true;
    let unlisten: (() => void) | undefined;
    if (native) void listen("settings-hidden", invalidate).then(stop => {
      if (!mounted) stop(); else { unlisten = stop; lifetimeReady = true; }
    }).catch(() => { if (mounted) { invalidate(); error = "Settings lifetime unavailable. Reopen this section."; } });
    return () => { mounted = false; lifetimeReady = false; invalidate(); unlisten?.(); };
  });
</script>

<!-- Native owner setup extends the approved Owner card's gap-3 and small-button rows (Settings.dc.html565–583). -->
<div class="flex flex-col gap-3 border-t border-white/10 pt-3">
  <div class="flex items-center gap-3"><span class="min-w-0 flex-1 text-[13px] font-medium text-zinc-100">Owner registration on Spark</span><span class={status?.state === "registered" && !busy ? "av-chip bg-av-500/10 text-av-300 ring-av-500/40" : "av-chip text-amber-200 ring-amber-400/30"}>{busy ? "Checking…" : label}</span></div>
  <p class="av-hint">The paired Spark remembers this PC’s local owner. It does not enable voice, grant permissions or start a model. Verify with Windows above before Register or Revoke.</p>
  {#if status?.intent === "unavailable"}<p class="av-hint text-amber-200">The saved registration request is unavailable. You can still refresh Spark’s record and revoke this owner’s registration.</p>{/if}
  {#if status?.state === "revoked"}<p class="av-hint">This pairing's registration is revoked. It cannot be replaced or revived here.</p>{/if}
  <div class="flex flex-wrap items-center gap-2">
    <button class="av-btn av-btn-ghost av-btn-sm" disabled={!enabled || busy || parentBusy} onclick={() => operate("refresh")}>Refresh registration</button>
    {#if status?.state === "unregistered"}<button class="av-btn av-btn-primary av-btn-sm" disabled={!enabled || busy || parentBusy || status.intent === "unavailable"} onclick={() => operate("register")}>Register owner with Spark</button>{/if}
    {#if status?.binding && !status.binding.revoked && status.state !== "different_owner"}<button class="av-btn av-btn-ghost av-btn-sm" disabled={!enabled || busy || parentBusy} onclick={() => operate("revoke")}>Revoke this registration</button>{/if}
  </div>
  {#if error}<p class="av-hint text-amber-200" role="status">{error}</p>{/if}
</div>
