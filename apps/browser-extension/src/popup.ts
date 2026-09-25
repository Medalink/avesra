import { object, pairing, scope, type Pairing } from "./protocol.js";
import { view, type View } from "./permissions.js";
let proposal: View | null = null;
let requesting = false;
let proposalDeadline = 0;
let statusDeadline = 0;
function clearProposal() {
  proposal = null; proposalDeadline = 0; statusDeadline = 0;
  for (const id of ["scope-approve", "scope-decline"]) (document.getElementById(id) as HTMLButtonElement).disabled = true;
  document.getElementById("scope-note")!.textContent = "Proposal state unavailable. Refresh or reopen this popup.";
}
let saved: Pairing | null = null;
let publication = 0;
async function send(operation: string) {
  const generation = ++publication;
  try {
    const response: unknown = await chrome.runtime.sendMessage(operation === "forget" ? { operation, pairing: saved } : { operation });
    if (generation !== publication) return;
    if (!object(response, ["state", "displayPhase", "tone", "comparison", "installation", "pairing", "connected", "busy", "proposal", "scope"]) || typeof response.state !== "string" || typeof response.displayPhase !== "string" || !["error", "unavailable", "paired"].includes(String(response.tone)) || typeof response.comparison !== "string" || typeof response.installation !== "string" || typeof response.connected !== "boolean" || typeof response.busy !== "boolean" || (response.proposal !== null && !view(response.proposal)) || (response.scope !== null && !scope(response.scope))) { clearProposal(); return; }
    statusDeadline = performance.now() + 2500;
    document.getElementById("status")!.textContent = response.state;
    document.getElementById("comparison")!.textContent = response.comparison;
    document.getElementById("compare-block")!.hidden = !response.comparison;
    document.getElementById("installation")!.textContent = response.installation;
    document.getElementById("installation-block")!.hidden = !response.installation;
    const nextProposal = view(response.proposal) ? response.proposal : null;
    const same = nextProposal && proposal && nextProposal.reference.id === proposal.reference.id && nextProposal.reference.revision === proposal.reference.revision && nextProposal.session === proposal.session && nextProposal.generation === proposal.generation;
    proposalDeadline = nextProposal ? same ? Math.min(proposalDeadline, performance.now() + nextProposal.remaining_ms) : performance.now() + nextProposal.remaining_ms : 0;
    proposal = nextProposal;
    const outcome = scope(response.scope) ? response.scope : null;
    document.getElementById("scope")!.hidden = !proposal && !outcome;
    document.getElementById("scope-reference")!.textContent = outcome ? `${outcome.reference.id} / ${outcome.reference.revision}` : "";
    document.getElementById("scope-actions")!.hidden = !proposal;
    document.getElementById("scope-outcome")!.textContent = outcome ? ({pending:"Pending browser approval",saving:"Saving native scope metadata",saved:"Native scope metadata saved",declined:"Proposal declined",unavailable:"Save unverified; refresh Windows Settings"}[outcome.state]) : "";
    document.getElementById("scope-origin")!.textContent = proposal?.origin ?? "";
    document.getElementById("scope-operations")!.textContent = proposal ? `Requested operations: ${proposal.operations.join(", ")}` : "";
    document.getElementById("scope-note")!.textContent = !proposal ? "Page operations are unavailable. Chrome permission and saved native metadata remain separate; inspect Settings to recover this exact revision." : proposal.requested ? "Waiting for the browser permission result. This proposal expires; reopening does not retry the request." : "Approve this exact site in Chrome. Saved scope metadata does not enable page actions yet.";
    for (const id of ["scope-approve", "scope-decline"]) (document.getElementById(id) as HTMLButtonElement).disabled = requesting || !proposal || proposal.requested || performance.now() >= proposalDeadline;
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
  } catch { if (generation === publication) { clearProposal(); document.getElementById("status")!.textContent = "Extension state unavailable. Reopen this popup."; } }
}
for (const operation of ["connect", "disconnect", "forget"]) document.getElementById(operation)!.addEventListener("click", () => void send(operation));
void send("status");
const timer = setInterval(() => void send("status"), 1000);
window.addEventListener("pagehide", () => { publication++; clearInterval(timer); });

function decide(allow: boolean) {
  const owned = proposal;
  if (!owned || requesting || owned.requested) return;
  if (performance.now() >= proposalDeadline || performance.now() >= statusDeadline) { clearProposal(); return; }
  requesting = true;
  if (!allow) {
    void chrome.runtime.sendMessage({ operation: "permission_decline", proposal: owned }).finally(() => { requesting = false; void send("status"); }).catch(() => {});
    return;
  }
  // Do not await the background intent: Chrome requires this request directly
  // inside the click gesture. Popup teardown may lose its completion callback.
  void chrome.runtime.sendMessage({ operation: "permission_intent", proposal: owned }).catch(() => {});
  const result = chrome.permissions.request({ origins: [owned.origin + "/*"] });
  void result.then(permitted => chrome.runtime.sendMessage({ operation: "permission_result", proposal: owned, permitted }))
    .catch(() => { /* Unknown completion; background reconciliation owns it. */ })
    .finally(() => { requesting = false; void send("status"); });
}
document.getElementById("scope-approve")!.addEventListener("click", () => decide(true));
document.getElementById("scope-decline")!.addEventListener("click", () => decide(false));
