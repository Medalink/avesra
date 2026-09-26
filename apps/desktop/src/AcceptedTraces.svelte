<script lang="ts">
  import { command, native, type Runtime } from "./runtime";
  let { runtime }: { runtime: Runtime | null } = $props();
  type Span = { id:string; link:{turn:string;operation:string;parent:string|null}; host:string; process:string; stage:string; outcome:string; error:string|null; duration_us:number; queue_us:number|null; retries:number; deployment:{model:string|null;image:string|null;config:string|null}; analysis?:{request:string;receipts:{worker:string;host:string;process:string;lane:string;model_revision:string;duration_us:number}[]}|null };
  type Snapshot = { records:Span[]; trace_days:number; observer_loss:number; evicted:number; collector_starts:number; truncated:boolean };
  type View = { local:Snapshot; controller:Snapshot|null; controller_error:string|null };
  let view = $state<View|null>(null);
  let selected = $state("");
  let busy = $state(false);
  let message = $state("");
  let days = $state(7);
  let generation=0;
  $effect(()=>{if(runtime?.locked){generation++;view=null;selected="";message="";}});
  const spans=$derived([...(view?.local.records??[]),...(view?.controller?.records??[])]);
  const turns=$derived([...new Set(spans.map(s=>s.link.turn))]);
  const rows=$derived(selected?spans.filter(s=>s.link.turn===selected):[]);
  const summaries=$derived.by(()=>{
    const groups=new Map<string,Span[]>();
    for(const span of selected?rows:spans){const key=`${span.host} / ${span.stage}`;const group=groups.get(key)??[];group.push(span);groups.set(key,group);}
    return [...groups].map(([name,values])=>{
      const response=values[0].stage==="endpoint_response_submission";
      const measured=response?values.filter(v=>v.outcome==="complete"):values;
      const durations=measured.map(v=>v.duration_us/1000).sort((a,b)=>a-b);
      const percentile=(p:number)=>durations.length?durations[Math.ceil(durations.length*p)-1].toFixed(1):"unavailable";
      return {name,response,count:values.length,measured:measured.length,complete:values.filter(v=>v.outcome==="complete").length,missing:values.filter(v=>v.outcome==="missing").length,errors:values.filter(v=>v.outcome==="failed"||v.outcome==="truncated").length,withdrawn:values.filter(v=>v.outcome==="withdrawn").length,abandoned:values.filter(v=>v.outcome==="abandoned").length,p50:percentile(.5),p95:percentile(.95),p99:percentile(.99),max:durations.length?durations[durations.length-1].toFixed(1):"unavailable"};
    });
  });
  async function inspect(retention=false){
    if(busy||!native||!runtime||runtime.locked)return;
    busy=true;message="";const current=++generation;
    try{const result=await command<View>(retention?"set_trace_retention":"accepted_traces",retention?{days:Number(days)}:{turn:null});if(current===generation){view=result;days=result.local.trace_days;if(!turns.includes(selected))selected="";}}
    catch(e){if(current===generation)message=String(e);}finally{busy=false;}
  }
  async function exportTrace(){
    if(busy||!native||!runtime||runtime.locked)return;
    busy=true;message="";const current=++generation;
    try{const path=await command<string>("export_accepted_traces",{turn:selected||null});if(current===generation)message=`Saved redacted trace export: ${path}`;}
    catch(e){if(current===generation)message=String(e);}finally{busy=false;}
  }
