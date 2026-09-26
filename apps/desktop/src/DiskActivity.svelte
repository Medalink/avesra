<script lang="ts">
  type Reading<T>={state:"available";value:T}|{state:"unavailable";reason:string};
  type Activity={drive:string;scope:"system_volume"|"selected_destination";interval_ms:number;non_idle_basis_points:Reading<number>;read_bytes_per_second:Reading<number>;write_bytes_per_second:Reading<number>};
  let {value}:{value?:Reading<Activity>}=$props();
  const measured=(reading:Reading<number>,format:(value:number)=>string)=>reading.state==="available"?format(reading.value):`unavailable (${reading.reason.replaceAll("_"," ")})`;
  const rate=(value:number)=>`${(value/1e6).toFixed(2)} MB/s`;
</script>
{#if value?.state==="available"}
  <p class="av-hint">{value.value.scope==="selected_destination"?"Selected destination":"Windows system volume"} {value.value.drive}: · {(value.value.interval_ms/1000).toFixed(2)} s collection interval · non-idle {measured(value.value.non_idle_basis_points,v=>`${(v/100).toFixed(2)}%`)} · disk reads {measured(value.value.read_bytes_per_second,rate)} · disk writes {measured(value.value.write_bytes_per_second,rate)}.</p>
  <p class="av-hint">Logical-volume activity includes other applications and uses decimal MB/s. It does not establish download throughput, physical-device saturation or a bottleneck. Queue length and transfer latency are unmeasured.</p>
{:else}
  <p class="av-hint">Logical-volume activity unavailable ({value?.state==="unavailable"?value.reason.replaceAll("_"," "):"missing historical observation"}). No zero or utilization estimate is substituted.</p>
{/if}
