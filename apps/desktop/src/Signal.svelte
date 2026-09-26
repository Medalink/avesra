<script lang="ts">
  import { onMount } from "svelte";
  import { VoiceBarsRenderer, barLevels, type Tone } from "./voice-bars";
  export type SignalFrame = {
    kind: "human" | "thinking" | "speaking";
    source: "owner" | "other" | "background" | "assistant";
    samples: number[];
    rms?: number;
    peak?: number;
    sequence: number;
    capturedAt: number;
    displayExpiresAt?: number;
    captureEpoch?: number;
    playbackEpoch?: number;
    outputId?: string;
    purpose?: "preview" | "greeting" | "reply";
  };
  let {
    frame = null,
    visible = true,
    count = 44,
    height = 76,
    tone: forcedTone = undefined,
  }: { frame?: SignalFrame | null; visible?: boolean; count?: number; height?: number; tone?: Tone } = $props();

  let canvas: HTMLCanvasElement;
  let renderer = $state<VoiceBarsRenderer | null>(null);
  let failed = $state(false);
  let reduced = $state(false);
  let pageVisible = $state(true);
  let expired = $state(false);
  let frozenFrame = $state<SignalFrame | null>(null);
  // Reduced motion holds one frame per source and output instead of following every update.
  $effect(() => {
    if (!reduced || !frame) {
      frozenFrame = null;
    } else if (
      frozenFrame?.kind !== frame.kind ||
      frozenFrame?.source !== frame.source ||
      frozenFrame?.captureEpoch !== frame.captureEpoch ||
      frozenFrame?.playbackEpoch !== frame.playbackEpoch ||
      frozenFrame?.outputId !== frame.outputId
    ) {
      frozenFrame = frame;
    }
  });
  const effectiveFrame = $derived(reduced ? (frozenFrame ?? frame) : frame);
  // Each frame keeps its calibrated display expiry; an expired frame falls back to the resting line.
  $effect(() => {
    const expires = effectiveFrame?.displayExpiresAt;
    expired = false;
    if (expires === undefined) return;
    const wait = expires - performance.now();
    if (wait <= 0) {
      expired = true;
      return;
    }
    const timer = setTimeout(() => (expired = true), wait);
    return () => clearTimeout(timer);
  });
  const shown = $derived.by(() => {
    const value = visible && !expired ? effectiveFrame : null;
    if (!value || !value.samples.length || value.samples.length > 512 || value.samples.some((v) => !Number.isFinite(v))) return null;
    return value;
  });
  const tone = $derived<Tone>(
    !shown
      ? "rest"
      : (forcedTone ??
        (shown.kind === "speaking" || shown.source === "assistant" ? "assistant" : shown.kind === "thinking" ? "thinking" : shown.source)),
  );
  const levels = $derived(barLevels(shown?.samples ?? [], count, tone));

  onMount(() => {
    const media = matchMedia("(prefers-reduced-motion: reduce)");
    reduced = media.matches;
    const motion = () => (reduced = media.matches);
    const visibility = () => (pageVisible = !document.hidden);
    media.addEventListener("change", motion);
    document.addEventListener("visibilitychange", visibility);
    const lost = (event: Event) => {
      event.preventDefault();
      failed = true;
      renderer?.dispose();
      renderer = null;
    };
    canvas.addEventListener("webglcontextlost", lost);
    try {
      renderer = new VoiceBarsRenderer(canvas, count, height);
    } catch {
      failed = true;
    }
    return () => {
      media.removeEventListener("change", motion);
      document.removeEventListener("visibilitychange", visibility);
      canvas.removeEventListener("webglcontextlost", lost);
      renderer?.dispose();
      renderer = null;
    };
  });

  // Draw every animation frame while visible; reduced motion draws one still per change.
  $effect(() => {
    const draw = renderer;
    if (!draw || !visible || !pageVisible) return;
    draw.setTone(tone, performance.now());
    draw.setLevels(levels);
    if (reduced) {
      draw.draw(performance.now(), true);
      return;
    }
    let animation = requestAnimationFrame(function step(now) {
      draw.draw(now);
      animation = requestAnimationFrame(step);
    });
    return () => cancelAnimationFrame(animation);
  });

  // CSS fallback: the same bars without WebGL.
  const toneClass: Record<Tone, string> = {
    owner: "av-listen bg-[#3a5dd8]",
    other: "av-listen bg-[#8ede4a]",
    background: "av-listen bg-zinc-500",
    assistant: "av-speak bg-av-500",
    thinking: "av-breathe bg-av-300",
    rest: "bg-white/20",
  };
  const fallbackBars = $derived(
    levels.map((level, i) => {
      const envelope = Math.sin((Math.PI * (i + 0.5)) / count);
      const span = height - 6;
      const size =
        tone === "rest" ? 2
        : tone === "thinking" ? 4 + envelope * 20 * (0.45 + 0.55 * level)
        : tone === "background" ? 3 + envelope * 14 * (0.3 + 0.7 * level)
        : tone === "other" ? 5 + envelope * span * 0.72 * level
        : 6 + envelope * span * level;
      return {
        height: Math.round(size * 10) / 10,
        opacity: Math.round((0.35 + 0.65 * envelope) * 100) / 100,
        duration: tone === "assistant" ? 1300 : 900 + ((i * 137) % 700),
        delay: -((i * 97) % 1100),
        ripple: Math.round(Math.abs(i - (count - 1) / 2) * 9),
      };
    }),
  );
</script>

<canvas bind:this={canvas} class:hidden={failed} class="pointer-events-none absolute inset-0 h-full w-full" aria-hidden="true"></canvas>
{#if failed}<div class="pointer-events-none absolute inset-0 flex items-center justify-center gap-[3px]" aria-hidden="true">
    {#each fallbackBars as bar, i (i)}<span
        class="block w-[3px] shrink-0 transition-[height,background-color,opacity] duration-[420ms] ease-out {toneClass[tone]}"
        style:height="{bar.height}px"
        style:opacity={bar.opacity}
        style:animation-duration="{bar.duration}ms"
        style:animation-delay="{bar.delay}ms"
        style:transition-delay="{bar.ripple}ms"
      ></span>{/each}
  </div>{/if}
