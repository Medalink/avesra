// Voice bars, drawn with WebGL. One renderer for every place Avesra shows voice activity.
// Geometry matches the CSS fallback: 3 px bars on a 6 px pitch, centred, tallest in the middle.
// Colours are fixed: blue you, green other people, grey background, Ruby Avesra speaking,
// light Ruby Avesra thinking. A tone change ripples out from the centre over 420 ms.

export type Tone = "owner" | "other" | "background" | "assistant" | "thinking" | "rest";

export const TONE_RGB: Record<Tone, [number, number, number]> = {
  owner: [0.227, 0.365, 0.847], // #3A5DD8
  other: [0.557, 0.871, 0.29], // #8EDE4A
  background: [0.443, 0.443, 0.478], // zinc-500
  assistant: [0.878, 0.067, 0.373], // Ruby #E0115F
  thinking: [0.953, 0.545, 0.69], // av-300 #F38BB0
  rest: [1, 1, 1],
};

export const BAR_WIDTH = 3;
export const BAR_PITCH = 6;
const RIPPLE_MS = 9;
const BLEND_MS = 420;

export const fragmentShader = `precision mediump float;
uniform sampler2D uBars;
uniform float uN;
uniform vec2 uRes;
uniform float uDpr;
uniform float uMaxH;
uniform float uTime;
const float PITCH = ${BAR_PITCH.toFixed(1)};
const float BARW = ${BAR_WIDTH.toFixed(1)};
void main() {
  vec2 size = uRes / uDpr;
  vec2 p = gl_FragCoord.xy / uDpr;
  float total = uN * PITCH - (PITCH - BARW);
  float x0 = (size.x - total) * 0.5;
  float cy = size.y * 0.5;
  float idx = floor((p.x - x0) / PITCH);
  vec3 col = vec3(0.0);
  float alpha = 0.0;
  for (int k = -3; k <= 3; k++) {
    float i = idx + float(k);
    if (i < 0.0 || i >= uN) continue;
    vec4 look = texture2D(uBars, vec2((i + 0.5) / uN, 0.25));
    vec4 shape = texture2D(uBars, vec2((i + 0.5) / uN, 0.75));
    float halfH = max(1.0, shape.r * uMaxH) * 0.5;
    float cx = x0 + i * PITCH + BARW * 0.5;
    float dx = abs(p.x - cx) - BARW * 0.5;
    float dy = abs(p.y - cy) - halfH;
    float cover = clamp(0.5 - max(dx, dy) * uDpr, 0.0, 1.0) * look.a;
    float v = clamp(abs(p.y - cy) / halfH, 0.0, 1.0);
    // Bright core, dimmer toward the tips, a crisp highlight at each tip.
    vec3 core = look.rgb * (1.2 - 0.5 * v) + vec3(0.22) * smoothstep(0.84, 1.0, v) * shape.b;
    // Slow sheen travelling out from the centre line (speaking only).
    float band = fract(uTime * 0.55 + i * 0.045);
    core += vec3(0.28) * exp(-pow((v - band) * 7.0, 2.0)) * shape.g;
    // Soft halo in the bar's own colour.
    float d = length(max(vec2(dx, dy), 0.0));
    float glow = exp(-d * d / 30.0) * 0.26 * (0.35 + 0.65 * shape.r) * look.a * (1.0 - cover);
    col += min(core, vec3(1.0)) * cover + look.rgb * glow;
    alpha += cover + glow;
  }
  gl_FragColor = vec4(min(col, vec3(1.0)), clamp(alpha, 0.0, 1.0));
}`;

const vertexShader = "attribute vec2 p;void main(){gl_Position=vec4(p,0.0,1.0);}";

type Bar = { height: number; target: number; from: [number, number, number]; fromHeight: number };

export class VoiceBarsRenderer {
  private gl: WebGLRenderingContext;
  private program: WebGLProgram;
  private buffer: WebGLBuffer | null;
  private texture: WebGLTexture | null;
  private u: Record<string, WebGLUniformLocation | null> = {};
  private bars: Bar[];
  private pixels: Uint8Array;
  private tone: Tone = "rest";
  private toneAt = 0;
  private levels: number[] = [];

