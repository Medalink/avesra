<script lang="ts">
  type Reading={state:"value";value:number}|{state:"unavailable";reason:string};
  type Value={metric:string;gpu:number|null;reading:Reading};
  type Sample={host:string;process:string;boot:string|null;process_started:number|null;sequence:number;at_ms:number;monotonic_ms:number;window_ms:number|null;collection_ms:number;values:Value[]};
  type Resources={samples:Sample[];rollups:unknown[];evicted:number;truncated:boolean;latest_age_ms:number|null};
  let {value,label}:{value:Resources;label:string}=$props();
  const latest=$derived(value.samples.at(-1));
  const summaries=$derived.by(()=>{
    if(!latest)return [];
    const samples=value.samples.filter(sample=>sample.process===latest.process);
    const groups=new Map<string,{metric:string;gpu:number|null;values:number[]}>();
    for(const sample of samples)for(const row of sample.values){
      const key=`${row.metric}/${row.gpu??"host"}`;
      const group=groups.get(key)??{metric:row.metric,gpu:row.gpu,values:[]};
      if(row.reading.state==="value")group.values.push(row.reading.value);
      groups.set(key,group);
    }
    return [...groups.values()].map(group=>{
      group.values.sort((a,b)=>a-b);
      const percentile=(p:number)=>group.values.length?group.values[Math.ceil(group.values.length*p)-1]:null;
      const last=latest.values.find(row=>row.metric===group.metric&&row.gpu===group.gpu)?.reading??null;
      return {...group,last,missing:samples.length-group.values.length,count:group.values.length,p50:percentile(.5),p95:percentile(.95),max:group.values.at(-1)??null};
    });
  });
  function format(metric:string,value:number|null):string{
    if(value===null)return "unavailable";
    if(metric.endsWith("_bytes"))return `${(value/1048576).toFixed(1)} MiB`;
    if(metric.endsWith("_cpu")||metric==="gpu_busy")return `${(value/100).toFixed(2)}%`;
    if(metric==="gpu_temperature_milli_c")return `${(value/1000).toFixed(1)} °C`;
    if(metric==="gpu_power_milli_w")return `${(value/1000).toFixed(1)} W`;
    if(metric==="host_uptime_ms")return `${(value/3600000).toFixed(2)} h`;
    return String(value);
  }
</script>
<div class="av-card p-4">
  <span class="av-kicker">{label} · cached host resources</span>
  {#if latest}
    <p class="av-hint">Sample age at refresh: {value.latest_age_ms===null?"unavailable (prior process or sampler gap)":`${(value.latest_age_ms/1000).toFixed(1)} s`}{value.latest_age_ms===null||value.latest_age_ms>15000?" · stale/unavailable":""}. Collection {latest.collection_ms} ms · CPU window {latest.window_ms??"unavailable"} ms.</p>
    <p class="av-hint break-all">Process {latest.process} · process-start marker {latest.process_started??"unavailable"} · boot {latest.boot??"unavailable"}. Host usage is not attributed to the selected accepted turn.</p>
    {#each summaries as row}
      <p class="av-hint">{row.metric.replaceAll("_"," ")}{row.gpu===null?"":` · GPU ${row.gpu}`}: {row.last===null?"unavailable (absent from latest observation)":row.last.state==="value"?format(row.metric,row.last.value):`unavailable (${row.last.reason})`}. {row.count} numeric / {row.missing} absent, unavailable or reset samples. Historical numeric-only p50 {format(row.metric,row.p50)} · p95 {format(row.metric,row.p95)} · max {format(row.metric,row.max)}.</p>
    {/each}
  {:else}<p class="av-hint">No resource observations available. This is not zero usage.</p>{/if}
  <p class="av-hint">Five-second sampling; {value.samples.length} detailed records in this view · {value.evicted} expired/capacity-evicted · {value.rollups.length} daily metric rollups. {value.truncated?"Detailed view/export is truncated to the newest 720 samples. ":""}Numeric summaries use only the latest observer process. Detailed storage is bounded to 8,192 samples; rollups retain 30 days.</p>
  <p class="av-hint">Process CPU uses total logical-CPU capacity. Windows process working set excludes WebView children and other workers; it cannot prove the whole-companion memory budget. Host available RAM is headroom, not reserved model memory. GPU readings are device-wide where supported. No gaming frame-time, capture-cost or per-model allocation measurement is claimed.</p>
</div>
