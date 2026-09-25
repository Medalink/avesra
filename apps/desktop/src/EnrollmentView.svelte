<script lang="ts">
  import { onMount } from "svelte";
  import { command, native, type Runtime } from "./runtime";
  let { runtime }: { runtime: Runtime | null } = $props();
  type Status = { enrollment: string; completed_segments: number; next_segment: string | null; reason: string };
  type Candidate = { id: string; revision: string; model_revision: string | null; segments: number | null; state: string };
  let status = $state<Status | null>(null);
  let candidates = $state<Candidate[]>([]);
  let busy = $state(false);
  let error = $state("");
  let generation = 0;
  let context = "";
  $effect(() => {
    const next = `${runtime?.capture_epoch}:${runtime?.connected}`;
    if (next !== context) { context = next; if (!busy) { generation++; status = null; } }
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
  const prompts = ["Read naturally: Avesra helps me keep track of my work and the things I want to do today.", "Read naturally: I can pause the assistant whenever I need a quiet moment to think.", "Read naturally: The next project will take several careful steps, and I want to review each result.", "Speak naturally for eight seconds about a typical part of your day.", "Held-out phrase: A clear voice carries across the room while the afternoon light changes.", "Held-out phrase: Tomorrow I may choose a different task, but today I will finish this one."];
  async function record() {
    busy = true; error = "";
    const current = ++generation;
    try { const next = await command<Status>("record_enrollment"); if (current === generation) status = next.enrollment === "unavailable" ? null : next; }
    catch (e) { if (current === generation) { error = String(e); status = null; } }
    finally { busy = false; }
  }
  async function save() {
    busy = true; error = "";
    try { await command("finish_enrollment"); status = null; await refresh(); }
    catch(e) { error = String(e); }
    finally { busy = false; }
  }
  async function remove(candidate: Candidate) {
    busy = true; error = "";
    try { await command("delete_speaker_candidate", { id: candidate.id, revision: candidate.revision }); await refresh(); }
    catch (e) { error = String(e); }
    finally { busy = false; }
  }
  async function select(candidate: Candidate | null) {
    busy = true; error = "";
    try { await command(candidate ? "select_speaker_candidate" : "clear_speaker_selection", candidate ? {id: candidate.id, revision: candidate.revision} : undefined); await refresh(); }
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
    <button class="av-btn av-btn-primary self-start" disabled={busy || !runtime?.connected || !runtime.settings.microphone || runtime.settings.explicit_mute || !!status} onclick={prepare}>Prepare owner enrollment</button>
    {#if runtime?.settings.explicit_mute}<p class="av-hint">Deliberate microphone mute is on. Unmute in Audio & Voice before verifying and preparing enrollment.</p>{/if}
  </div>
  {#if status}
    <!-- Inline enrollment card follows Settings.dc.html 605–614. -->
    <div class="av-card flex flex-col gap-2.5 p-3.5">
      <span class="text-[12.5px] font-medium text-zinc-100">Owner enrollment · {status.completed_segments} / 6 segments</span>
      <p class="av-hint">{status.reason}</p>
      <p class="text-[12.5px] text-zinc-100">{prompts[status.completed_segments] ?? "Collection complete. Save this candidate for quality review."}</p>
      <p class="av-hint" role="status">{runtime?.enrollment_capture ? "Microphone recording now · eight seconds" : busy ? "Checking the speaker service or processing the explicit phrase…" : "Microphone off between recordings."}</p>
      <div class="flex items-center justify-end gap-2.5">
        <button class="av-btn av-btn-ghost av-btn-sm" onclick={() => cancel().catch(e => error = String(e))}>Cancel</button>
        {#if status.completed_segments < 6}<button class="av-btn av-btn-primary av-btn-sm" disabled={busy || runtime?.settings.explicit_mute} onclick={record}>Record eight seconds</button>{:else}<button class="av-btn av-btn-primary av-btn-sm" disabled={busy} onclick={save}>Save unqualified candidate</button>{/if}
      </div>
    </div>
  {/if}
  {#each candidates as candidate (candidate.revision)}
    <div class="av-card flex flex-col gap-2.5 p-3.5">
      <span class="text-[12.5px] font-medium">Protected voice candidate · {candidate.state === "unreadable_candidate" ? "unreadable" : candidate.state === "selected_quality_unqualified" ? "selected, unqualified" : candidate.state === "selection_unavailable" ? "selection unavailable" : "quality unqualified"}</span>
      <p class="av-hint">{candidate.segments ?? "Unknown"} segments · model revision {candidate.model_revision?.slice(0, 7) ?? "unavailable"}. This candidate grants no listening or action rights. Unreadable candidates may be removed after verification.</p>
      <button class="av-btn av-btn-ghost av-btn-sm self-start" disabled={busy} onclick={() => remove(candidate)}>Delete candidate · Windows verification required</button>
      <button class="av-btn av-btn-secondary av-btn-sm self-start" disabled={busy || candidate.state !== "candidate_quality_unqualified"} onclick={() => select(candidate)}>Select for validation · Windows verification required</button>
    </div>
  {/each}
  <button class="av-btn av-btn-ghost av-btn-sm self-start" disabled={busy} onclick={() => select(null)}>Clear profile selection · Windows verification required</button>
  <p class="av-hint">Selecting a candidate preserves the chosen revision for validation. It does not enable listening or grant owner rights. Clear selection also recovers an unreadable selection record.</p>
  {#if error}<p class="av-hint text-amber-200" role="status">{error}</p>{/if}
</section>
