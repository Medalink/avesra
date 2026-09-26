<script lang="ts">
  import VoiceAvatar from "./VoiceAvatar.svelte";
  import type { VoiceAvatar as Avatar, SpeakerCandidate } from "./speaker-profiles";
  let { avatar, candidate, name, owner, listening, paused, loading, blocked, prompts, ontest, onenroll }: { avatar: Avatar; candidate: SpeakerCandidate | undefined; name: string; owner: boolean; listening: boolean; paused: string; loading: boolean; blocked: boolean; prompts: string[]; ontest: () => void; onenroll: () => void } = $props();
  let phrasesOpen = $state(false);
  const ready = $derived((avatar.state === "ready_without_portrait" || avatar.state === "ready_with_portrait") && !!avatar.parameters);
  const personal = $derived(ready && avatar.candidate === null);
</script>
<section class="flex flex-col gap-2.5">
  <span class="av-kicker">Your voice</span>
  <div class="av-card flex flex-col">
    <div class="flex gap-4 p-3.5">
      <VoiceAvatar parameters={avatar.parameters} phrases={!!candidate} {name} {owner} />
      <div class="flex min-w-0 flex-1 flex-col gap-2">
        <div class="flex flex-wrap items-center gap-2"><span class="text-[13px] font-medium text-zinc-50">{name}{owner ? " · owner" : ""}</span><span class="av-chip bg-av-500/10 text-av-300 ring-av-500/40">{loading ? "Checking…" : ready ? "Saved" : "Unavailable"}</span><span class="flex-1"></span><span class="av-chip text-zinc-400 ring-white/10" aria-live="polite"><span class="size-1.5 bg-zinc-500" aria-hidden="true"></span>{listening ? "Listening" : paused}</span></div>
        <span class="av-hint">{candidate ? `6 saved phrases · ECAPA-TDNN${candidate.model_revision ? ` rev ${candidate.model_revision.slice(0, 7)}` : ""} · held-out results not measured here` : personal ? "Built from your saved Personal voice observations." : ready ? "Saved avatar; its matching phrase summary is unavailable." : avatar.reason ?? "Your voice avatar is not available yet. You can keep talking normally."}</span>
        <div class="flex h-[60px] items-center justify-center bg-black/25 ring-1 ring-white/[0.06] ring-inset" aria-label="Live voice signal unavailable"></div>
        <div class="flex items-center gap-3"><span class="av-kicker w-11">Match</span><div class="relative h-1.5 flex-1 bg-white/[0.08]"></div><span class="w-[88px] text-right font-mono text-[11px] text-zinc-500">Not measured</span></div>
      </div>
    </div>
    <div class="flex flex-wrap items-center gap-2 border-t border-white/[0.06] px-3.5 py-2.5">
      <button type="button" class="av-btn av-btn-secondary av-btn-sm" disabled={blocked || !candidate} onclick={ontest}>Test my voice</button>
      <button type="button" class="av-btn av-btn-ghost av-btn-sm" disabled={blocked} onclick={onenroll}>Re-enroll</button>
      <span class="flex-1"></span><button type="button" class="av-btn av-btn-ghost av-btn-sm" aria-expanded={phrasesOpen} onclick={() => phrasesOpen = !phrasesOpen}>{phrasesOpen ? "Hide phrases" : "What it's built from"}</button>
    </div>
    {#if phrasesOpen}
      <div class="av-rise flex flex-col border-t border-white/[0.06]">
        <p class="av-hint m-0 px-3.5 pt-2.5 pb-1">{candidate ? "Four saved phrases form the profile; two are reserved for held-out checks. Dots show saved slots, not passed checks. Raw audio is not retained." : personal ? "This avatar uses saved observations learned from your natural speech. It is not linked to a six-phrase recording." : ready ? "This avatar is linked to a saved candidate, but its exact phrase summary is unavailable." : "No current owner-bound avatar source is available."}</p>
        {#if candidate}{#each prompts as prompt, i}<div class="flex h-9 items-center gap-3 px-3.5"><span class="w-3 font-mono text-[11px] text-zinc-500">{i + 1}</span><span class="min-w-0 flex-1 truncate text-[12px] text-zinc-300" title={prompt}>{prompt.replace(/^(Read naturally: |Held-out phrase: )/, "")}</span><span class={`av-chip ${i < 4 ? "text-[#c3cdff] ring-[#3a5dd8]/50" : "text-zinc-300 ring-white/15"}`}>{i < 4 ? "Profile" : "Held-out"}</span><span class="w-10 text-right font-mono text-[11px] text-zinc-400" title="Per-phrase duration and similarity unavailable">—</span></div>{/each}{/if}
        <p class="av-hint px-3.5 pb-2.5">A voice avatar is a private visual summary, not identity verification or permission.</p>
      </div>
    {/if}
  </div>
</section>
