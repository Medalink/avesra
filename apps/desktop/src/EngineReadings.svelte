<script lang="ts">
  type Admission = {lane:string;operation:string;attempts:number;busy:number;closed:number};
  type Times = {count:number;total_us:number;maximum_us:number};
  type Stats = {enqueued:number;dequeued:number;dropped:number;maximum_outstanding:number;dwell:Times;discarded_dwell:Times;capacity_wait:Times;failed_wait:Times;abandoned_wait:Times};
  type Link = {turn:string;operation:string;parent:string|null};
  type Queue = {id:string;process:string;link:Link;at_ms:number;elapsed_us:number;outcome:string;stats:Stats};
  type Live = {id:string;process:string;link:Link;capacity:number;outstanding:number;oldest_outstanding_age_us:number|null;stats:Stats};
  type Rollup = {day:number;outcome:string;queues:number;enqueued:number;dequeued:number;dropped:number;dwell_us:number;maximum_dwell_us:number;capacity_wait_us:number;discarded_dwell_us:number;failed_wait_us:number;abandoned_wait_us:number};
  type Value = {supported:boolean;process:string;admissions:Admission[];daily_admissions:(Admission&{day:number;process:string})[];live:Live[];queues:Queue[];rollups:Rollup[];evicted:number;admission_evicted:number;rollup_evicted:number;truncated:boolean};
  let {value,selected=""}:{value:Value;selected?:string}=$props();
  const queues=$derived(value.queues.filter(v=>!selected||v.link.turn===selected));
  const live=$derived(value.live.filter(v=>!selected||v.link.turn===selected));
  const ms=(v:number)=>(v/1000).toFixed(2);
</script>
{#if value.supported}
  <section class="av-card p-3 mt-3">
    <strong>Controller admission and speech buffer</strong>
    <p class="av-hint">Observed at the last refresh. Admission attempts do not wait in a queue: busy means no permit at that instant. These are controller operation counts, not inference counts, GPU utilization or model compute. Process {value.process}.</p>
    {#if value.admissions.length===0}<p class="av-hint">No instrumented admission attempts observed in this controller process.</p>{/if}
    {#each value.admissions as row}
      <p class="av-hint">{row.lane} / {row.operation}: {row.attempts} attempts · {row.busy} busy refusals · {row.closed} closed refusals · {row.attempts-row.busy-row.closed} permits acquired.</p>
    {/each}
    <p class="av-hint">TTS has a real 64-piece controller output channel. Ticket dwell ends at the consumer receive handoff; discarded dwell is separate. Up to 65 outstanding tickets cover 64 channel slots plus one receive handoff, so this is not exact instantaneous channel occupancy. Capacity wait is output backpressure, not model or GPU time. No PCM or response text is retained.</p>
    {#if live.length===0}<p class="av-hint">No matching live queue observer at refresh. This does not prove that private model work has retired.</p>{/if}
    {#each live as row}
      <p class="av-hint break-all">Live output {row.link.operation}: {row.capacity} channel slots · {row.outstanding} outstanding tickets · oldest outstanding age {row.oldest_outstanding_age_us===null?"empty":`${ms(row.oldest_outstanding_age_us)} ms`} · peak outstanding {row.stats.maximum_outstanding} · {row.stats.enqueued} enqueued / {row.stats.dequeued} received / {row.stats.dropped} dropped.</p>
    {/each}
    <details>
      <summary>Retired queue details ({queues.length})</summary>
      {#each queues as row}
        <div class="av-card p-2 mt-2">
          <p class="av-hint break-all">{row.outcome} · turn {row.link.turn} · output {row.link.operation} · process {row.process} · observer lifetime {ms(row.elapsed_us)} ms.</p>
          <p class="av-hint">{row.stats.enqueued} enqueued / {row.stats.dequeued} received / {row.stats.dropped} dropped · peak outstanding {row.stats.maximum_outstanding}/65.</p>
          <p class="av-hint">Received dwell: {row.stats.dwell.count} observations, sum {ms(row.stats.dwell.total_us)} ms, max {ms(row.stats.dwell.maximum_us)} ms. Discarded dwell: {row.stats.discarded_dwell.count} observations, sum {ms(row.stats.discarded_dwell.total_us)} ms.</p>
          <p class="av-hint">Capacity reservations: {row.stats.capacity_wait.count} complete / {row.stats.failed_wait.count} failed / {row.stats.abandoned_wait.count} abandoned. Elapsed sums {ms(row.stats.capacity_wait.total_us)} / {ms(row.stats.failed_wait.total_us)} / {ms(row.stats.abandoned_wait.total_us)} ms respectively.</p>
        </div>
      {/each}
    </details>
    <details>
      <summary>Persisted daily totals (all owner turns; controller admissions are process scoped)</summary>
      {#each value.daily_admissions as row}
        <p class="av-hint break-all">UTC day {row.day} · process {row.process} · {row.lane}/{row.operation}: {row.attempts} attempts, {row.busy} busy, {row.closed} closed.</p>
      {/each}
      {#each value.rollups as row}
        <p class="av-hint">UTC day {row.day} · {row.outcome}: {row.queues} retired queues · {row.enqueued} enqueued / {row.dequeued} received / {row.dropped} dropped · received dwell sum/max {ms(row.dwell_us)}/{ms(row.maximum_dwell_us)} ms · capacity wait sum {ms(row.capacity_wait_us)} ms · discarded dwell sum {ms(row.discarded_dwell_us)} ms · failed/abandoned wait sums {ms(row.failed_wait_us)}/{ms(row.abandoned_wait_us)} ms.</p>
      {/each}
    </details>
    <p class="av-hint">{value.evicted} detailed queue records, {value.admission_evicted} daily admission rows and {value.rollup_evicted} daily queue rollups expired/evicted. Detail cap 8,192 with configured trace retention; daily rows retain at most 30 days and 8,192 rows per table. Export includes these fixed fields. Pending admission increments since the last five-second checkpoint can be lost on process exit; observer/storage losses appear in controller totals above.{value.truncated?" Only the newest 256 matching retired queues are included.":""}</p>
  </section>
{/if}
