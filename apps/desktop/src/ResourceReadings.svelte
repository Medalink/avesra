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
    <p class="av-hint" class:text-amber-200={value.latest_age_ms===null||value.latest_age_ms>15000}>Sample age at refresh: {value.latest_age_ms===null?"unavailable (prior process or sampler gap)":`${(value.latest_age_ms/1000).toFixed(1)} s`}{value.latest_age_ms===null||value.latest_age_ms>15000?" · stale/unavailable":""}.</p>
    <p class="av-hint">Historical percentiles use numeric samples from the latest observer process only. Coverage shows numeric / absent, unavailable or reset samples.</p>
    <div class="overflow-x-auto">
      <table class="w-full text-left text-sm">
        <thead><tr class="border-b border-white/10">
          <th class="py-2 pr-4 font-medium">Metric</th>
          <th class="py-2 pr-4 font-medium">Current observation</th>
          <th class="py-2 pr-4 font-medium">Historical p50</th>
          <th class="py-2 pr-4 font-medium">Historical p95</th>
          <th class="py-2 pr-4 font-medium">Historical max</th>
          <th class="py-2 font-medium">Coverage</th>
        </tr></thead>
        <tbody>
          {#each summaries as row}
            <tr class="border-b border-white/5 align-top">
              <th scope="row" class="py-2 pr-4 font-normal">{row.metric.replaceAll("_"," ")}{row.gpu===null?"":` · GPU ${row.gpu}`}</th>
              <td class="py-2 pr-4" class:text-amber-200={row.last===null||row.last.state!=="value"}>{row.last===null?"unavailable (absent from latest observation)":row.last.state==="value"?format(row.metric,row.last.value):`unavailable (${row.last.reason})`}</td>
              <td class="py-2 pr-4 whitespace-nowrap">{format(row.metric,row.p50)}</td>
              <td class="py-2 pr-4 whitespace-nowrap">{format(row.metric,row.p95)}</td>
              <td class="py-2 pr-4 whitespace-nowrap">{format(row.metric,row.max)}</td>
              <td class="py-2 whitespace-nowrap">{row.count} / {row.missing}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {:else}<p class="av-hint text-amber-200">No resource observations available. This is not zero usage.</p>{/if}
  <p class="av-hint">Five-second sampling · {value.samples.length} detailed records · {value.evicted} expired/capacity-evicted. {value.truncated?"View/export is truncated to the newest 720 samples.":""}</p>
  <details>
    <summary class="av-hint cursor-pointer">Process identity and measurement scope</summary>
    {#if latest}
      <p class="av-hint break-all">Process {latest.process} · process-start marker {latest.process_started??"unavailable"} · boot {latest.boot??"unavailable"}. Collection {latest.collection_ms} ms · CPU window {latest.window_ms??"unavailable"} ms. Host usage is not attributed to the selected accepted turn.</p>
    {/if}
    <p class="av-hint">{value.rollups.length} daily metric rollups. Detailed storage is bounded to 8,192 samples; rollups retain 30 days.</p>
    <p class="av-hint">Process CPU uses total logical-CPU capacity. Windows process working set excludes WebView children and other workers; it cannot prove the whole-companion memory budget. Host available RAM is headroom, not reserved model memory. GPU readings are device-wide where supported. No gaming frame-time, capture-cost or per-model allocation measurement is claimed.</p>
  </details>
</div>
