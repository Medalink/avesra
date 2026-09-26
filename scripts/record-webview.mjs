// Purposeful manual media capture for the isolated production Settings WebView.
// No input, mocks, assertions, microphone or desktop/audio capture.
import { mkdir, writeFile, access } from 'node:fs/promises';
import { join, isAbsolute } from 'node:path';

const directory = process.argv[2];
const seconds = Number(process.argv[3] ?? 30);
if (!directory || !isAbsolute(directory) || !Number.isFinite(seconds) || seconds < 1 || seconds > 120) {
  throw new Error('Usage: node scripts/record-webview.mjs <absolute-new-directory> [1..120 seconds]');
}
const pages = await (await fetch('http://127.0.0.1:9475/json/list')).json();
const candidates = pages.filter(page => page.url === 'http://tauri.localhost/index.html?window=settings');
if (candidates.length !== 1) throw new Error('Expected one production Settings WebView');
const page = candidates[0];
const url = new URL(page.webSocketDebuggerUrl);
if (url.protocol !== 'ws:' || !['127.0.0.1', 'localhost'].includes(url.hostname) || url.port !== '9475') throw new Error('Unexpected inspection socket');
await mkdir(directory); // Never replace an earlier recording.
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = reject; });
let id = 0;
const pending = new Map();
ws.onmessage = ({ data }) => {
  const response = JSON.parse(data);
  const request = pending.get(response.id);
  if (!request) return;
  pending.delete(response.id);
  clearTimeout(request.timer);
  if (response.error) request.reject(new Error(JSON.stringify(response.error)));
  else request.resolve(response.result);
};
function send(method, params) {
  return new Promise((resolve, reject) => {
    const requestId = ++id;
    const timer = setTimeout(() => { pending.delete(requestId); reject(new Error('Capture timed out')); }, 5000);
    pending.set(requestId, { resolve, reject, timer });
    ws.send(JSON.stringify({ id: requestId, method, params }));
  });
}
const start = performance.now();
const report = { kind: 'sampled_production_settings_webview', url: page.url, start_utc: new Date().toISOString(), target_interval_ms: 200, frames: [], complete: false };
try {
  while (performance.now() - start < seconds * 1000) {
    try { await access(join(directory, 'stop')); break; } catch { /* no stop request */ }
    const began = performance.now();
    const utc = new Date().toISOString();
    const image = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
    const filename = String(report.frames.length).padStart(5, '0') + '.png';
    await writeFile(join(directory, filename), Buffer.from(image.data, 'base64'), { flag: 'wx' });
    report.frames.push({ filename, utc, elapsed_ms: began - start, capture_ms: performance.now() - began });
    await new Promise(resolve => setTimeout(resolve, Math.max(0, 200 - (performance.now() - began))));
  }
  report.complete = true;
} finally {
  report.end_utc = new Date().toISOString();
  await writeFile(join(directory, 'capture.json'), JSON.stringify(report, null, 2), { flag: 'wx' });
  ws.close();
}
console.log(JSON.stringify({ directory, frames: report.frames.length, complete: report.complete }));
