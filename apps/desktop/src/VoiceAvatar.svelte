<script lang="ts">
  import type { AvatarParameters } from "./speaker-profiles";
  let { parameters, phrases = false, name = "You", owner = false }: { parameters: AvatarParameters | null; phrases?: boolean; name?: string; owner?: boolean } = $props();
  const geometry = $derived.by(() => {
    if (!parameters) return null;
    const points = Array.from({ length: 192 }, (_, i) => {
      const index = i / 8, left = Math.floor(index), blend = index - left;
      const value = (parameters.shape[left] * (1 - blend) + parameters.shape[(left + 1) % 24] * blend) / 63;
      const radius = 44 + Math.max(0.04, Math.min(1.05, value)) * 60;
      const angle = i / 192 * Math.PI * 2 - Math.PI / 2;
      return { x: (120 + radius * Math.cos(angle)).toFixed(1), y: (120 + radius * Math.sin(angle)).toFixed(1), angle };
    });
    return { fill: points.map((p, i) => `${i ? "L" : "M"}${p.x},${p.y}`).join(" ") + " Z", spokes: points.filter((_, i) => i % 2 === 0).map(p => `M${(120 + 46 * Math.cos(p.angle)).toFixed(1)},${(120 + 46 * Math.sin(p.angle)).toFixed(1)} L${p.x},${p.y}`).join(" ") };
  });
  const dots = Array.from({ length: 6 }, (_, i) => { const angle = i / 6 * Math.PI * 2 - Math.PI / 2 + .26; return { x: 120 + 112 * Math.cos(angle), y: 120 + 112 * Math.sin(angle) }; });
</script>
<div class="relative grid size-[148px] shrink-0 place-items-center">
  <svg width="148" height="148" viewBox="0 0 240 240" fill="none" role="img" aria-label={geometry ? "Voice avatar: 192 display points interpolated from 24 lossy native shape values; not a live match score" : "Voice avatar unavailable"}>
    <circle cx="120" cy="120" r="44" stroke="rgba(255,255,255,0.08)" />
    <circle cx="120" cy="120" r="108" stroke="rgba(255,255,255,0.06)" stroke-dasharray="2 4" />
    {#if geometry}<path d={geometry.fill} fill="rgba(58,93,216,0.16)" stroke="#3a5dd8" stroke-width="1.4" stroke-linejoin="round" /><path d={geometry.spokes} stroke="#6f88ea" stroke-width="1.2" opacity="0.75" />{/if}
    {#if phrases}{#each dots as dot, i}<circle cx={dot.x} cy={dot.y} r="5" fill={i < 4 ? "#3a5dd8" : "#1f1f23"} stroke="#3a5dd8" stroke-width="2" />{/each}{/if}
  </svg>
  <div class="pointer-events-none absolute flex max-w-[58px] flex-col items-center"><span class="max-w-full truncate text-[12.5px] font-semibold text-zinc-50" title={name}>{name}</span><span class="font-mono text-[10px] text-zinc-400">{owner ? "owner" : "unavailable"}</span></div>
</div>
