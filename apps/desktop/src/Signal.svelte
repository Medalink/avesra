<script lang="ts">
  import { onMount } from "svelte";
  import { fragmentShader } from "./signal-shader";
  export type SignalFrame = {
    kind: "human" | "thinking" | "speaking";
    source: "owner" | "other" | "background" | "assistant";
    samples: number[];
    sequence: number;
    capturedAt: number;
    captureEpoch?: number;
    playbackEpoch?: number;
    outputId?: string;
    purpose?: "preview" | "reply";
  };
  let {
    frame = null,
    visible = true,
  }: { frame?: SignalFrame | null; visible?: boolean } = $props();
  let canvas: HTMLCanvasElement;
  let failed = $state(false);
  let reduced = $state(false);
  let pageVisible = $state(true);
  let frozenFrame = $state<SignalFrame | null>(null);
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
  let render: ((value: SignalFrame | null) => void) | undefined;
  const color = $derived(
    frame?.kind === "speaking" || frame?.kind === "thinking"
      ? "#e0115f"
      : frame?.source === "owner"
        ? "#3a5dd8"
        : frame?.source === "other"
          ? "#8ede4a"
          : "#71717a",
  );
  const fallbackPath = $derived.by(() => {
    if (!effectiveFrame?.samples.length) return "M0 44H424";
    const values = effectiveFrame.samples;
    return values
      .map(
        (value, i) =>
          `${i ? "L" : "M"}${(i * 424) / Math.max(1, values.length - 1)},${44 - Math.min(1, Math.max(-1, value)) * 35}`,
      )
      .join(" ");
  });
  $effect(() => {
    if (visible && pageVisible) render?.(effectiveFrame);
  });
  onMount(() => {
    const media = matchMedia("(prefers-reduced-motion: reduce)");
    reduced = media.matches;
    const motion = () => {
      reduced = media.matches;
      if (visible && pageVisible) render?.(effectiveFrame);
    };
    const visibility = () => {
      pageVisible = !document.hidden;
      if (pageVisible && visible) render?.(effectiveFrame);
    };
    media.addEventListener("change", motion);
    document.addEventListener("visibilitychange", visibility);
    const gl = canvas.getContext("webgl", {
      alpha: true,
      premultipliedAlpha: true,
      antialias: false,
    });
    if (!gl) {
      failed = true;
      return () => {
        media.removeEventListener("change", motion);
        document.removeEventListener("visibilitychange", visibility);
      };
    }
    const shaders: WebGLShader[] = [];
    const compile = (type: number, source: string) => {
      const shader = gl.createShader(type);
      if (!shader) throw Error("Shader allocation failed");
      shaders.push(shader);
      gl.shaderSource(shader, source);
      gl.compileShader(shader);
      if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS))
        throw Error("Shader compilation failed");
      return shader;
    };
    let program: WebGLProgram | null = null;
    let buffer: WebGLBuffer | null = null;
    let texture: WebGLTexture | null = null;
    const lost = (event: Event) => {
      event.preventDefault();
      failed = true;
      render = undefined;
    };
    canvas.addEventListener("webglcontextlost", lost);
    try {
      program = gl.createProgram();
      if (!program) throw Error("Program unavailable");
      gl.attachShader(
        program,
        compile(
          gl.VERTEX_SHADER,
          "attribute vec2 p;void main(){gl_Position=vec4(p,0.0,1.0);}",
        ),
      );
      gl.attachShader(program, compile(gl.FRAGMENT_SHADER, fragmentShader));
      gl.linkProgram(program);
      if (!gl.getProgramParameter(program, gl.LINK_STATUS))
        throw Error("Program linking failed");
      gl.useProgram(program);
      buffer = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
      gl.bufferData(
        gl.ARRAY_BUFFER,
        new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]),
        gl.STATIC_DRAW,
      );
      const position = gl.getAttribLocation(program, "p");
      gl.enableVertexAttribArray(position);
      gl.vertexAttribPointer(position, 2, gl.FLOAT, false, 0, 0);
      texture = gl.createTexture();
      gl.bindTexture(gl.TEXTURE_2D, texture);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
      const u: Record<string, WebGLUniformLocation | null> = {};
      for (const name of [
        "uBars",
        "uN",
        "uRes",
        "uDpr",
        "uGlow",
        "uMode",
        "uTime",
        "uCore",
        "uLow",
        "uHigh",
      ])
        u[name] = gl.getUniformLocation(program, name);
      gl.uniform1i(u.uBars, 0);

      render = (value) => {
        if (!visible || document.hidden) return;
        const dpr = Math.min(devicePixelRatio, 2);
        canvas.width = Math.round(canvas.clientWidth * dpr);
        canvas.height = Math.round(canvas.clientHeight * dpr);
        gl.viewport(0, 0, canvas.width, canvas.height);
        gl.clearColor(0, 0, 0, 0);
        gl.clear(gl.COLOR_BUFFER_BIT);
        if (
          !value ||
          !value.samples.length ||
          value.samples.length > 512 ||
          value.samples.some((v) => !Number.isFinite(v))
        )
          return;

        const values = value.samples;
        const n =
          value.kind === "thinking"
            ? Math.round(canvas.clientWidth)
            : values.length;
        const bytes = new Uint8Array(n);
        for (let i = 0; i < n; i++) {
          const sample =
            values[
              Math.min(values.length - 1, Math.floor((i * values.length) / n))
            ];
          bytes[i] = Math.round(
            255 *
              (value.kind === "thinking"
                ? (Math.max(-1, Math.min(1, sample)) + 1) / 2
                : Math.max(0, Math.min(1, Math.abs(sample)))),
          );
        }
        gl.texImage2D(
          gl.TEXTURE_2D,
          0,
          gl.LUMINANCE,
          n,
          1,
          0,
          gl.LUMINANCE,
          gl.UNSIGNED_BYTE,
          bytes,
        );
        gl.uniform1f(u.uN, n);
        gl.uniform2f(u.uRes, canvas.width, canvas.height);
        gl.uniform1f(u.uDpr, dpr);
        gl.uniform1f(u.uGlow, 1);
        gl.uniform1f(
          u.uMode,
          value.kind === "thinking" ? 1 : value.kind === "speaking" ? 3 : 2,
        );
        gl.uniform1f(u.uTime, reduced ? 0 : value.capturedAt / 1000);
        const rgb =
          value.kind !== "human"
            ? [0.88, 0.07, 0.37]
            : value.source === "owner"
              ? [0.227, 0.365, 0.847]
              : value.source === "other"
                ? [0.557, 0.871, 0.29]
                : [0.44, 0.44, 0.48];
        gl.uniform3fv(u.uLow, rgb);
        gl.uniform3fv(
          u.uHigh,
          value.kind === "speaking" ? [1, 0.15, 0.28] : rgb,
        );
        gl.uniform3fv(
          u.uCore,
          rgb.map((v) => Math.min(1, v + 0.3)),
        );
        gl.drawArrays(gl.TRIANGLES, 0, 6);
      };
      render(effectiveFrame);
    } catch {
      failed = true;
    }
    return () => {
      render = undefined;
      media.removeEventListener("change", motion);
      document.removeEventListener("visibilitychange", visibility);
      canvas.removeEventListener("webglcontextlost", lost);
      for (const shader of shaders) gl.deleteShader(shader);
      gl.deleteProgram(program);
      gl.deleteBuffer(buffer);
      gl.deleteTexture(texture);
    };
  });
</script>

<canvas
  bind:this={canvas}
  class:hidden={failed}
  class="absolute inset-0 h-full w-full"
  aria-hidden="true"
></canvas>
{#if failed}<svg
    class="absolute inset-0 h-full w-full"
    viewBox="0 0 424 88"
    aria-hidden="true"
    style:color
    ><path
      d={fallbackPath}
      fill="none"
      stroke="currentColor"
      stroke-width="1.5"
    />{#if frame?.kind === "human"}<path
        d={`${fallbackPath} L424 44 L0 44 Z`}
        fill="currentColor"
        opacity="0.25"
        transform="translate(0 88) scale(1 -1)"
      />{:else if frame?.kind === "speaking"}<path
        d={fallbackPath}
        fill="none"
        stroke="currentColor"
        stroke-width="1"
        transform="translate(0 7)"
      /><path
        d={fallbackPath}
        fill="none"
        stroke="currentColor"
        stroke-width="1"
        transform="translate(0 -7)"
      />{:else if frame?.kind === "thinking"}<path
        d={fallbackPath}
        fill="none"
        stroke="currentColor"
        stroke-dasharray="4 3"
        transform="translate(0 3)"
      />{/if}</svg
  >{/if}
