<script lang="ts">
  import { onMount } from "svelte";
  import { command, native, type Runtime } from "./runtime";
  type Reference = { id: string; revision: string };
  type Scope = { state: string; reference: Reference; origin?: string; operations?: string[]; remaining_ms?: number };
  type Status = { attempt: string | null; state: string; selection: string | null; action_epoch: number; mode_allows_actions: boolean; scope: Scope | null };
  type Grant = Reference & { origin: string; operations: string[]; selection: string; browser_app: string; browser_revision: string };
  let { runtime, status }: { runtime: Runtime | null; status: Status | null } = $props();
  let origin = $state(""), read = $state(true), navigate = $state(false);
  let busy = $state(false), error = $state(""), saved = $state<Grant[] | null>(null);
  let owned = $state<Reference | null>(null);
  let submitted = $state<{ reference: Reference; origin: string; operations: string[] } | null>(null);
  let mounted = false, generation = 0, context = "";
  const enabled = $derived(native && !!runtime?.connected && !runtime.locked);
  const canPropose = $derived(enabled && status?.state === "authenticated_no_scopes" && !!status.selection && status.mode_allows_actions);
  const pending = $derived(status?.scope ?? null);
  const submittedMatches = $derived(!!submitted && !!(pending?.reference ?? owned) && submitted.reference.id === (pending?.reference ?? owned)?.id && submitted.reference.revision === (pending?.reference ?? owned)?.revision);
  const shownOrigin = $derived(pending?.origin ?? (submittedMatches ? submitted!.origin : "Origin not available in this status; refresh saved scopes."));
  const shownOperations = $derived(pending?.operations ?? (submittedMatches ? submitted!.operations : null));
  const pendingLabel = $derived(pending ? ({ pending: "Awaiting extension approval", saving: "Saving scope metadata", saved: "Scope metadata saved", declined: "Declined", unavailable: "Save unverified" }[pending.state] ?? "Unavailable") : "");
  function invalidate() {
    generation++; saved = null; submitted = null;
    const reference = owned; owned = null;
    if (native && reference) void command("cancel_browser_scope", { reference }).catch(() => {});
  }
  $effect(() => {
    const next = `${runtime?.connected}:${runtime?.locked}:${runtime?.capture_epoch}:${runtime?.action_epoch}:${status?.attempt}:${status?.selection}`;
    if (context !== next) { context = next; invalidate(); }
  });
  async function run(work: (current: number) => Promise<void>) {
    if (!enabled || busy) return;
    busy = true; error = ""; const current = ++generation;
    try { await work(current); }
    catch (e) { if (mounted && current === generation) error = String(e); }
    finally { busy = false; }
  }
  function refresh() { return run(async current => {
    saved = null;
    const records = await command<Grant[]>("saved_browser_scopes");
    if (mounted && current === generation) saved = records;
  }); }
  function propose() { return run(async current => {
    const admittedOrigin = origin;
    const operations = [...(read ? ["read"] : []), ...(navigate ? ["navigate"] : [])];
    const reference = await command<Reference>("propose_browser_scope", { origin: admittedOrigin, operations });
    if (!mounted || current !== generation) { void command("cancel_browser_scope", { reference }).catch(() => {}); return; }
    owned = reference; submitted = { reference, origin: admittedOrigin, operations: [...operations] };
  }); }
  function cancel() { return run(async current => {
    const reference = owned ?? pending?.reference;
    if (reference) await command("cancel_browser_scope", { reference });
    if (mounted && current === generation) { owned = null; submitted = null; }
  }); }
  function revoke(record: Grant) { return run(async current => {
    saved = null;
    await command("revoke_browser_scope", { reference: { id: record.id, revision: record.revision } });
    if (!mounted || current !== generation) return;
    const records = await command<Grant[]>("saved_browser_scopes");
    if (mounted && current === generation) saved = records;
  }); }
  onMount(() => { mounted = true; return () => { mounted = false; invalidate(); }; });
</script>

