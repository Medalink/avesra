// Manual development inspection only. Does not synthesize UI state or run assertions.
// Use only with the explicit isolated process started by inspect-background.ps1.
import { writeFile } from 'node:fs/promises';
import { isAbsolute } from 'node:path';
const window = process.argv[4] ?? 'settings';
const urls = { settings: 'http://tauri.localhost/index.html?window=settings', overlay: 'http://tauri.localhost/' };
if (!process.argv[2] || (process.argv[3] && !isAbsolute(process.argv[3])) || !Object.hasOwn(urls, window) || process.argv.length > 5) {
  throw new Error('Usage: node scripts/inspect-webview.mjs <observed-DOM-expression> [absolute-new-screenshot.png] [settings|overlay]');
}
const pages = await (await fetch('http://127.0.0.1:9475/json/list')).json();
const candidates = pages.filter(p => p.type === 'page' && p.url === urls[window]);
if (candidates.length !== 1) throw new Error(`Expected exactly one production Avesra ${window} WebView`);
const page = candidates[0];
const socketUrl = new URL(page.webSocketDebuggerUrl);
if (socketUrl.protocol !== 'ws:' || !['127.0.0.1', 'localhost'].includes(socketUrl.hostname) || socketUrl.port !== '9475') throw new Error('Inspection socket is not the expected local endpoint');
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = reject; });
let id = 0;
const requests = new Map();
ws.onmessage = ({ data }) => {
  const message = JSON.parse(data);
  const pending = requests.get(message.id);
  if (!pending) return;
  requests.delete(message.id);
  clearTimeout(pending.timer);
  if (message.error) pending.reject(new Error(JSON.stringify(message.error)));
  else pending.resolve(message.result);
};
function send(method, params = {}) {
  return new Promise((resolve, reject) => {
    const requestId = ++id;
    const timer = setTimeout(() => { requests.delete(requestId); reject(new Error(`${method} timed out`)); }, 15000);
    requests.set(requestId, { resolve, reject, timer });
    ws.send(JSON.stringify({ id: requestId, method, params }));
  });
}
try {
  const result = await send('Runtime.evaluate', { expression: process.argv[2], awaitPromise: true, returnByValue: true });
  if (result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails));
  console.log(JSON.stringify(result.result.value, null, 2));
  if (process.argv[3]) {
    const shot = await send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false });
    await writeFile(process.argv[3], Buffer.from(shot.data, 'base64'), { flag: 'wx' });
    console.log(`Screenshot: ${process.argv[3]}`);
  }
} finally { ws.close(); }