  constructor(
    private canvas: HTMLCanvasElement,
    private count: number,
    private maxHeight: number,
  ) {
    const gl = canvas.getContext("webgl", { alpha: true, premultipliedAlpha: true, antialias: false });
    if (!gl) throw Error("WebGL unavailable");
    this.gl = gl;
    const compile = (type: number, source: string) => {
      const shader = gl.createShader(type);
      if (!shader) throw Error("Shader allocation failed");
      gl.shaderSource(shader, source);
      gl.compileShader(shader);
      if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) throw Error("Shader compilation failed");
      return shader;
    };
    const program = gl.createProgram();
    if (!program) throw Error("Program unavailable");
    const vs = compile(gl.VERTEX_SHADER, vertexShader);
    const fs = compile(gl.FRAGMENT_SHADER, fragmentShader);
    gl.attachShader(program, vs);
    gl.attachShader(program, fs);
    gl.linkProgram(program);
    gl.deleteShader(vs);
    gl.deleteShader(fs);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) throw Error("Program linking failed");
    this.program = program;
    gl.useProgram(program);
    this.buffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, this.buffer);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]), gl.STATIC_DRAW);
    const position = gl.getAttribLocation(program, "p");
    gl.enableVertexAttribArray(position);
    gl.vertexAttribPointer(position, 2, gl.FLOAT, false, 0, 0);
    this.texture = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    for (const [key, value] of [
      [gl.TEXTURE_MIN_FILTER, gl.NEAREST],
      [gl.TEXTURE_MAG_FILTER, gl.NEAREST],
      [gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE],
      [gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE],
    ])
      gl.texParameteri(gl.TEXTURE_2D, key, value);
    for (const name of ["uBars", "uN", "uRes", "uDpr", "uMaxH", "uTime"]) this.u[name] = gl.getUniformLocation(program, name);
    gl.uniform1i(this.u.uBars, 0);
    this.bars = Array.from({ length: count }, () => ({ height: 0.03, target: 0.03, from: TONE_RGB.rest, fromHeight: 0.03 }));
    this.pixels = new Uint8Array(count * 2 * 4);
  }

  // A new tone starts a centre-out blend from whatever is on screen now.
  setTone(tone: Tone, now: number) {
    if (tone === this.tone) return;
    const t = this.blend(now);
    this.bars.forEach((bar, i) => {
      bar.from = this.mix(bar.from, TONE_RGB[this.tone], t[i]);
      bar.fromHeight = bar.height;
    });
    this.tone = tone;
    this.toneAt = now;
  }

  // Per-bar levels, 0–1, already on their display scale.
  setLevels(levels: number[]) {
    this.levels = levels;
  }

  private blend(now: number) {
    const centre = (this.count - 1) / 2;
    return this.bars.map((_, i) => {
      const x = Math.min(1, Math.max(0, (now - this.toneAt - Math.abs(i - centre) * RIPPLE_MS) / BLEND_MS));
      return x * x * (3 - 2 * x);
    });
  }

  private mix(a: [number, number, number], b: [number, number, number], t: number): [number, number, number] {
    return [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
  }

  draw(now: number, still = false) {
    const { gl, canvas } = this;
    const dpr = Math.min(devicePixelRatio, 2);
    const width = Math.round(canvas.clientWidth * dpr);
    const height = Math.round(canvas.clientHeight * dpr);
    if (canvas.width !== width) canvas.width = width;
    if (canvas.height !== height) canvas.height = height;
    const t = this.blend(now);
    const seconds = still ? 0 : now / 1000;
    const tone = this.tone;
    this.bars.forEach((bar, i) => {
      const envelope = Math.sin((Math.PI * (i + 0.5)) / this.count);
      const level = this.levels[i] ?? 0;
      // Pulse, matching the CSS keyframes: listening 0.3–1, speaking 0.22–1, thinking breathes.
      const phase = seconds * ((2 * Math.PI) / ((900 + ((i * 137) % 700)) / 1000)) + i * 0.83;
      const pulse = still || tone === "rest" || tone === "thinking" ? 1 : tone === "assistant" ? 0.61 + 0.39 * Math.sin(seconds * 4.83 + i * 0.29) : 0.65 + 0.35 * Math.sin(phase);
      const shape =
        tone === "rest" ? 0.03
        : tone === "thinking" ? (4 + envelope * 20 * (0.45 + 0.55 * level)) / this.maxHeight
        : tone === "background" ? (3 + envelope * 14 * (0.3 + 0.7 * level)) / this.maxHeight
        : tone === "other" ? (5 + envelope * (this.maxHeight - 6) * 0.72 * level) / this.maxHeight
        : (6 + envelope * (this.maxHeight - 6) * level) / this.maxHeight;
      const target = Math.min(1, shape * pulse);
      bar.target = bar.fromHeight + (target - bar.fromHeight) * t[i];
      bar.height += (bar.target - bar.height) * (still ? 1 : bar.target > bar.height ? 0.35 : 0.18);
      const rgb = this.mix(bar.from, TONE_RGB[tone], t[i]);
      const breathe = tone === "thinking" && !still ? 0.68 + 0.32 * Math.sin(seconds * 1.96 + i * 0.21) : 1;
      const opacity = (tone === "rest" ? 0.2 : 0.35 + 0.65 * envelope) * breathe;
      const o = i * 4;
      this.pixels[o] = Math.round(rgb[0] * 255);
      this.pixels[o + 1] = Math.round(rgb[1] * 255);
      this.pixels[o + 2] = Math.round(rgb[2] * 255);
      this.pixels[o + 3] = Math.round(opacity * 255);
      const s = (this.count + i) * 4;
      this.pixels[s] = Math.round(Math.min(1, bar.height) * 255);
      this.pixels[s + 1] = tone === "assistant" && !still ? Math.round(255 * t[i]) : 0; // sheen
      this.pixels[s + 2] = tone === "rest" ? 0 : 255; // tip highlight
      this.pixels[s + 3] = 255;
    });
    gl.viewport(0, 0, canvas.width, canvas.height);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.bindTexture(gl.TEXTURE_2D, this.texture);
    gl.pixelStorei(gl.UNPACK_ALIGNMENT, 1);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, this.count, 2, 0, gl.RGBA, gl.UNSIGNED_BYTE, this.pixels);
    gl.uniform1f(this.u.uN, this.count);
    gl.uniform2f(this.u.uRes, canvas.width, canvas.height);
    gl.uniform1f(this.u.uDpr, dpr);
    gl.uniform1f(this.u.uMaxH, this.maxHeight);
    gl.uniform1f(this.u.uTime, seconds);
    gl.drawArrays(gl.TRIANGLES, 0, 6);
  }

  dispose() {
    const { gl } = this;
    gl.deleteProgram(this.program);
    gl.deleteBuffer(this.buffer);
    gl.deleteTexture(this.texture);
    gl.getExtension("WEBGL_lose_context")?.loseContext();
  }
}

// Per-bar display levels from a signal frame's samples.
export function barLevels(samples: number[], count: number, tone: Tone) {
  return Array.from({ length: count }, (_, i) => {
    if (!samples.length) return 0;
    const from = Math.floor((i * samples.length) / count);
    const to = Math.max(from + 1, Math.floor(((i + 1) * samples.length) / count));
    let level = 0;
    for (let k = from; k < to && k < samples.length; k++) level = Math.max(level, Math.abs(samples[k]));
    // Playback keeps its specified display curve (docs/playback-telemetry.md); capture arrives
    // well below full scale, so it is lifted and compressed so quiet talk still reads.
    return tone === "assistant" ? Math.min(0.94, 1 - Math.exp(-4 * level)) : Math.min(1, Math.pow(level * 4, 0.6));
  });
}
