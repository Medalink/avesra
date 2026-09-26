import * as p from "./protocol.js";
import { ObservationAuthority } from "./authority.js";
import { Documents } from "./documents.js";
import { ReadJob, acknowledgeRead, pendingSettlement } from "./read-job.js";
import { PermissionProposal, view } from "./permissions.js";
let port: chrome.runtime.Port | null = null;
let generation = 0;
let state = "Not connected";
let displayPhase = "Disconnected";
let tone: "error" | "unavailable" | "paired" = "error";
let comparison = "";
let installation = "";
let savedPairing: p.Pairing | null = null;
let connected = false;
let busy = false;
let poll: ReturnType<typeof setTimeout> | undefined;
let observation: ObservationAuthority | null = null;
let permissions: PermissionProposal | null = null;
let documents: Documents | null = null;
let reading: ReadJob | null = null;
let scopeOutcome: p.Scope | null = null;
const reconnectAlarm="avesra-saved-pair-reconnect";
let retry:ReturnType<typeof setTimeout>|undefined;
let retrySeconds=1, retryBlocked=false, preferenceGeneration=0;
let activating=false, preferenceBusy=false;
function stopRetry(){clearTimeout(retry);retry=undefined;}
function scheduleRetry(){
  if(retryBlocked || retry!==undefined || port)return;
  // Jitter avoids phase-locking repeated host attempts into the native
  // listener's bounded accept/retry gaps. It is timing only, never authority.
  const random=crypto.getRandomValues(new Uint32Array(1))[0]/0x1_0000_0000;
  const delay=1+Math.max(0,retrySeconds-1)*(0.8+0.2*random);retrySeconds=Math.min(60,retrySeconds*2);
  retry=setTimeout(()=>{retry=undefined;void automaticConnect();},delay*1000);
}
async function automaticConnect(){
  if(activating||retryBlocked||port||busy)return;
  activating=true;const token=preferenceGeneration;
  try {
    if(await p.automatic() && token===preferenceGeneration && !retryBlocked)await connect(false);
  } catch {if(token===preferenceGeneration){retryBlocked=true;state="Saved pairing or reconnect preference unavailable; explicit recovery required.";}}
  finally{activating=false;}
}
async function userPreference(enabled:boolean){
  if(preferenceBusy)throw new Error("Reconnect preference is busy");
  preferenceBusy=true;
  try {
  const token=++preferenceGeneration;retryBlocked=true;stopRetry();
  if(!enabled)close("Disconnected; automatic reconnect disabled");
  await p.automaticPreference(enabled);
  if(token!==preferenceGeneration)return;
  retryBlocked=!enabled;retrySeconds=1;
  if(enabled)await connect(true);
  } finally {preferenceBusy=false;}
}
// Timers disappear when a service worker terminates. One named alarm provides
// a bounded wakeup; every wake rereads the saved preference and credential shape.
chrome.alarms.onAlarm.addListener(alarm=>{if(alarm.name===reconnectAlarm)void automaticConnect();});
async function activate(){
  try {await chrome.alarms.create(reconnectAlarm,{periodInMinutes:1});await automaticConnect();}
  catch {state="Automatic browser connection unavailable";}
}
chrome.runtime.onStartup.addListener(()=>{void activate();});
chrome.runtime.onInstalled.addListener(()=>{void activate();});
function current(owned: chrome.runtime.Port, epoch: number) { return owned === port && generation === epoch; }
function close(reason: string, owned = port, epoch = generation, unavailable = false) {
  if (owned !== port || epoch !== generation) return;
  reading?.dispose(); reading = null;
  documents?.dispose(); documents = null;
  permissions?.dispose(); permissions = null; scopeOutcome = null;
  observation?.dispose(); observation = null;
  port = null; generation++; state = reason; connected = false; comparison = "";
  displayPhase = unavailable ? "Unavailable" : "Disconnected"; tone = unavailable ? "unavailable" : "error";
  clearTimeout(poll); poll = undefined; owned?.disconnect();
  scheduleRetry();
}
function snapshot() { return { state, displayPhase, tone, comparison, installation, pairing: savedPairing, connected, busy: busy || preferenceBusy || p.isWriting(), proposal: permissions?.snapshot() ?? null, scope: scopeOutcome }; }
async function connect(explicit=false) {
  if (port || busy || p.isWriting()) return;
  busy = true; state = "Reading this installation"; displayPhase = "Preparing"; tone = "unavailable";
  const epoch = ++generation;
  try {
    const loaded = explicit ? await p.load() : await p.loadSaved();
    if(!loaded)return;
    if (generation !== epoch) return;
    installation = loaded.installation; savedPairing = loaded.record?.pairing ?? null;
    let record = loaded.record;
    const connection = crypto.randomUUID(), nonce = p.nonce();
    const owned = chrome.runtime.connectNative("com.avesra.companion");
    port = owned; state = record ? "Connecting saved browser pairing" : "Connecting to the Settings pairing window"; displayPhase = "Pairing";
    const authority = new ObservationAuthority(() => close("Native status expired; browser observations withdrawn.", owned, epoch, true));
    observation = authority;
    const scopes = new PermissionProposal(authority, () => current(owned, epoch));
    permissions = scopes;
    const metadata = new Documents(authority, () => current(owned, epoch), () => close("Document observation revision exhausted; reconnect explicitly.", owned, epoch, true));
    documents = metadata;
    const reads = new ReadJob(authority, () => current(owned, epoch), () => metadata.observationRevision(),
      () => close("Browser read cleanup unavailable; resource ownership retained.", owned, epoch, true));
    reading = reads;
    let challenge: p.Challenge | null = null, handling = false, waiting = true, sequence = 0;
    let nativeGeneration: number | null = null;
    let phase: "challenge" | "pending" | "proof" | "authenticated" = "challenge";
    function send(value: unknown) {
      if (!current(owned, epoch)) throw new Error("Connection changed");
      waiting = true; owned.postMessage(value);
    }
    function schedule() {
      if (!current(owned, epoch) || !challenge) return;
      poll = setTimeout(() => {
        if (!current(owned, epoch) || !challenge || waiting || handling) return;
        if (!p.counter(sequence + 1)) { close("Browser sequence exhausted", owned, epoch, true); return; }
        if (phase === "authenticated" && record) {
          const chunk=reads.takeMailboxChunk();
          if(chunk){send({type:"mailbox_chunk",body:{session:challenge.session,sequence:++sequence,observation_revision:metadata.observationRevision(),chunk}});return;}
          const reply = reads.takeReply();
          if (reply) { send({type:"read_result",body:{session:challenge.session,sequence:++sequence,observation_revision:metadata.observationRevision(),reply}}); return; }
          const settlement = pendingSettlement(record.pairing);
          if (settlement) { send({type:"read_settlement",body:{session:challenge.session,sequence:++sequence,observation_revision:metadata.observationRevision(),settlement}}); return; }
        }
        const result = metadata.takeReply();
        if (result) { send({type:"document_result",body:{session:challenge.session,sequence:++sequence,reply:result}}); return; }
        const decision = scopes.takeDecision();
        send(decision ? { type: "scope_result", body: { session: challenge.session, sequence: ++sequence, observation_revision:metadata.observationRevision(), ...decision } } : { type: "poll", body: { session: challenge.session, sequence: ++sequence, observation_revision:metadata.observationRevision() } });
      }, reads.mailboxWaiting()?20:1000);
    }
    async function authenticate() {
      if (!challenge || !record) throw new Error("Pairing unavailable");
      const proof = await p.proof(challenge, record, nonce);
      if (!current(owned, epoch)) return;
      phase = "proof"; state = "Verifying saved pairing";
      send({ type: "authenticate", body: { version: 10, session: challenge.session, challenge: challenge.challenge, pairing: record.pairing, proof } });
    }
    async function receive(value: unknown) {
      if (!current(owned, epoch)) return;
      if (handling || !waiting) throw new Error("Unexpected native response");
      handling = true; waiting = false;
      try {
        if (new TextEncoder().encode(JSON.stringify(value)).length > 65536) throw new Error("Native response exceeds limit");
        if (p.object(value, ["type", "body"]) && value.type === "challenge" && p.challenge(value.body)) {
          if (phase !== "challenge" || value.body.installation !== installation || value.body.connection !== connection || !p.equal(value.body.pairing, savedPairing)) throw new Error("Native challenge mismatch");
          challenge = value.body; const code = await p.comparison(challenge);
          if (!current(owned, epoch)) return;
          comparison = code;
          if (record) await authenticate();
          else { phase = "pending"; state = "Compare this code and approve once in Windows Settings"; schedule(); }
        } else if (p.object(value, ["type", "version", "challenge", "pairing", "credential"]) && value.type === "issued" && value.version === 10 && p.challenge(value.challenge) && p.pairing(value.pairing) && p.hex(value.credential)) {
          const next = value.challenge;
          if (phase !== "pending" || record || !challenge || next.session !== challenge.session || next.challenge !== challenge.challenge || next.nonce !== challenge.nonce || next.installation !== installation || next.connection !== connection || !p.equal(next.pairing, value.pairing)) throw new Error("Issued pairing mismatch");
          record = { version: 1, installation, pairing: value.pairing, credential: value.credential };
          // A disconnect while persistence is in flight may leave a saved unconfirmed
          // record. Never rollback/reissue automatically or publish for the old port.
          await p.persist(record);
          if (!current(owned, epoch)) return;
          savedPairing = record.pairing; challenge = next; await authenticate();
        } else if (p.object(value, ["type", "body"]) && value.type === "authenticated" && p.object(value.body, ["version", "session", "installation", "connection", "pairing", "generation"])) {
          const body = value.body;
          if (phase !== "proof" || !challenge || !record || body.version !== 10 || body.session !== challenge.session || body.installation !== installation || body.connection !== connection || !p.pairing(body.pairing) || !p.equal(body.pairing, record.pairing) || !Number.isSafeInteger(body.generation) || Number(body.generation) <= 0) throw new Error("Authentication reply mismatch");
          if (nativeGeneration !== null && body.generation !== nativeGeneration) throw new Error("Native generation changed during authentication");
          nativeGeneration = Number(body.generation);
          phase = "authenticated"; retrySeconds=1; stopRetry(); connected = true; comparison = ""; state = "Paired connection ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â· page operations unavailable"; displayPhase = "Paired"; tone = "paired"; schedule();
        } else if (p.object(value, ["type", "body"]) && value.type === "status" && p.status(value.body)) {
          const status = value.body;
          if (!challenge || status.session !== challenge.session || status.sequence !== sequence || !["pending", "authenticated"].includes(phase) || status.state !== (phase === "authenticated" ? "authenticated_no_scopes" : "pending") || (nativeGeneration !== null && status.generation !== nativeGeneration)) throw new Error("Native status mismatch");
          if (nativeGeneration === null) nativeGeneration = status.generation;
          if (phase === "authenticated") {
            if (!record) throw new Error("Authenticated pairing missing");
            authority.observe(status.authority);
            acknowledgeRead(status.read_ack, record.pairing);
            reads.acknowledgeMailbox(status.mailbox_ack);
            metadata.observe(status.document,status.session,status.generation);
            // Starts a retained independent promise chain; control polling stays
            // responsive and no popup/setup route can supply a read request.
            reads.observe(status.read,status.session,status.generation,record.pairing);
            scopes.observe(status);
            scopeOutcome = status.scope;
            void scopes.reconcile();
            state = status.authority ? "Selected installation ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â· page operations unavailable" : "Paired connection ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â· page operations unavailable";
            displayPhase = status.authority ? "Selected" : "Paired";
          }
          schedule();
        } else throw new Error("Invalid native response");
      } finally { handling = false; }
    }
    owned.onMessage.addListener((value: unknown) => {
      void receive(value).catch(() => {if(!current(owned,epoch))return;retryBlocked=true;stopRetry();close("Connection or persistence unavailable. Check saved pairings before retrying.", owned, epoch, true);});
    });
    owned.onDisconnect.addListener(() => {
      const failed = chrome.runtime.lastError;
      if (!current(owned, epoch)) return;
      close(failed ? "Native companion unavailable; saved pairing will reconnect automatically unless disabled in Windows Settings." : "Connection ended; reconnecting saved pairing", owned, epoch, !!failed);
      scheduleRetry();
    });
    send({ type: "hello", body: { version: 10, installation, connection, extension: chrome.runtime.id, nonce, pairing: savedPairing } });
  } catch {
    if (generation === epoch) {
      const identity = await p.recoveryIdentity().catch(() => null);
      if (generation === epoch) { savedPairing = identity; retryBlocked=true; stopRetry(); close("Setup or saved pairing unavailable; nothing was replaced.", port, epoch, true); }
    }
  }
  finally { busy = false; }
}
chrome.runtime.onMessage.addListener((message: unknown, sender, reply) => {
  if (sender.id !== chrome.runtime.id || sender.url !== chrome.runtime.getURL("popup.html")) return false;
  if (p.object(message, ["operation"])) {
    if (message.operation === "status") { void permissions?.reconcile(); reply(snapshot()); return false; }
    if (message.operation === "connect") { void userPreference(true).catch(()=>{state="Reconnect preference could not be saved";}).finally(() => reply(snapshot())); return true; }
    if (message.operation === "disconnect") { void userPreference(false).catch(()=>{state="Disconnected; reconnect preference could not be saved";}).finally(()=>reply(snapshot())); return true; }
  }
  if (p.object(message, ["operation", "proposal"]) && message.operation === "permission_intent" && view(message.proposal)) {
    reply({ accepted: permissions?.intent(message.proposal) ?? false }); return false;
  }
  if (p.object(message, ["operation", "proposal"]) && message.operation === "permission_decline" && view(message.proposal)) {
    permissions?.decline(message.proposal); reply(snapshot()); return false;
  }
  if (p.object(message, ["operation", "proposal", "permitted"]) && message.operation === "permission_result" && view(message.proposal) && typeof message.permitted === "boolean") {
    permissions?.completed(message.proposal, message.permitted); reply(snapshot()); return false;
  }
  if (p.object(message, ["operation", "pairing"]) && message.operation === "forget" && p.pairing(message.pairing) && !busy && !preferenceBusy && !p.isWriting()) {
    const pairing = message.pairing;
    retryBlocked=true;preferenceGeneration++;stopRetry();
    close("Removing this installation's saved credential"); busy = true;
    const epoch = generation;
    void p.automaticPreference(false).then(()=>p.forget(pairing)).then(() => { if (generation === epoch) { savedPairing = null; state = "Local credential removed. Revoke its saved revision in Windows Settings too."; } })
      .catch(() => { if (generation === epoch) state = "Local removal uncertain; saved revision was not replaced."; })
      .finally(() => { busy = false; reply(snapshot()); });
    return true;
  }
  return false;
});