</script>
<section class="section">
  <div class="av-kicker">Accepted request traces</div>
  <p class="av-hint">Follow genuine accepted turns across this PC and the paired controller. Refresh starts no capture, model work or playback. IDs and typed outcomes contain no conversation text.</p>
  <div class="flex flex-wrap gap-2">
    <button class="av-btn av-btn-ghost av-btn-sm" disabled={busy||!native||!runtime||runtime.locked} onclick={()=>inspect()}>Refresh traces</button>
    <button class="av-btn av-btn-ghost av-btn-sm" disabled={busy||!view||runtime?.locked} onclick={exportTrace}>Export {selected?"selected turn":"retained traces"}</button>
    <label class="av-hint">Local trace retention <select bind:value={days} disabled={busy}>{#each [1,2,3,4,5,6,7] as day}<option value={day}>{day} day{day===1?"":"s"}</option>{/each}</select></label>
    <button class="av-btn av-btn-ghost av-btn-sm" disabled={busy||!view||runtime?.locked} onclick={()=>inspect(true)}>Apply retention</button>
  </div>
  {#if view}
    <p class="av-hint">PC: {view.local.records.length} spans · {view.local.observer_loss} observer losses · {view.local.evicted} expired/evicted · {view.local.collector_starts} collector starts. Detailed records use a 32,768-span hard cap; rollups retain 30 days. Collector/process gaps may contain missing spans.</p>
    {#if view.controller}<p class="av-hint">Controller: {view.controller.records.length} spans · {view.controller.observer_loss} observer losses · {view.controller.evicted} expired/evicted · {view.controller.trace_days}-day retention.</p>{:else}<p class="av-hint text-amber-200">Controller unavailable: {view.controller_error}. This is not an empty successful trace.</p>{/if}
    {#if view.local.truncated||view.controller?.truncated}<p class="av-hint">Inspection/export is limited to the newest 2,048 spans per host. Select a turn to export its exact retained records.</p>{/if}
    <label class="av-hint">Accepted turn <select bind:value={selected}><option value="">Select a turn ({turns.length} retained)</option>{#each turns as turn}<option value={turn}>{turn}</option>{/each}</select></label>
    {#each summaries as summary}
      <p class="av-hint">{summary.name}: {summary.count} observations · {summary.complete} completed · {summary.errors} failed · {summary.missing} missing · {summary.withdrawn} withdrawn · {summary.abandoned} abandoned. {summary.response?"Successful submission latency only":"All-outcome elapsed durations"}: p50 {summary.p50} / p95 {summary.p95} / p99 {summary.p99} / max {summary.max} ms ({summary.measured} measured){summary.measured<30?" · provisional":""}.</p>
    {/each}
    {#each rows as row}
      <div class="av-card p-3"><strong>{row.stage}</strong> · {row.host} · {row.outcome}{row.error?` (${row.error})`:""}
        <p class="av-hint">{(row.duration_us/1000).toFixed(2)} ms host-local {row.stage==="endpoint_response_submission"&&row.outcome!=="complete"?"elapsed until failure/missing/abandonment (not submission latency)":"duration"} · queue {row.queue_us===null?"unavailable":`${(row.queue_us/1000).toFixed(2)} ms`} · {row.retries} retries</p>
        <p class="av-hint break-all">Operation {row.link.operation} · parent {row.link.parent??"none"} · process {row.process}</p>
        <p class="av-hint break-all">Model {row.deployment.model??"unavailable"} · image {row.deployment.image??"unavailable"} · config {row.deployment.config??"unavailable"}</p>
        {#if row.analysis}{#each row.analysis.receipts as receipt}<p class="av-hint break-all">{receipt.host} {receipt.lane} driver round trip: {(receipt.duration_us/1000).toFixed(2)} ms · worker {receipt.worker} · process {receipt.process} · model {receipt.model_revision}. Includes IPC and validation; not GPU kernel time.</p>{/each}{:else if row.stage==="voice_analysis"}<p class="av-hint">Controller stage receipts unavailable.</p>{/if}
      </div>
    {/each}
  {/if}
  {#if message}<p class="av-hint break-all" role="status">{message}</p>{/if}
  <p class="av-hint">Different hosts have unsynchronized clocks; durations are never subtracted across hosts. Endpoint response submission starts after the required quiet tail and minimum capture span, not at the user's last speech. It ends at the start of the first reference block containing nonzero accepted speech, with up to 20 ms block granularity. It excludes the quiet tail from user-speech-end latency and does not prove audibility or human-rated usefulness; A06 latency targets remain unmeasured here. Failed/missing/abandoned observations remain in totals and drilldown. Successful controller ASR/speaker receipts are separate from native failure totals. Hardware headroom and complete release benchmarks remain separate evidence.</p>
</section>
