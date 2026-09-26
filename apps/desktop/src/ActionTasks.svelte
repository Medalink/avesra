<script lang="ts">
  import SelectFrame from "./SelectFrame.svelte";
  import DiskActivity from "./DiskActivity.svelte";
  import type { ComponentProps } from "svelte";
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import SetupLock from "./SetupLock.svelte";
  import { command, native, type Runtime } from "./runtime";
  let { runtime }: { runtime: Runtime | null } = $props();
  type Catalog = "host_resources" | "cisco_vpn_status";
  type VpnProfile = { id:string; revision:string; name:string; address:string };
  type Scope = { id:string; revision:string; origin:string; operations:string[] };
  type ProviderProbe = { provider:"gmail"|"x"; complete:boolean; choices:{id:string;parent:string|null;role:string;label:string;attributes:{name:string;value:string}[]}[] };
  type Inbox={account:string;messages:{id:string;thread:string;timestamp_ms:number;sender:string;subject:string;reference:string;body:string}[];incomplete:string|null};
  type Page = { inbox:Inbox|null; task:string; origin:string; blocks:string[]; truncated:boolean; excluded_content:boolean; remaining_ms:number; provider:ProviderProbe|null; x_ready:{account:string;focused:boolean}|null; x_needs_input:{account:string;reason:"login_required"|"account_mismatch"|"unsupported_page"}|null };
  type DownloadContext={id:string;revision:string;actor:string;input:{application:string;endpoint:string;port:number;destination_drive:string|null;reported_rate_milli:number|null;reported_unit:string;reported_throttle:boolean|null}};
  type DownloadReport={dns_a:string;dns_aaaa:string;dns_micros:number;endpoints:{address:string;route_interface:Reading<number>;tcp_connect_micros:Reading<number>}[];destination_space:Reading<{drive:string;caller_available_bytes:number;caller_total_bytes:number}>;resources:Diagnostic};
  type Target = {kind:"gmail_inbox";scope:Scope;account:string} | {kind:"download";context:DownloadContext;configuration:boolean} | {kind:"x_ready";scope:Scope;account:string} | {kind:"vpn";profile:VpnProfile} | { kind: "browser_read"; scope:Scope } | { kind: "application"; app: string; app_revision: string; alias: string; alias_revision: string } | { kind: "volume"; target: string; endpoint: string } | { kind: "diagnostic"; catalog: Catalog } | { kind: "prompt"; binding: { id: string; project: string; app_name: string } };
  type Reading<T> = { state: "available"; value: T } | { state: "unavailable"; reason: string };
  type Diagnostic = { kind: "cisco_vpn_status"; state: Reading<string> } | { kind: "host_resources"; interval_ms: number; cpu_busy_basis_points: Reading<number>; network: Reading<{ name: string; interface_type: number; operational_status: number; receive_link_bits_per_second: number; transmit_link_bits_per_second: number; receive_bytes_per_second: Reading<number>; transmit_bytes_per_second: Reading<number> }[]>; system_drive_space: Reading<{ drive: string; caller_available_bytes: number; caller_total_bytes: number }>; disk_pressure: Reading<number>; disk_activity?:ComponentProps<typeof DiskActivity>["value"]; default_routes:Reading<{interface_index:number;ipv6:boolean;route_metric:number}[]>; ipv4_dns_servers:Reading<number> };
  type Permission = { permission: { id: string; actor: string; name: string; target: Target }; revoked: boolean };
  type Task = { action_revision:string; expires_at_ms:number; download:DownloadReport|null; task: string; turn: string; revision: string; state: string; target: Target; payload: { kind: string; percent?: number }; outcome: string | null; diagnostic: Diagnostic | null; vpn:{state:string;command_issued:boolean;target_matched:boolean;observed:Reading<string>;owner_step:string|null}|null; created_ms: number };
  type Snapshot = { sequence: number; permissions: Permission[]; aliases: { id: string; revision: string; phrase: string; name: string }[]; tasks: Task[] };
  type Output = { id: string; name: string };
  let panel = $state<string | null>(null);
  let snapshot = $state<Snapshot | null>(null);
  let outputs = $state<Output[]>([]);
  let scopes = $state<Scope[]>([]), scope = $state("");
  let gmailAccount=$state("");
  const gmailScopes=$derived(scopes.filter(s=>s.origin==="https://mail.google.com"&&s.operations.includes("read")&&s.operations.includes("navigate")));
  let xAccount=$state("");
  const xScopes=$derived(scopes.filter(s=>s.origin==="https://x.com"&&s.operations.includes("read")&&s.operations.includes("navigate")));
  let page = $state<Page|null>(null);
  let pageTimer: ReturnType<typeof setTimeout> | undefined;
  let alias = $state("");
  let endpoint = $state("");
  let catalog = $state<Catalog>("host_resources");
  let downloadApp=$state(""), downloadEndpoint=$state(""), downloadDrive=$state(""), downloadRate=$state(""), downloadUnit=$state("megabytes_per_second"), downloadPort=$state(443), downloadThrottle=$state("unknown");
  let vpnProfile=$state<VpnProfile|null>(null);
  type VpnChannel={step:string;dispatch:string;authenticated:boolean;checked_ms:number};
  let vpnChannel=$state<VpnChannel|null>(null);
  let busy = $state(false);
  let cancelling = $state("");
  let error = $state("");
  let notice = $state("");
  type PromptSurface = { app: string; revision: string; complete: boolean; controls: { id: string; identity: { name: string; automation_id: string; control_type: number }; invoke: boolean; writable_value: boolean; readable_text: boolean; claude_route: string | null }[] };
  let promptSurface = $state<PromptSurface | null>(null);
  let projectName = $state("");
  let promptChoices = $state<Record<string,string>>({ project_button: "", project_indicator: "", new_chat: "", prompt: "", route: "" });
  const roles = [{key:"project_button",label:"Project selector"},{key:"project_indicator",label:"Current project heading"},{key:"new_chat",label:"New Chat action"},{key:"prompt",label:"Empty prompt field"},{key:"route",label:"Document project route"}];
  let mounted = false;
  let generation = 0;
  let context = "";
  const enabled = $derived(native && runtime?.connected && !runtime.locked);
  const activePermissions = $derived(snapshot?.permissions.filter(p => !p.revoked) ?? []);
  const downloadPermission=$derived(activePermissions.find(p=>p.permission.target.kind==="download"&&!p.permission.target.configuration));
  const hasVolume = $derived(activePermissions.some(p => p.permission.target.kind === "volume"));
  const hasDiagnostic = $derived(activePermissions.some(p => p.permission.target.kind === "diagnostic" && p.permission.target.catalog === catalog));
  function applySnapshot(value: Snapshot) { if (Number.isSafeInteger(value.sequence) && value.sequence > 0 && (!snapshot || value.sequence >= snapshot.sequence)) snapshot = value; }
  function invalidate() { vpnProfile=null; vpnChannel=null; clearTimeout(pageTimer); page=null; scopes=[]; scope=""; const previous = panel; generation++; panel = null; snapshot = null; promptSurface = null; outputs = []; alias = ""; endpoint = ""; if (previous && native) void command("close_action_panel", { panel: previous }).catch(() => {}); }
  $effect(() => {
    const next = `${runtime?.connected}:${runtime?.locked}:${runtime?.action_epoch}`;
    if (next !== context) { context = next; invalidate(); }
  });
  async function run(work: (current: number) => Promise<void>, mutation = false) {
    if (busy) return;
    busy = true; error = ""; notice = "";
    const current = ++generation;
    try { await work(current); }
    catch (e) { if (mounted && current === generation) error = `${String(e)}${mutation ? " Refresh to inspect the durable result before retrying." : ""}`; }
    finally { busy = false; }
  }
  async function open(current: number) {
    if (panel) return panel;
    const value = await command<string>("open_action_panel");
    if (!mounted || current !== generation) { void command("close_action_panel", { panel: value }).catch(() => {}); return null; }
    panel = value; return value;
  }
  function refresh() { return run(async current => {
    const active = await open(current); if (!active) return;
    const started=performance.now();
    const value = await command<{ snapshot: Snapshot; outputs: Output[]; scopes:Scope[]; page:Page|null; vpn_channel:VpnChannel|null }>("action_status", { panel: active });
    if (mounted && current === generation) { applySnapshot(value.snapshot); vpnChannel=value.vpn_channel; outputs = value.outputs; scopes=value.scopes; clearTimeout(pageTimer); const remaining=(value.page?.remaining_ms??0)-(performance.now()-started); page=remaining>0?value.page:null; if(page) pageTimer=setTimeout(()=>{page=null;},remaining); }
  }); }
  function allowApp() { return run(async current => {
    const selected = snapshot?.aliases.find(a => a.id === alias); if (!panel || !selected) return;
    const value = await command<Snapshot>("grant_app_action", { panel, alias: selected.id, revision: selected.revision });
    if (mounted && current === generation) { applySnapshot(value); notice = "Launch permission saved for this exact application."; }
  }, true); }
  function allowBrowser() { return run(async current=>{
    const selected=scopes.find(s=>s.id===scope); if(!panel||!selected)return;
    const value=await command<Snapshot>("grant_browser_read_action",{panel,reference:{id:selected.id,revision:selected.revision}});
    if(mounted&&current===generation){applySnapshot(value);notice="Partial page read permission saved. Select the exact document in Browser setup before the accepted request.";}
  },true); }
  function allowGmail() {return run(async current=>{
    const selected=gmailScopes.length===1?gmailScopes[0]:gmailScopes.find(s=>s.id===scope); if(!panel||!selected)return;
    const value=await command<Snapshot>("grant_browser_read_action",{panel,reference:{id:selected.id,revision:selected.revision},gmailAccount:gmailAccount.trim()});
    if(mounted&&current===generation){applySnapshot(value);notice="Gmail account saved. Reads require this exact signed-in account; incomplete evidence will be reported.";}
  },true);}
  function inboxReason(reason:string):string {
    return ({account_unverified:"The signed-in account could not be verified.",inbox_membership_unverified:"Individual messages could not be verified as belonging to Inbox.",individual_order_unverified:"The newest individual-message order could not be verified.",date_unverified:"Message dates could not be established unambiguously.",body_incomplete:"Some message bodies were not fully available.",pagination_unverified:"The remaining Inbox pages could not be verified.",provider_unsupported:"This Gmail view could not be inspected reliably.",limit_reached:"This read reached its bounded size or duration limit."} as Record<string,string>)[reason]??"Mailbox evidence is incomplete.";
  }
  function allowX() { return run(async current=>{
    const selected=xScopes.length===1?xScopes[0]:xScopes.find(s=>s.id===scope);
    if(!panel||!selected)return;
    const account=xAccount.trim().replace(/^@/,"").toLowerCase();
    const value=await command<Snapshot>("grant_browser_read_action",{panel,reference:{id:selected.id,revision:selected.revision},xAccount:account});
    if(mounted&&current===generation){applySnapshot(value);notice="X account saved. Say ‘Open X’ to open it in your paired browser. Nothing will be posted.";}
  },true); }
  function inspectPrompt() { return run(async current => {
    promptSurface = null;
    promptChoices = { project_button: "", project_indicator: "", new_chat: "", prompt: "", route: "" };
    const selected = snapshot?.aliases.find(a => a.id === alias); if (!panel || !selected) return;
    const observed = await command<PromptSurface>("inspect_prompt_surface", { panel, alias: selected.id, revision: selected.revision });
    if (mounted && current === generation) promptSurface = observed;
  }); }
  function bindProject() { return run(async current => {
    const selected = snapshot?.aliases.find(a => a.id === alias); if (!panel || !selected || !promptSurface?.complete) return;
    const value = await command<Snapshot>("bind_prompt_project", { panel, alias: selected.id, revision: selected.revision, choices: { project: projectName, ...promptChoices } });
    if (mounted && current === generation) { applySnapshot(value); promptSurface = null; notice = "Project draft permission saved from observed controls. Live drafting remains unverified until an accepted request succeeds."; }
  }, true); }
  function allowVolume() { return run(async current => {
    if (!panel || !endpoint) return;
    const value = await command<Snapshot>("grant_volume_action", { panel, endpoint });
    if (mounted && current === generation) { applySnapshot(value); notice = "Volume permission saved for the selected output. Mute stays unchanged."; }
  }, true); }
  function revoke(id: string) { return run(async current => {
    if (!panel) return;
    const value = await command<Snapshot>("revoke_action_permission", { panel, id });
    if (mounted && current === generation) { applySnapshot(value); notice = "Permission revoked. Completed effects are not undone."; }
  }, true); }
  function allowDiagnostic() { return run(async current => {
    if (!panel) return;
    const value = await command<Snapshot>("grant_diagnostic_action", { panel, catalog });
    if (mounted && current === generation) { applySnapshot(value); notice = "Read permission saved for this fixed diagnostic. It cannot change settings or connect the VPN."; }
  }, true); }
  function inspectVpn(){return run(async current=>{
    const active=await open(current);if(!active)return;
    const value=await command<VpnProfile>("inspect_vpn_profile",{panel:active});
    if(mounted&&current===generation){vpnProfile=value;notice="Saved Cisco target inspected. Review it before allowing connection.";}
  });}
  function allowVpn(){return run(async current=>{
    if(!panel||!vpnProfile)return;
    const value=await command<Snapshot>("grant_vpn_action",{panel,id:vpnProfile.id,revision:vpnProfile.revision});
    if(mounted&&current===generation){applySnapshot(value);vpnProfile=null;notice="Existing VPN target allowed. Say connect work VPN; Cisco retains authentication and MFA.";}
  },true);}
  function allowDownload(){return run(async current=>{
    const active=await open(current);if(!active)return;
    const rate=downloadRate.trim()===""?null:Math.round(Number(downloadRate)*1000);
    if(rate!==null&&(!Number.isSafeInteger(rate)||rate<0))throw new Error("Enter a nonnegative reported rate with its correct unit");
    const value=await command<Snapshot>("grant_download_diagnosis",{panel:active,input:{application:downloadApp.trim(),endpoint:downloadEndpoint.trim().toLowerCase(),port:Number(downloadPort),destination_drive:downloadDrive.trim()===""?null:downloadDrive.trim().toUpperCase(),reported_rate_milli:rate,reported_unit:downloadUnit,reported_throttle:downloadThrottle==="unknown"?null:downloadThrottle==="yes"}});
    if(mounted&&current===generation){applySnapshot(value);notice="Exact download context saved. Say diagnose download to observe it.";}
  },true);}
  function allowDownloadFix(){return run(async current=>{
    const target=downloadPermission?.permission.target;if(!panel||target?.kind!=="download")return;
    const value=await command<Snapshot>("grant_download_fix",{panel,id:target.context.id,revision:target.context.revision});
    if(mounted&&current===generation){applySnapshot(value);notice="DNS-cache action scope saved. Every actual flush still requires a fresh accepted request and exact approval.";}
  },true);}
  function approveDownload(task:Task){return run(async current=>{
    if(!panel)return;const value=await command<Snapshot>("approve_download_fix",{panel,revision:task.action_revision});
    if(mounted&&current===generation){applySnapshot(value);notice="Exact approval recorded. Refresh for the actual command outcome; no automatic retry.";}
  },true);}
  async function cancel(task: Task) {
    if (!panel || cancelling) return;
    const active = panel;
    cancelling = task.task; error = "";
    try {
      await command("cancel_action_task", { panel: active, task: task.task });
      if (mounted && panel === active) notice = "Cancellation requested for this task. Refresh for its observed outcome.";
    } catch (e) { if (mounted && panel === active) error = String(e); }
    finally { cancelling = ""; }
  }
  function label(task: Task) {
    if(task.vpn?.state==="already_connected")return "Already connected to the selected VPN · no command issued";
    if(task.payload.kind==="flush_download_dns"&&task.outcome==="success")return "DNS-cache command completed · repaired download not established";
    if (task.outcome === "success") return task.diagnostic || task.download ? "Diagnostic finished · see available measurements" : "Verified effect";
    if (task.outcome === "unsupported") return "Unsupported target";
    if (task.outcome === "needs_input" || task.state === "waiting_for_user") return "Needs your input";
    if (task.outcome === "unknown_effect" || task.state === "unknown_effect") return "Effect unknown · inspect before retrying";
    return task.state.replaceAll("_", " ");
  }
  function targetName(task: Task) {
    if(task.target.kind==="download")return `${task.target.context.input.application || "Unspecified application"} · ${task.target.context.input.endpoint}`;
    if(task.target.kind==="vpn")return task.target.profile.name;
    if (task.target.kind === "gmail_inbox") return `Inbox for ${task.target.account}`;
    if (task.target.kind === "x_ready") return `X @${task.target.account}`;
    if (task.target.kind === "browser_read") return task.target.scope.origin;
    if (task.target.kind === "diagnostic") return task.target.catalog === "host_resources" ? "Computer performance" : "Cisco VPN status";
    if (task.target.kind === "prompt") return `${task.target.binding.app_name} · ${task.target.binding.project}`;
    const targetId = task.target.kind === "application" ? task.target.app : task.target.target;
    return snapshot?.permissions.find(p => (p.permission.target.kind === "application" ? p.permission.target.app : p.permission.target.kind === "volume" ? p.permission.target.target : "") === targetId)?.permission.name ?? "Saved target";
  }
  function measured<T>(value: Reading<T>, format: (item: T) => string) { return value.state === "available" ? format(value.value) : `Unavailable (${value.reason.replaceAll("_", " ")})`; }
  function transfer(value: Reading<number>) { return measured(value, bytes => `${(bytes / 1_000_000).toFixed(2)} MB/s · ${(bytes * 8 / 1_000_000).toFixed(2)} Mbps`); }
  onMount(() => {
    mounted = true;
    let unlisten: (() => void) | undefined;
    let unlistenTasks: (() => void) | undefined;
    if (native) void (async () => { const stop = await listen("settings-hidden", invalidate); if (!mounted) { stop(); return; } unlisten = stop; if (enabled) await refresh(); })().catch(e => { if (mounted) error = String(e); });
    if (native) void listen<{ panel: string; snapshot: Snapshot }>("action-task-snapshot", event => {
      if (mounted && enabled && panel === event.payload.panel) applySnapshot(event.payload.snapshot);
    }).then(stop => { if (mounted) unlistenTasks = stop; else stop(); }).catch(e => { if (mounted) error = String(e); });
    return () => { mounted = false; generation++; unlisten?.(); unlistenTasks?.(); if (panel) void command("close_action_panel", { panel }).catch(() => {}); };
  });
