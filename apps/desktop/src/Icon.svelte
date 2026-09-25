<script lang="ts">
  let { name, size = 16 }: { name: string; size?: number } = $props();
  const paths: Record<string, string> = {
    mic_off:
      "M15 9V6a3 3 0 0 0-5.7-1.3M9 9v3a3 3 0 0 0 4.8 2.4M19 11a7 7 0 0 1-.9 3.4M16.2 17.4A7 7 0 0 1 5 11M12 18v3M3 3l18 18",
    deafen_off:
      "M3 14v-2a9 9 0 0 1 15.4-6.4M21 12v2M21 14v4a2 2 0 0 1-2 2h-1a1 1 0 0 1-1-1v-3M3 14v4a2 2 0 0 0 2 2h1a1 1 0 0 0 1-1v-4a1 1 0 0 0-1-1H3zM3 3l18 18",
    audio: "M4 10v4M8 7v10M12 4v16M16 8v8M20 11v2",
    models: "m12 3 9 5-9 5-9-5z M3 13l9 5 9-5",
    profiles: "M3 4h18v7H3z M3 13h18v7H3z M7 7.5h.01M7 16.5h.01",
    people:
      "M12.5 8a3.5 3.5 0 1 1-7 0 3.5 3.5 0 0 1 7 0 M2.5 20a6.5 6.5 0 0 1 13 0 M16 4.5a3.5 3.5 0 0 1 0 7 M18 14a6.5 6.5 0 0 1 3.5 6",
    awareness:
      "M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0",
    memory:
      "M4 5c0-4 16-4 16 0s-16 4-16 0v14c0 4 16 4 16 0V5 M4 12c0 4 16 4 16 0",
    close: "m6 6 12 12M18 6 6 18",
    settings:
      "M3 6h10M18 6h3M3 12h4M11 12h10M3 18h12M19 18h2 M13.5 4h4v4h-4z M7 10h4v4H7z M15 16h4v4h-4z",
    mic: "M9 6a3 3 0 0 1 6 0v6a3 3 0 0 1-6 0z M19 11a7 7 0 0 1-14 0M12 18v3",
    deafen:
      "M3 14v-2a9 9 0 0 1 18 0v2 M3 14h4v6H5a2 2 0 0 1-2-2z M21 14h-4v6h2a2 2 0 0 0 2-2z",
    minimize: "M12 4v10m-4.5-4.5L12 14l4.5-4.5 M5 20h14",
    pause: "M7 5v14M17 5v14",
    check: "m5 12 4 4L19 6",
    arrow: "m9 5 7 7-7 7",
    stop: "M5 5h14v14H5z",
  };
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.8"
  stroke-linecap="square"
  stroke-linejoin="miter"
  aria-hidden="true"><path d={paths[name] ?? paths.audio} /></svg
>