// Register synchronously: worker restart discards pending proposal authority.
chrome.permissions.onAdded.addListener(() => { void permissions?.reconcile(); });
chrome.permissions.onRemoved.addListener(() => { retryBlocked=true;stopRetry();close("Browser permission removed; connection authority withdrawn.", port, generation, true); });

// These listeners only invalidate in-memory ownership; no page data is retained.
const invalidateDocuments = () => { reading?.invalidate(); documents?.invalidate(); };
chrome.webNavigation.onBeforeNavigate.addListener(d=>{if(!reading?.creationEvent({kind:"before",tab:d.tabId,frame:d.frameId,url:d.url}))invalidateDocuments();});
chrome.webNavigation.onCommitted.addListener(d=>{if(!reading?.creationEvent({kind:"committed",tab:d.tabId,frame:d.frameId,url:d.url,document:d.documentId}))invalidateDocuments();});
chrome.webNavigation.onErrorOccurred.addListener(invalidateDocuments);
chrome.webNavigation.onHistoryStateUpdated.addListener(d=>{if(!reading?.creationEvent({kind:"history",tab:d.tabId,frame:d.frameId,url:d.url,document:d.documentId}))invalidateDocuments();});
chrome.webNavigation.onReferenceFragmentUpdated.addListener(d=>{if(!reading?.creationEvent({kind:"fragment",tab:d.tabId,frame:d.frameId,url:d.url,document:d.documentId}))invalidateDocuments();});
chrome.webNavigation.onTabReplaced.addListener(invalidateDocuments);
chrome.tabs.onRemoved.addListener(invalidateDocuments);
chrome.tabs.onReplaced.addListener(invalidateDocuments);
chrome.tabs.onCreated.addListener(tab=>{if(!tab.id||!reading?.creationEvent({kind:"created",tab:tab.id,window:tab.windowId,url:tab.url}))invalidateDocuments();});
chrome.tabs.onUpdated.addListener((tab,change,info)=>{if(!reading?.expectedUpdate(tab,change,info)&&!reading?.creationEvent({kind:"updated",tab,window:info.windowId,url:change.url}))invalidateDocuments();});
chrome.tabs.onAttached.addListener(invalidateDocuments);
chrome.tabs.onDetached.addListener(invalidateDocuments);
chrome.tabs.onMoved.addListener(invalidateDocuments);
chrome.tabs.onActivated.addListener(info=>{if(!reading?.expectedActivation(info.tabId,info.windowId))invalidateDocuments();});
chrome.windows.onFocusChanged.addListener(window=>{if(!reading?.expectedFocus(window))invalidateDocuments();});

void activate();