</script>

<section class="section">
  <div class="flex items-center justify-between"><span class="av-kicker">Action permissions & tasks</span><button class="av-btn av-btn-ghost av-btn-sm" disabled={!enabled || busy} onclick={refresh}>{busy ? "Working…" : "Refresh"}</button></div>
  <p class="av-hint">Saving a learned name does not allow actions. These permissions apply only to accepted requests; normal spoken acceptance still requires qualification.</p>
  <SetupLock {runtime} purpose="action permissions" />
  <label class="av-label" for="action-app">Allow opening a saved application</label>
  <div class="flex gap-2"><SelectFrame grow><select id="action-app" class="av-input av-select" bind:value={alias} disabled={!enabled || busy || !snapshot}><option value="">Choose a learned name</option>{#each snapshot?.aliases ?? [] as item}<option value={item.id}>{item.phrase} · {item.name}</option>{/each}</select></SelectFrame><button class="av-btn av-btn-secondary" disabled={!enabled || busy || !alias} onclick={allowApp}>Allow launch</button></div>
  <button class="av-btn av-btn-ghost av-btn-sm self-start" disabled={!enabled || busy || !alias} onclick={inspectPrompt}>Inspect prompt controls</button>
  <p class="av-hint">Reads accessible controls in one already-open matching window. It does not launch, focus, type or bind a project.</p>
  {#if promptSurface}<details><summary class="av-hint">Observed controls · {promptSurface.complete ? "bounded scan complete" : "partial scan; cannot bind"} · project workflow not configured</summary>{#each promptSurface.controls as control}<p class="av-hint">{control.identity.name || control.identity.automation_id || "Unnamed control"} · type {control.identity.control_type} · {control.invoke ? "invoke " : ""}{control.writable_value ? "writable value " : ""}{control.readable_text ? "readable text" : ""}</p>{/each}</details>{/if}
  {#if promptSurface?.complete}<div class="av-card p-4"><h3>Bind an observed Claude project</h3><p class="av-hint">Open the intended project yourself, verify the empty composer, then select its observed controls. Requires a readable Claude document route and writable ValuePattern. Text-only editors remain unsupported; this does not grant keyboard handoff. Inspection expires after 30 seconds.</p><label class="av-label" for="prompt-project">Project name as displayed</label><input id="prompt-project" class="av-input" bind:value={projectName} disabled={busy} />{#each roles as role}<label class="av-label" for={`prompt-${role.key}`}>{role.label}</label><SelectFrame><select id={`prompt-${role.key}`} class="av-input av-select" bind:value={promptChoices[role.key]} disabled={busy}><option value="">Choose observed control</option>{#each promptSurface.controls.filter(c => role.key === "new_chat" ? c.invoke && c.identity.name === "New Chat" : role.key === "project_button" ? c.invoke : role.key === "project_indicator" ? c.identity.control_type === 50020 : role.key === "prompt" ? c.writable_value : !!c.claude_route) as control}<option value={control.id}>{control.identity.name || control.identity.automation_id || "Unnamed control"}{control.claude_route ? ` · ${control.claude_route}` : ""}</option>{/each}</select></SelectFrame>{/each}<button class="av-btn av-btn-secondary mt-3" disabled={!enabled || busy || !projectName.trim() || roles.some(r => !promptChoices[r.key])} onclick={bindProject}>Bind & allow unsubmitted drafts</button><p class="av-hint">Accepted format: “Open Claude for the IRIS project and type 123 into a new chat”. This permission cannot submit.</p></div>{/if}
  <label class="av-label" for="action-output">Allow setting speakers volume</label>
  <label class="av-label" for="action-browser">Allow a partial page read</label>
  <div class="flex gap-2"><SelectFrame grow><select id="action-browser" class="av-input av-select" bind:value={scope} disabled={!enabled || busy}><option value="">Choose an existing browser Read scope</option>{#each scopes.filter(s=>s.operations.includes("read")) as item}<option value={item.id}>{item.origin}</option>{/each}</select></SelectFrame><button class="av-btn av-btn-secondary" disabled={!enabled || busy || !panel || !scope || activePermissions.some(p=>p.permission.target.kind==="browser_read"&&p.permission.target.scope.id===scope)} onclick={allowBrowser}>Allow page read</button></div>
  <p class="av-hint">Select the exact document in Browser setup, then say “Read page at https://example.com” using your saved origin. This reads a partial excerpt; it cannot count emails or verify an account. Refresh here to view the temporary result.</p>
  <div class="av-card p-4"><h3>X account</h3><p class="av-hint">Uses your paired browser and saved X site permission. Nothing is posted or typed.</p>
    {#if xScopes.length===0}<p class="av-hint">Choose your browser and allow X reading and navigation in Browser setup first.</p>{/if}
    {#if xScopes.length>1}<label class="av-label" for="x-browser">Browser</label><SelectFrame><select id="x-browser" class="av-input av-select" bind:value={scope} disabled={busy}><option value="">Choose your X browser</option>{#each xScopes as item}<option value={item.id}>{item.origin}</option>{/each}</select></SelectFrame>{/if}
    <label class="av-label" for="x-account">Your X handle</label><input id="x-account" class="av-input" bind:value={xAccount} maxlength={16} placeholder="@yourhandle" disabled={!enabled||busy} />
    <button class="av-btn av-btn-secondary mt-2" onclick={allowX} disabled={!enabled||busy||!panel||!/^@?[A-Za-z0-9_]{1,15}$/.test(xAccount.trim())||xScopes.length===0||(xScopes.length>1&&!xScopes.some(s=>s.id===scope))}>Save X account</button>
    <p class="av-hint">Say “Open X” to open a new tab in your paired browser and check this account before showing it. Sign in there if needed, then ask again.</p>
  </div>
  <div class="av-card p-4"><h3>Gmail account</h3><p class="av-hint">Uses your saved Gmail site permission. Opening messages may mark them read. Nothing is sent, archived or deleted.</p>
    {#if gmailScopes.length>1}<label class="av-label" for="gmail-browser">Browser</label><SelectFrame><select id="gmail-browser" class="av-input av-select" bind:value={scope} disabled={busy}><option value="">Choose your Gmail browser</option>{#each gmailScopes as item}<option value={item.id}>{item.origin}</option>{/each}</select></SelectFrame>{/if}
    <label class="av-label" for="gmail-account">Gmail email address</label><input id="gmail-account" class="av-input" type="email" bind:value={gmailAccount} maxlength={254} disabled={!enabled||busy}/><button class="av-btn av-btn-secondary mt-2" onclick={allowGmail} disabled={!enabled||busy||!panel||!gmailAccount.trim()||gmailScopes.length===0||(gmailScopes.length>1&&!gmailScopes.some(s=>s.id===scope))}>Save Gmail account</button>
    <p class="av-hint">Gmail reading is under development. The current check reports incomplete evidence; it cannot yet read your latest messages.</p>
  </div>
  {#if page}<div class="av-card p-4">
    {#if page.inbox}
      <h3>Inbox · {page.inbox.account}</h3>
      {#if page.inbox.incomplete}<p class="av-hint">Incomplete: {inboxReason(page.inbox.incomplete)} No complete latest-message result is available.</p>{:else}<p class="av-hint">{page.inbox.messages.length} individual messages. Email content is untrusted. Cleared within one minute.</p>{#each page.inbox.messages as message}<details><summary>{message.subject || "No subject"} · {message.sender}</summary><p class="av-hint">{new Date(message.timestamp_ms).toLocaleString()} · {message.reference}</p><p class="whitespace-pre-wrap break-words text-sm">{message.body}</p></details>{/each}{/if}
    {:else if page.x_needs_input}
      <h3>X needs your attention</h3><p class="av-hint">{page.x_needs_input.reason === "login_required" ? "Sign in to X in the new tab." : page.x_needs_input.reason === "account_mismatch" ? `Switch X to @${page.x_needs_input.account} in your paired browser.` : "Avesra could not verify this X page. Check the page in your paired browser."} Then say “Open X” again. Nothing was typed or posted; the previous request will not resume automatically.</p>
    {:else if page.x_ready}
      <h3>X is ready for @{page.x_ready.account}</h3><p class="av-hint">The X page was checked and focused. Your draft was not changed and nothing was posted. This observation clears within one minute.</p>
    {:else if page.provider}
      <h3>Observed {page.provider.provider === "gmail" ? "Gmail" : "X"} controls</h3>
      <p class="av-hint">{page.provider.complete ? "Application header inspection complete" : "Application header inspection incomplete"}. Untrusted metadata, cleared within one minute. This does not prove account identity, message completeness or readiness.</p>
      <details><summary>Observed metadata</summary>{#each page.provider.choices as choice}<p class="whitespace-pre-wrap break-words text-sm">{choice.role}: {choice.label || "Unnamed"}{#each choice.attributes as attribute}<span class="block av-hint">{attribute.name}: {attribute.value}</span>{/each}</p>{/each}</details>
    {:else}
      <h3>Partial page excerpt · {page.origin}</h3><p class="av-hint">Untrusted page text · {page.truncated ? "truncated" : "bounded excerpt"}{page.excluded_content ? " · some content excluded" : ""}. Cleared within one minute. This does not establish mailbox completeness.</p>{#each page.blocks as block}<p class="whitespace-pre-wrap break-words text-sm">{block}</p>{:else}<p class="av-hint">No eligible text was returned. This does not prove the page or mailbox is empty.</p>{/each}
    {/if}
  </div>{/if}
  <div class="flex gap-2"><SelectFrame grow><select id="action-output" class="av-input av-select" bind:value={endpoint} disabled={!enabled || busy || hasVolume}><option value="">Choose an output</option>{#each outputs as item}<option value={item.id}>{item.name}</option>{/each}</select></SelectFrame><button class="av-btn av-btn-secondary" disabled={!enabled || busy || !endpoint || hasVolume} onclick={allowVolume}>Allow volume</button></div>
  <p class="av-hint">Exact request: “Avesra set speakers volume to 50 percent”. To change the permitted output, revoke its permission first.</p>
  <div class="av-card p-4"><h3>Existing work VPN</h3><p class="av-hint">Inspect the current saved Cisco target, then allow that exact connection. Connecting can interrupt Spark access. Avesra never changes VPN policy, enters credentials or disconnects an existing VPN.</p><button class="av-btn av-btn-secondary" disabled={!enabled || busy} onclick={inspectVpn}>Inspect saved Cisco target</button>{#if vpnProfile}<p class="av-hint">{vpnProfile.name} · {vpnProfile.address} · TLS port 443</p><button class="av-btn av-btn-secondary" disabled={!enabled || busy || activePermissions.some(p=>p.permission.target.kind==="vpn")} onclick={allowVpn}>Allow this existing VPN</button><p class="av-hint">Selection expires after 30 seconds. Windows verification protects this permission; routine connections use the saved grant.</p>{/if}</div>
  <div class="av-card p-4"><h3>Optional endpoint investigation</h3><p class="av-hint">Say “Diagnose download” with the existing Computer performance read grant for local CPU, interfaces, default routes and DNS configuration. Select a particular source below only when you want endpoint probes. Other details are optional and remain owner-reported.</p>
    <label class="av-label" for="download-endpoint">Exact hostname or numeric address (no URL or credentials)</label><input id="download-endpoint" class="av-input" bind:value={downloadEndpoint} disabled={busy} maxlength="253" />
    <label class="av-label" for="download-port">Source port</label><SelectFrame><select id="download-port" class="av-input av-select" bind:value={downloadPort} disabled={busy}><option value={443}>443</option><option value={80}>80</option></select></SelectFrame>
    <details><summary>Optional reported context</summary><label class="av-label" for="download-app">Application / download (optional)</label><input id="download-app" class="av-input" bind:value={downloadApp} disabled={busy} maxlength="128" />
    <label class="av-label" for="download-drive">Destination drive letter (optional)</label><input id="download-drive" class="av-input" bind:value={downloadDrive} maxlength="1" disabled={busy} />
    <label class="av-label" for="download-rate">Reported displayed rate (optional)</label><input id="download-rate" class="av-input" bind:value={downloadRate} disabled={busy} inputmode="decimal" />
    <label class="av-label" for="download-unit">Displayed unit</label><SelectFrame><select id="download-unit" class="av-input av-select" bind:value={downloadUnit} disabled={busy}><option value="megabytes_per_second">MB/s (decimal)</option><option value="megabits_per_second">Mbps</option><option value="bytes_per_second">bytes/s</option></select></SelectFrame>
    <label class="av-label" for="download-throttle">App throttle visibly enabled?</label><SelectFrame><select id="download-throttle" class="av-input av-select" bind:value={downloadThrottle} disabled={busy}><option value="unknown">Unknown</option><option value="yes">Yes, observed by me</option><option value="no">No, observed by me</option></select></SelectFrame>
    </details>
    <button class="av-btn av-btn-secondary" disabled={!enabled||busy||!downloadEndpoint.trim()||!!downloadPermission} onclick={allowDownload}>Save context & allow diagnosis</button>
    <p class="av-hint">Say “Diagnose download”. To change context, revoke both old download permissions first. Queries only this endpoint; no speed-test download or cache inspection.</p>
    {#if downloadPermission}<button class="av-btn av-btn-secondary" disabled={!enabled||busy||activePermissions.some(p=>p.permission.target.kind==="download"&&p.permission.target.configuration)} onclick={allowDownloadFix}>Allow exact DNS-cache proposals</button><p class="av-hint">This only permits proposals. After a measured DNS failure, say “Clear download DNS cache” and approve the exact pending action below. Clearing the system-wide resolver cache may temporarily increase lookup traffic and is not guaranteed to fix the problem.</p>{/if}
  </div>
  <label class="av-label" for="action-diagnostic">Allow a fixed read-only diagnostic</label>
  <div class="flex gap-2"><SelectFrame grow><select id="action-diagnostic" class="av-input av-select" bind:value={catalog} disabled={!enabled || busy}><option value="host_resources">Computer performance</option><option value="cisco_vpn_status">Cisco VPN status</option></select></SelectFrame><button class="av-btn av-btn-secondary" disabled={!enabled || busy || !panel || hasDiagnostic} onclick={allowDiagnostic}>Allow read</button></div>
  <p class="av-hint">Accepted requests: “Check computer performance” or “Check VPN status”. These observe bounded system counters or Cisco status; they do not run scripts or change settings.</p>
  {#each activePermissions as item}<div class="row"><div><h3>{item.permission.name}</h3><p class="av-hint">{item.permission.target.kind === "gmail_inbox" ? `Read Inbox for ${item.permission.target.account}; no sending or deletion` : item.permission.target.kind === "x_ready" ? `Open X for @${item.permission.target.account}; no posting` : item.permission.target.kind === "download" ? `Selected ${item.permission.target.context.input.endpoint}: ${item.permission.target.configuration ? "DNS-cache proposal scope; exact approval required" : "read-only diagnosis"}` : item.permission.target.kind === "vpn" ? `Connect existing Cisco target ${item.permission.target.profile.name} (${item.permission.target.profile.address})` : item.permission.target.kind === "browser_read" ? `Partial excerpt from ${item.permission.target.scope.origin}` : item.permission.target.kind === "prompt" ? `Prepare unsubmitted drafts in ${item.permission.target.binding.app_name}` : item.permission.target.kind === "diagnostic" ? "Fixed read-only diagnostic; no configuration changes" : item.permission.target.kind === "application" ? "Open this exact saved application" : outputs.find(o => o.id === (item.permission.target.kind === "volume" ? item.permission.target.endpoint : ""))?.name ?? "Saved speakers endpoint"}</p></div><button class="av-btn av-btn-ghost av-btn-sm" disabled={!enabled || busy} onclick={() => revoke(item.permission.id)}>Revoke</button></div>{/each}
  {#if vpnChannel}<p class="av-hint">Post-VPN Spark check at {new Date(vpnChannel.checked_ms).toLocaleTimeString()}: {vpnChannel.authenticated ? "paired server authenticated" : "unavailable within 3 seconds; routing cause is not established"}. This is a separate channel observation, not proof of VPN connection.</p>{/if}
  <div class="line"></div><h2>Recent accepted actions</h2>
  {#if !snapshot}<p class="av-hint">Refresh to inspect durable task state.</p>{:else if snapshot.tasks.length === 0}<p class="av-hint">No accepted action tasks for the current owner.</p>{:else}{#each snapshot.tasks as task}<div class="row"><div><h3>{task.payload.kind === "diagnose_download" ? `Diagnose ${targetName(task)}` : task.payload.kind === "flush_download_dns" ? `Clear DNS cache for diagnosis of ${targetName(task)}` : task.payload.kind === "connect_vpn" ? `Connect ${targetName(task)}` : task.payload.kind === "read_page" ? `Read partial page at ${targetName(task)}` : task.payload.kind === "fill_prompt" ? `Prepare unsubmitted draft in ${targetName(task)}` : task.payload.kind === "read_inbox" ? `Read ${targetName(task)}` : task.payload.kind === "diagnostic" ? targetName(task) : task.payload.kind === "set_volume" ? `Set ${targetName(task)} to ${task.payload.percent}%` : `Open ${targetName(task)}`}</h3><p class="av-hint">{label(task)} · {new Date(task.created_ms).toLocaleString()}</p>
      {#if task.payload.kind==="flush_download_dns"&&task.state==="awaiting_approval"}<p class="av-hint">Exact proposal: run Windows ipconfig /flushdns once, clearing the system-wide DNS resolver cache. Bound to this download's recent measured DNS failure. A stale cache is only a hypothesis; improvement is not guaranteed. Expires {new Date(task.expires_at_ms).toLocaleTimeString()}. Verify Windows, then approve this revision.</p><button class="av-btn av-btn-secondary" disabled={!enabled||busy} onclick={()=>approveDownload(task)}>Approve this DNS-cache action</button>{/if}
      {#if task.download}
        <p class="av-hint">DNS A: {task.download.dns_a.replaceAll("_"," ")} · AAAA: {task.download.dns_aaaa.replaceAll("_"," ")} · {(task.download.dns_micros/1000).toFixed(1)} ms. These queries do not prove cache corruption.</p>
        {#each task.download.endpoints as endpoint}<p class="av-hint">{endpoint.address}: route {measured(endpoint.route_interface,v=>`interface ${v}`)} · TCP connection {measured(endpoint.tcp_connect_micros,v=>`${(v/1000).toFixed(1)} ms`)}. TCP timing is not TLS or download throughput.</p>{/each}
        <p class="av-hint">Destination {measured(task.download.destination_space,v=>`${v.drive}: ${(v.caller_available_bytes/1e9).toFixed(2)} GB available under your quota`)}</p>
        {#if task.download.resources.kind==="host_resources"}<DiskActivity value={task.download.resources.disk_activity}/><p class="av-hint">CPU (primary group): {measured(task.download.resources.cpu_busy_basis_points,v=>`${(v/100).toFixed(1)}%`)} · interval {(task.download.resources.interval_ms/1000).toFixed(2)}s · legacy disk-pressure estimate {measured(task.download.resources.disk_pressure,v=>`${v/100}%`)}</p>{#if task.download.resources.network.state==="available"}{#each task.download.resources.network.value as network}<p class="av-hint">{network.name}: received {transfer(network.receive_bytes_per_second)} · link {(network.receive_link_bits_per_second/1e6).toFixed(0)} Mbps</p>{/each}{:else}<p class="av-hint">Interface counters unavailable.</p>{/if}{/if}
        {#if task.target.kind==="download"}<p class="av-hint">Owner-reported rate: {task.target.context.input.reported_rate_milli===null?"unknown":`${task.target.context.input.reported_rate_milli/1000} ${task.target.context.input.reported_unit.replaceAll("_"," ")}`} · owner-reported app throttle: {task.target.context.input.reported_throttle===null?"unknown":task.target.context.input.reported_throttle?"enabled":"disabled"}.</p>{/if}
        <p class="av-hint">Interface traffic includes other apps. Server limits, app throttling and disk pressure remain unproven causes where no corresponding measurement is available.</p>
      {/if}
      {#if task.vpn}<p class="av-hint">Cisco: {measured(task.vpn.observed, state=>state.replaceAll("_"," "))} · {task.vpn.target_matched ? "selected peer matched" : "selected peer not verified"}. {task.vpn.command_issued ? "A connection command may have reached the VPN agent; it is not retried automatically." : "No connection command issued."}</p>{#if task.vpn.owner_step}<p class="av-hint">Your next step: {task.vpn.owner_step.replaceAll("_"," ")}. Continue in Cisco yourself, then inspect status. Avesra does not receive your credentials or MFA response.</p>{/if}{/if}
      {#if task.diagnostic}
        {#if task.diagnostic.kind === "cisco_vpn_status"}
          <p class="av-hint">Cisco: {measured(task.diagnostic.state, state => state.replaceAll("_", " "))}. This does not establish Spark reachability.</p>
        {:else}
          <p class="av-hint">CPU busy (primary processor group): {measured(task.diagnostic.cpu_busy_basis_points, value => `${(value / 100).toFixed(1)}%`)} · sampled over {(task.diagnostic.interval_ms / 1000).toFixed(2)} s</p>
          <p class="av-hint">{measured(task.diagnostic.system_drive_space, disk => `${disk.drive}: system drive · ${(disk.caller_available_bytes / 1_000_000_000).toFixed(2)} GB available of ${(disk.caller_total_bytes / 1_000_000_000).toFixed(2)} GB under your quota`)}</p>
          <p class="av-hint">Configured IPv4 DNS server count: {measured(task.diagnostic.ipv4_dns_servers,v=>String(v))}. IPv6 DNS configuration is outside this API scope.</p>
          {#if task.diagnostic.default_routes.state==="available"}{#each task.diagnostic.default_routes.value as route}<p class="av-hint">Default IPv{route.ipv6?6:4} route: interface {route.interface_index}, route metric {route.route_metric}. This is not the combined interface metric or proof of reachability.</p>{:else}<p class="av-hint">No default route observed.</p>{/each}{:else}<p class="av-hint">Default routes unavailable ({task.diagnostic.default_routes.reason}).</p>{/if}
          <DiskActivity value={task.diagnostic.disk_activity}/><p class="av-hint">Legacy disk-pressure estimate: {measured(task.diagnostic.disk_pressure, value => `${(value / 100).toFixed(1)}%`)}</p>
          {#if task.diagnostic.network.state === "available"}
            <details><summary class="av-hint">Network interface counters ({task.diagnostic.network.value.length})</summary>
              {#each task.diagnostic.network.value as network, index}
                <p class="av-hint">{network.name || `Interface ${index + 1}`} · type {network.interface_type} · {network.operational_status === 1 ? "up" : "not up"} · link receive {(network.receive_link_bits_per_second / 1_000_000).toFixed(0)} Mbps / transmit {(network.transmit_link_bits_per_second / 1_000_000).toFixed(0)} Mbps</p>
                <p class="av-hint">Receive {transfer(network.receive_bytes_per_second)} · Transmit {transfer(network.transmit_bytes_per_second)}</p>
              {/each}
            </details>
          {:else}<p class="av-hint">Network: unavailable ({task.diagnostic.network.reason.replaceAll("_", " ")})</p>{/if}
          <p class="av-hint">Interface traffic includes other apps and may include virtual adapters. These counters do not measure the download server or identify the cause of a slow download.</p>
        {/if}
      {/if}
      </div>{#if ["queued", "running", "awaiting_approval", "waiting_for_user", "suspended"].includes(task.state)}<button class="av-btn av-btn-ghost av-btn-sm" disabled={!enabled || cancelling !== ""} onclick={() => cancel(task)}>{cancelling === task.task ? "Cancelling…" : "Cancel task"}</button>{/if}</div>{/each}{/if}
  {#if error}<p class="av-hint text-amber-200" role="status">{error}</p>{/if}{#if notice}<p class="av-hint" role="status">{notice}</p>{/if}
</section>
