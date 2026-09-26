<script lang="ts">
  import type { Snippet } from "svelte";
  let { children, grow = false }: { children: Snippet; grow?: boolean } = $props();
</script>

<div class="relative min-w-0" class:flex-1={grow}>
  {@render children()}
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="square" class="pointer-events-none absolute top-1/2 right-2.5 -translate-y-1/2 text-zinc-400" aria-hidden="true"><path d="m6 9 6 6 6-6"></path></svg>
</div>
