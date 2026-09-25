<script lang="ts">
  import { onMount } from "svelte";
  import { command, native, type Runtime } from "./runtime";
  let { runtime }: { runtime: Runtime | null } = $props();
  type Status = { enrollment: string; completed_segments: number; next_segment: string | null; reason: string };
  type Candidate = { id: string; revision: string; model_revision: string; segments: number; state: string };
  let status = $state<Status | null>(null);
  let candidates = $state<Candidate[]>([]);
  let busy = $state(false);
  let error = $state("");
  let generation = 0;
  let context = "";
  $effect(() => {
    const next = `${runtime?.capture_epoch}:${runtime?.connected}`;
    if (next !== context) { context = next; generation++; status = null; }
  });
  async function refresh() {
    const current = generation;
    const next = await command<Candidate[]>("speaker_candidates");
    if (current === generation) candidates = next;
  }
  async function prepare() {
    busy = true; error = "";
    const current = ++generation;
    try { const next = await command<Status>("begin_enrollment"); if (current === generation) status = next; }
    catch (e) { if (current === generation) error = String(e); }
    finally { busy = false; }
  }
  async function cancel() { generation++; status = null; await command("cancel_setup"); }
  async function remove(candidate: Candidate) {
    busy = true; error = "";
    try { await command("delete_speaker_candidate", { id: candidate.id, revision: candidate.revision }); await refresh(); }
    catch (e) { error = String(e); }
    finally { busy = false; }
  }
  onMount(() => { if (native) void refresh().catch(e => error = String(e)); return () => { generation++; }; });
</script>

<section class="section">
  <span class="av-kicker">Owner</span>
  <div class="av-card flex flex-col gap-3 p-3.5">
    <div class="row"><h2>No owner enrolled</h2><span class="av-chip text-amber-200 ring-amber-400/30">Setup required</span></div>
    <p class="av-hint">Enrollment collects prompted phrases, natural speech and separate held-out phrases. Windows verification is required to prepare it.</p>
    <button class="av-btn av-btn-primary self-start" disabled={busy || !runtime?.connected || !runtime.settings.microphone || !!status} onclick={prepare}>{busy ? "Preparing…" : "Prepare owner enrollment"}</button>
  </div>
  {#if status}
    <!-- Inline enrollment card follows Settings.dc.html 605–614. -->
    <div class="av-card flex flex-col gap-2.5 p-3.5">
      <span class="text-[12.5px] font-medium text-zinc-100">Owner enrollment · {status.completed_segments} / 6 segments</span>
      <p class="av-hint">{status.reason} The microphone remains off.</p>
      <div class="flex items-center justify-end gap-2.5">
        <button class="av-btn av-btn-ghost av-btn-sm" onclick={() => cancel().catch(e => error = String(e))}>Cancel</button>
        <button class="av-btn av-btn-primary av-btn-sm" disabled>Record prompted phrase</button>
      </div>
    </div>
  {/if}
  {#each candidates as candidate (candidate.revision)}
    <div class="av-card flex flex-col gap-2.5 p-3.5">
      <span class="text-[12.5px] font-medium">Protected voice candidate · quality unqualified</span>
      <p class="av-hint">{candidate.segments} segments · model revision {candidate.model_revision.slice(0, 7)}. This candidate grants no listening or action rights.</p>
      <button class="av-btn av-btn-ghost av-btn-sm self-start" disabled={busy} onclick={() => remove(candidate)}>Delete candidate · Windows verification required</button>
    </div>
  {/each}
  {#if error}<p class="av-hint text-amber-200" role="status">{error}</p>{/if}
</section>
