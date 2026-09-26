<script lang="ts">
  import type { PreferenceDraft, PreferenceField } from "./preference-draft.svelte";
  import type { AudioDevice } from "./runtime";
  let { draft, devices }: { draft: PreferenceDraft; devices: AudioDevice[] } = $props();
  const names: Record<PreferenceField, string> = { microphone: "Microphone", speaker: "Speakers", learning_chime: "Learning chime", action_chime: "Action chime", learning_chime_volume: "Learning volume", action_chime_volume: "Action volume", profile: "Profile", interface_scale: "Interface size", always_on_top: "Always on top" };
  function value(field: PreferenceField, input: unknown) {
    if (input === null) return "Default / none";
    if (field === "microphone" || field === "speaker") return devices.find(device => device.id === input)?.name ?? "Unavailable device";
    return typeof input === "boolean" ? input ? "On" : "Off" : String(input);
  }
</script>

{#if draft.review}
  <div class="shrink-0 border-t border-white/[0.06] bg-black/20 px-4 py-3">
    <p class="av-label">Review current preferences</p>
    <div class="av-scroll my-2 max-h-32 overflow-y-auto">
      {#each draft.rows as row}<p class="av-hint">{names[row.field]}: current {value(row.field, draft.review[row.field])}; your draft {value(row.field, row.value)}.</p>{/each}
    </div>
    <button class="av-btn av-btn-secondary av-btn-sm" disabled={draft.busy || draft.closing || !!draft.closeRequest} onclick={() => draft.keepDraft()}>Keep my draft for these values</button>
  </div>
{/if}
<footer class="flex h-[52px] shrink-0 items-center gap-2 border-t border-white/[0.06] bg-black/20 px-4">
  {#if draft.closeRequest}
    <span class="flex min-w-0 flex-1 items-center gap-2 text-[12.5px] text-amber-100" role="status"><span class="size-1.5 shrink-0 bg-amber-400" aria-hidden="true"></span><span class="truncate" title={draft.message}>{draft.busy ? "Wait for the current operation to finish." : draft.message || (draft.concern ? "Some changes may already be live. Closing will not undo them." : draft.dirty ? `Close ${draft.closeRequest.intent === "quit" ? "Avesra" : "Settings"} without saving? Your draft will be lost.` : "No unsaved changes. Close now?")}</span></span>
    {#if draft.message}<button class="av-btn av-btn-secondary av-btn-sm" title={draft.message} disabled={draft.busy || draft.closing} onclick={() => draft.refreshClose()}>Refresh close decision</button>{/if}
    <button class="av-btn av-btn-ghost" disabled={draft.busy || draft.closing} onclick={() => draft.answer(false)}>Keep editing</button>
    <button class="av-btn av-btn-danger-soft" disabled={draft.busy || draft.closing} onclick={() => draft.answer(true)}>{draft.concern ? "Close without retrying" : draft.dirty ? "Discard and close" : "Close"}</button>
  {:else}
    <span class="flex min-w-0 flex-1 items-center gap-2 text-[12.5px]" role="status">
      {#if draft.busy}<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6" class="av-spin shrink-0 text-av-400" aria-hidden="true"><path d="M12 3a9 9 0 1 1-9 9"></path></svg>{:else if draft.dirty}<span class="size-1.5 shrink-0 bg-amber-400" aria-hidden="true"></span>{/if}
      <span class="truncate {draft.concern || draft.conflicts.length ? 'text-amber-200' : draft.dirty || draft.busy ? 'text-zinc-300' : 'text-zinc-400'}" title={draft.message || (draft.conflicts.length ? "Preferences changed elsewhere. Review before saving." : "")}>{draft.busy ? "Saving / reading preferences…" : draft.message || (draft.conflicts.length ? "Preferences changed elsewhere. Review before saving." : draft.dirty ? "Unsaved changes" : "No unsaved changes")}</span>
    </span>
    {#if !draft.editor}<button class="av-btn av-btn-secondary" disabled={draft.busy || draft.closing} onclick={() => draft.retryEditor()}>Open editor</button>{:else}
      {#if draft.conflicts.length || draft.concern}<button class="av-btn av-btn-secondary" disabled={draft.busy || draft.closing} onclick={() => draft.inspect()}>Review</button>{/if}
      <button class="av-btn av-btn-ghost" disabled={draft.busy || draft.closing || !draft.dirty || draft.concern} onclick={() => draft.discard()}>Discard</button>
      <button class="av-btn av-btn-primary" disabled={draft.busy || draft.closing || !draft.dirty || !!draft.conflicts.length || (draft.concern && !draft.reviewed)} onclick={() => draft.save()}>{draft.concern ? "Retry save" : "Save changes"}</button>
    {/if}
  {/if}
</footer>