<!-- Exact-scope approval geometry from Settings.dc.html 679–705. -->
<div class="av-card p-3.5">
  <div class="flex items-baseline justify-between gap-3"><span class="av-kicker">Browser site scopes</span><button class="av-btn av-btn-ghost av-btn-sm" disabled={!enabled || busy} onclick={refresh}>Refresh saved scopes</button></div>
  <p class="av-hint mt-2">These records describe setup permission. Page reading and navigation are not available yet. Verify above before proposing or revoking a scope.</p>
  <label class="av-label mt-3 block" for="browser-scope-origin">Exact HTTPS origin</label>
  <input id="browser-scope-origin" class="av-input w-full font-mono" maxlength="512" placeholder="https://example.com" bind:value={origin} disabled={busy || !canPropose || !!owned || !!pending} />
  <div class="av-seg mt-2.5" aria-label="Requested site operations"><button class="av-seg-btn" aria-pressed={read} disabled={busy || !canPropose || !!owned || !!pending} onclick={() => read = !read}>Read</button><button class="av-seg-btn" aria-pressed={navigate} disabled={busy || !canPropose || !!owned || !!pending} onclick={() => navigate = !navigate}>Same-origin navigation</button></div>
  <div class="mt-3 flex justify-end gap-1.5"><button class="av-btn av-btn-primary av-btn-sm" disabled={!canPropose || busy || !!owned || !!pending || !origin || (!read && !navigate)} onclick={propose}>Propose exact scope</button></div>
  {#if pending || owned}
    <div class="mt-3 ring-1 ring-amber-400/25 ring-inset p-3.5">
      <div class="flex items-start gap-3"><span class="flex-1 text-[13px] font-medium text-zinc-50">{pendingLabel || "Proposal admitted"}</span><span class="av-chip text-amber-200 ring-amber-400/25">Setup</span></div>
      <div class="mt-2.5 break-all bg-black/30 px-3 py-2 font-mono text-[11px] leading-4 text-zinc-300">{shownOrigin}<br />{shownOperations?.join(" / ") ?? "Operations unavailable in this status"}<br />{(pending?.reference ?? owned)?.id} / {(pending?.reference ?? owned)?.revision}</div>
      <div class="mt-2.5 grid grid-cols-3 gap-3 text-[11.5px] leading-4">
        <div class="flex flex-col gap-0.5"><span class="text-[10.5px] text-zinc-400">Expected effect</span><span class="text-zinc-200">Save native site scope metadata</span></div>
        <div class="flex flex-col gap-0.5"><span class="text-[10.5px] text-zinc-400">Scope</span><span class="text-zinc-200">{shownOperations?.join(" / ") ?? "Refresh exact saved revision"}</span></div>
        <div class="flex flex-col gap-0.5"><span class="text-[10.5px] text-zinc-400">Recovery</span><span class="text-zinc-200">Revoke this exact revision; Chrome permission is separate</span></div>
      </div>
      <p class="av-hint mt-2.5">{pending?.state === "saved" ? "Refresh saved scopes to inspect the durable result. This is not a document or account verification." : pending?.state === "unavailable" ? "Refresh to check whether the change was saved." : "Open the selected extension popup and approve this proposal there. Closing setup or letting it expire withdraws pending authority."}</p>
      <div class="mt-3 flex justify-end"><button class="av-btn av-btn-ghost av-btn-sm" disabled={!enabled || busy} onclick={cancel}>{pending?.state === "pending" || pending?.state === "saving" ? "Cancel proposal" : "Dismiss"}</button></div>
    </div>
  {/if}
  {#if error}<p class="av-hint mt-2 text-amber-200" role="alert">{error}</p>{/if}
</div>
{#if saved !== null}
  <div class="av-card divide-y divide-white/[0.06]">
    {#each saved as record (`${record.id}:${record.revision}`)}
      <div class="flex items-start gap-3 px-3.5 py-2.5"><div class="min-w-0 flex-1"><span class="text-[12.5px] text-zinc-200">{record.origin}</span><p class="av-hint">Saved metadata · {record.operations.join(" · ")} · Chrome permission not inspected</p><p class="break-all font-mono text-[10.5px] leading-4 text-zinc-400">{record.id} / {record.revision}<br />Selection: {record.selection}<br />Application: {record.browser_app} / {record.browser_revision}</p></div><button class="av-btn av-btn-ghost av-btn-sm" disabled={!enabled || busy} onclick={() => revoke(record)}>Revoke</button></div>
    {:else}<p class="av-hint px-3.5 py-2.5">No saved native site scopes.</p>{/each}
  </div>
  <p class="av-hint">Revoking a native scope closes its browser connection. It does not remove an independently granted Chrome site permission; manage that in the browser's extension settings.</p>
{/if}
