import { object, pairing, type Pairing } from "./protocol.js";
let saved: Pairing | null = null;
let publication = 0;
async function send(operation: string) {
  const generation = ++publication;
  try {
    const response: unknown = await chrome.runtime.sendMessage(operation === "forget" ? { operation, pairing: saved } : { operation });
    if (generation !== publication || !object(response, ["state", "comparison", "installation", "pairing", "connected", "busy"]) || typeof response.state !== "string" || typeof response.comparison !== "string" || typeof response.installation !== "string" || typeof response.connected !== "boolean" || typeof response.busy !== "boolean") return;
    document.getElementById("status")!.textContent = response.state;
    document.getElementById("comparison")!.textContent = response.comparison;
    document.getElementById("compare-block")!.hidden = !response.comparison;
    document.getElementById("installation")!.textContent = response.installation;
    document.getElementById("installation-block")!.hidden = !response.installation;
    saved = pairing(response.pairing) ? response.pairing : null;
    document.getElementById("recovery")!.hidden = !saved;
    document.getElementById("revision")!.textContent = saved ? `${saved.id} / ${saved.revision}` : "";
    const chip = document.getElementById("chip")!;
    chip.textContent = response.connected ? "Paired · no scopes" : "Disconnected";
    chip.classList.toggle("connected", response.connected);
    (document.getElementById("connect") as HTMLButtonElement).disabled = response.busy || response.connected;
    (document.getElementById("forget") as HTMLButtonElement).disabled = response.busy || !saved;
  } catch { if (generation === publication) document.getElementById("status")!.textContent = "Extension state unavailable. Reopen this popup."; }
}
for (const operation of ["connect", "disconnect", "forget"]) document.getElementById(operation)!.addEventListener("click", () => void send(operation));
void send("status");
const timer = setInterval(() => void send("status"), 1000);
window.addEventListener("pagehide", () => { publication++; clearInterval(timer); });
