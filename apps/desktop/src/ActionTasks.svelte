<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import SetupLock from "./SetupLock.svelte";
  import { command, native, type Runtime } from "./runtime";
  let { runtime }: { runtime: Runtime | null } = $props();
  type Catalog = "host_resources" | "cisco_vpn_status";
  type Scope = { id:string; revision:string; origin:string; operations:string[] };
  type Page = { task:string; origin:string; blocks:string[]; truncated:boolean; excluded_content:boolean; remaining_ms:number };
  type Target = { kind: "browser_read"; scope:Scope } | { kind: "application"; app: string; app_revision: string; alias: string; alias_revision: string } | { kind: "volume"; target: string; endpoint: string } | { kind: "diagnostic"; catalog: Catalog } | { kind: "prompt"; binding: { id: string; project: string; app_name: string } };
  type Reading<T> = { state: "available"; value: T } | { state: "unavailable"; reason: string };
  type Diagnostic = { kind: "cisco_vpn_status"; state: Reading<string> } | { kind: "host_resources"; interval_ms: number; cpu_busy_basis_points: Reading<number>; network: Reading<{ name: string; interface_type: number; operational_status: number; receive_link_bits_per_second: number; transmit_link_bits_per_second: number; receive_bytes_per_second: Reading<number>; transmit_bytes_per_second: Reading<number> }[]>; system_drive_space: Reading<{ drive: string; caller_available_bytes: number; caller_total_bytes: number }>; disk_pressure: Reading<number> };
  type Permission = { permission: { id: string; actor: string; name: string; target: Target }; revoked: boolean };
  type Task = { task: string; turn: string; revision: string; state: string; target: Target; payload: { kind: string; percent?: number }; outcome: string | null; diagnostic: Diagnostic | null; created_ms: number };
  type Snapshot = { sequence: number; permissions: Permission[]; aliases: { id: string; revision: string; phrase: string; name: string }[]; tasks: Task[] };
  type Output = { id: string; name: string };
  let panel = $state<string | null>(null);
  let snapshot = $state<Snapshot | null>(null);
  let outputs = $state<Output[]>([]);
  let scopes = $state<Scope[]>([]), scope = $state("");
  let page = $state<Page|null>(null);
  let pageTimer: ReturnType<typeof setTimeout> | undefined;
  let alias = $state("");
  let endpoint = $state("");
  let catalog = $state<Catalog>("host_resources");
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
  const hasVolume = $derived(activePermissions.some(p => p.permission.target.kind === "volume"));
  const hasDiagnostic = $derived(activePermissions.some(p => p.permission.target.kind === "diagnostic" && p.permission.target.catalog === catalog));
  function applySnapshot(value: Snapshot) { if (Number.isSafeInteger(value.sequence) && value.sequence > 0 && (!snapshot || value.sequence >= snapshot.sequence)) snapshot = value; }
  function invalidate() { clearTimeout(pageTimer); page=null; scopes=[]; scope=""; const previous = panel; generation++; panel = null; snapshot = null; promptSurface = null; outputs = []; alias = ""; endpoint = ""; if (previous && native) void command("close_action_panel", { panel: previous }).catch(() => {}); }
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
    const value = await command<{ snapshot: Snapshot; outputs: Output[]; scopes:Scope[]; page:Page|null }>("action_status", { panel: active });
    if (mounted && current === generation) { applySnapshot(value.snapshot); outputs = value.outputs; scopes=value.scopes; clearTimeout(pageTimer); const remaining=(value.page?.remaining_ms??0)-(performance.now()-started); page=remaining>0?value.page:null; if(page) pageTimer=setTimeout(()=>{page=null;},remaining); }
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
    if (task.outcome === "success") return task.diagnostic ? "Diagnostic finished · see available measurements" : "Verified effect";
    if (task.outcome === "unsupported") return "Unsupported target";
    if (task.outcome === "needs_input" || task.state === "waiting_for_user") return "Needs your input";
    if (task.outcome === "unknown_effect" || task.state === "unknown_effect") return "Effect unknown · inspect before retrying";
    return task.state.replaceAll("_", " ");
  }
  function targetName(task: Task) {
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
  <div class="flex gap-2"><select id="action-app" class="av-select flex-1" bind:value={alias} disabled={!enabled || busy || !snapshot}><option value="">Choose a learned name</option>{#each snapshot?.aliases ?? [] as item}<option value={item.id}>{item.phrase} · {item.name}</option>{/each}</select><button class="av-btn av-btn-secondary" disabled={!enabled || busy || !alias} onclick={allowApp}>Allow launch</button></div>
  <button class="av-btn av-btn-ghost av-btn-sm self-start" disabled={!enabled || busy || !alias} onclick={inspectPrompt}>Inspect prompt controls</button>
  <p class="av-hint">Reads accessible controls in one already-open matching window. It does not launch, focus, type or bind a project.</p>
  {#if promptSurface}<details><summary class="av-hint">Observed controls · {promptSurface.complete ? "bounded scan complete" : "partial scan; cannot bind"} · project workflow not configured</summary>{#each promptSurface.controls as control}<p class="av-hint">{control.identity.name || control.identity.automation_id || "Unnamed control"} · type {control.identity.control_type} · {control.invoke ? "invoke " : ""}{control.writable_value ? "writable value " : ""}{control.readable_text ? "readable text" : ""}</p>{/each}</details>{/if}
  {#if promptSurface?.complete}<div class="av-card p-4"><h3>Bind an observed Claude project</h3><p class="av-hint">Open the intended project yourself, verify the empty composer, then select its observed controls. Requires a readable Claude document route and writable ValuePattern. Text-only editors remain unsupported; this does not grant keyboard handoff. Inspection expires after 30 seconds.</p><label class="av-label" for="prompt-project">Project name as displayed</label><input id="prompt-project" class="av-input" bind:value={projectName} disabled={busy} />{#each roles as role}<label class="av-label" for={`prompt-${role.key}`}>{role.label}</label><select id={`prompt-${role.key}`} class="av-select" bind:value={promptChoices[role.key]} disabled={busy}><option value="">Choose observed control</option>{#each promptSurface.controls.filter(c => role.key === "new_chat" ? c.invoke && c.identity.name === "New Chat" : role.key === "project_button" ? c.invoke : role.key === "project_indicator" ? c.identity.control_type === 50020 : role.key === "prompt" ? c.writable_value : !!c.claude_route) as control}<option value={control.id}>{control.identity.name || control.identity.automation_id || "Unnamed control"}{control.claude_route ? ` · ${control.claude_route}` : ""}</option>{/each}</select>{/each}<button class="av-btn av-btn-secondary mt-3" disabled={!enabled || busy || !projectName.trim() || roles.some(r => !promptChoices[r.key])} onclick={bindProject}>Bind & allow unsubmitted drafts</button><p class="av-hint">Accepted format: “Open Claude for the IRIS project and type 123 into a new chat”. This permission cannot submit.</p></div>{/if}
  <label class="av-label" for="action-output">Allow setting speakers volume</label>
  <label class="av-label" for="action-browser">Allow a partial page read</label>
  <div class="flex gap-2"><select id="action-browser" class="av-select flex-1" bind:value={scope} disabled={!enabled || busy}><option value="">Choose an existing browser Read scope</option>{#each scopes.filter(s=>s.operations.includes("read")) as item}<option value={item.id}>{item.origin}</option>{/each}</select><button class="av-btn av-btn-secondary" disabled={!enabled || busy || !panel || !scope || activePermissions.some(p=>p.permission.target.kind==="browser_read"&&p.permission.target.scope.id===scope)} onclick={allowBrowser}>Allow page read</button></div>
  <p class="av-hint">Select the exact document in Browser setup, then say “Read page at https://example.com” using your saved origin. This reads a partial excerpt; it cannot count emails or verify an account. Refresh here to view the temporary result.</p>
  {#if page}<div class="av-card p-4"><h3>Partial page excerpt · {page.origin}</h3><p class="av-hint">Untrusted page text · {page.truncated ? "truncated" : "bounded excerpt"}{page.excluded_content ? " · some content excluded" : ""}. Cleared within one minute. This does not establish mailbox completeness.</p>{#each page.blocks as block}<p class="whitespace-pre-wrap break-words text-sm">{block}</p>{:else}<p class="av-hint">No eligible text was returned. This does not prove the page or mailbox is empty.</p>{/each}</div>{/if}
  <div class="flex gap-2"><select id="action-output" class="av-select flex-1" bind:value={endpoint} disabled={!enabled || busy || hasVolume}><option value="">Choose an output</option>{#each outputs as item}<option value={item.id}>{item.name}</option>{/each}</select><button class="av-btn av-btn-secondary" disabled={!enabled || busy || !endpoint || hasVolume} onclick={allowVolume}>Allow volume</button></div>
  <p class="av-hint">Exact request: “Avesra set speakers volume to 50 percent”. To change the permitted output, revoke its permission first.</p>
  <label class="av-label" for="action-diagnostic">Allow a fixed read-only diagnostic</label>
  <div class="flex gap-2"><select id="action-diagnostic" class="av-select flex-1" bind:value={catalog} disabled={!enabled || busy}><option value="host_resources">Computer performance</option><option value="cisco_vpn_status">Cisco VPN status</option></select><button class="av-btn av-btn-secondary" disabled={!enabled || busy || !panel || hasDiagnostic} onclick={allowDiagnostic}>Allow read</button></div>
  <p class="av-hint">Accepted requests: “Check computer performance” or “Check VPN status”. These observe bounded system counters or Cisco status; they do not run scripts or change settings.</p>
  {#each activePermissions as item}<div class="row"><div><h3>{item.permission.name}</h3><p class="av-hint">{item.permission.target.kind === "browser_read" ? `Partial excerpt from ${item.permission.target.scope.origin}` : item.permission.target.kind === "prompt" ? `Prepare unsubmitted drafts in ${item.permission.target.binding.app_name}` : item.permission.target.kind === "diagnostic" ? "Fixed read-only diagnostic; no configuration changes" : item.permission.target.kind === "application" ? "Open this exact saved application" : outputs.find(o => o.id === (item.permission.target.kind === "volume" ? item.permission.target.endpoint : ""))?.name ?? "Saved speakers endpoint"}</p></div><button class="av-btn av-btn-ghost av-btn-sm" disabled={!enabled || busy} onclick={() => revoke(item.permission.id)}>Revoke</button></div>{/each}
  <div class="line"></div><h2>Recent accepted actions</h2>
  {#if !snapshot}<p class="av-hint">Refresh to inspect durable task state.</p>{:else if snapshot.tasks.length === 0}<p class="av-hint">No accepted action tasks for the current owner.</p>{:else}{#each snapshot.tasks as task}<div class="row"><div><h3>{task.payload.kind === "read_page" ? `Read partial page at ${targetName(task)}` : task.payload.kind === "fill_prompt" ? `Prepare unsubmitted draft in ${targetName(task)}` : task.payload.kind === "diagnostic" ? targetName(task) : task.payload.kind === "set_volume" ? `Set ${targetName(task)} to ${task.payload.percent}%` : `Open ${targetName(task)}`}</h3><p class="av-hint">{label(task)} · {new Date(task.created_ms).toLocaleString()}</p>
      {#if task.diagnostic}
        {#if task.diagnostic.kind === "cisco_vpn_status"}
          <p class="av-hint">Cisco: {measured(task.diagnostic.state, state => state.replaceAll("_", " "))}. This does not establish Spark reachability.</p>
        {:else}
          <p class="av-hint">CPU busy (primary processor group): {measured(task.diagnostic.cpu_busy_basis_points, value => `${(value / 100).toFixed(1)}%`)} · sampled over {(task.diagnostic.interval_ms / 1000).toFixed(2)} s</p>
          <p class="av-hint">{measured(task.diagnostic.system_drive_space, disk => `${disk.drive}: system drive · ${(disk.caller_available_bytes / 1_000_000_000).toFixed(2)} GB available of ${(disk.caller_total_bytes / 1_000_000_000).toFixed(2)} GB under your quota`)}</p>
          <p class="av-hint">Disk utilization: {measured(task.diagnostic.disk_pressure, value => `${(value / 100).toFixed(1)}%`)}</p>
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
