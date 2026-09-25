import { object, pairing, type Pairing } from "./protocol.js";
let saved: Pairing | null = null;
let publication = 0;
async function send(operation: string) {
  const generation = ++publication;
  try {
    const response: unknown = await chrome.runtime.sendMessage(operation === "forget" ? { operation, pairing: saved } : { operation });
    if (generation !== publication || !object(response, ["state", "displayPhase", "tone", "comparison", "installation", "pairing", "connected", "busy"]) || typeof response.state !== "string" || typeof response.displayPhase !== "string" || !["error", "unavailable", "paired"].includes(String(response.tone)) || typeof response.comparison !== "string" || typeof response.installation !== "string" || typeof response.connected !== "boolean" || typeof response.busy !== "boolean") return;
    document.getElementById("status")!.textContent = response.state;
    document.getElementById("comparison")!.textContent = response.comparison;
    document.getElementById("compare-block")!.hidden = !response.comparison;
    document.getElementById("installation")!.textContent = response.installation;
    document.getElementById("installation-block")!.hidden = !response.installation;
    saved = pairing(response.pairing) ? response.pairing : null;
    document.getElementById("recovery")!.hidden = !saved;
    document.getElementById("revision")!.textContent = saved ? `${saved.id} / ${saved.revision}` : "";
    const chip = document.getElementById("chip")!;
    chip.textContent = response.displayPhase;
    chip.classList.toggle("connected", response.tone === "paired");
    chip.classList.toggle("unavailable", response.tone === "unavailable");
    chip.classList.toggle("error", response.tone === "error");
    (document.getElementById("connect") as HTMLButtonElement).disabled = response.busy || response.connected;
    (document.getElementById("forget") as HTMLButtonElement).disabled = response.busy || !saved;
  } catch { if (generation === publication) document.getElementById("status")!.textContent = "Extension state unavailable. Reopen this popup."; }
}
for (const operation of ["connect", "disconnect", "forget"]) document.getElementById(operation)!.addEventListener("click", () => void send(operation));
void send("status");
const timer = setInterval(() => void send("status"), 1000);
window.addEventListener("pagehide", () => { publication++; clearInterval(timer); });
