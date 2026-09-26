<script lang="ts">
  type Row = { key: string; stage: string; source: string; detail: string; p50: number | null; p95: number | null; p99: number | null; max: number | null; errors: string };
  let { rows, empty = "No timed observations yet", label = "Observed stage timings" }: { rows: Row[]; empty?: string; label?: string } = $props();
  const duration = (value: number | null) => value === null ? "—" : value >= 1000 ? `${(value / 1000).toFixed(1)} s` : `${value.toFixed(1)}`;
</script>
<div class="av-card overflow-x-auto" role="region" aria-label={label}>
  <div class="min-w-[560px]" role="table" aria-label={`${label}; values in milliseconds unless marked seconds`}>
    <div class="perf-grid border-b border-white/[0.06] py-2 text-[10.5px] text-zinc-400" role="row">
      <span role="columnheader">Stage</span><span role="columnheader">Model · device</span>
      {#each ["p50", "p95", "p99", "max", "err"] as heading}<span class="text-right" role="columnheader">{heading}</span>{/each}
    </div>
    {#each rows as row (row.key)}
      <div class="perf-grid h-9 items-center text-[12px] hover:bg-white/[0.025]" role="row" title={`${row.stage}: ${row.detail}`}>
        <span class="truncate text-zinc-100" role="cell">{row.stage}</span>
        <span class="truncate text-zinc-400" role="cell" title={row.source}>{row.source}</span>
        {#each [row.p50, row.p95, row.p99, row.max] as value, index}<span class="text-right font-mono {index < 2 ? 'text-zinc-200' : index === 2 ? 'text-zinc-300' : 'text-zinc-400'}" role="cell" aria-label={value === null ? "Unavailable" : `${value.toFixed(3)} milliseconds`}>{duration(value)}</span>{/each}
        <span class="truncate text-right font-mono text-zinc-400" role="cell">{row.errors}</span>
      </div>
    {/each}
  </div>
  {#if rows.length === 0}<p class="px-3.5 py-5 text-[12px] text-zinc-400" role="status">{empty}</p>{/if}
</div>
<style>
  .perf-grid { display: grid; grid-template-columns: minmax(0, 1.2fr) minmax(0, 1.1fr) 52px 52px 52px 52px 40px; gap: 8px; padding-inline: 14px; }
</style>
