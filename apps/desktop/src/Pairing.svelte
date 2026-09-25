<script lang="ts">
  import { onMount } from "svelte";
  import { command, native, sparkConnection, type Runtime } from "./runtime";
  let { runtime }: { runtime: Runtime | null } = $props();
  const spark = $derived(sparkConnection(runtime));
  const connected = $derived(spark.connected);
  let expanded = $state(false);
  let url = $state("https://192.168.50.11:9474");
  let certificate = $state("");
  let fingerprint = $state("");
  let code = $state("");
  let verified = $state(false);
  let busy = $state(false);
  let message = $state("");
  type Spark = {
    id: string;
    name: string;
    address: string;
    pairing_open: boolean;
  };
  let sparks = $state<Spark[]>([]);
  let scanning = $state(false);
  let scanned = $state(false);
  let scanError = $state("");
  let mounted = false;
  let generation = 0;
  async function scan() {
    if (!native || scanning || busy || spark.connecting) return;
    const current = ++generation;
    scanning = true;
    scanned = false;
    scanError = "";
    sparks = [];
    try {
      const found = await command<Spark[]>("discover_sparks", { hint: url });
      if (mounted && current === generation) {
        sparks = found;
        scanned = true;
      }
    } catch (e) {
      if (mounted && current === generation) scanError = String(e);
    } finally {
      if (mounted && current === generation) scanning = false;
    }
  }
  async function useSpark(spark: Spark) {
    if (busy || scanning) return;
    busy = true;
    message = "";
    try {
      await command("pair_discovered_spark", { id: spark.id });
      sparks = [];
    } catch (e) {
      message = String(e);
      sparks = [];
      scanned = false;
    } finally {
      busy = false;
    }
  }
  onMount(() => {
    mounted = true;
    if (!connected && !spark.connecting) void scan();
    return () => {
      mounted = false;
      generation++;
      if (native) void command("cancel_spark_discovery").catch(() => {});
    };
  });
  let removal = $state<{
    file_revision: string;
    device_id: string | null;
    readable: boolean;
  } | null>(null);
  let understood = $state(false);
  async function reviewRemoval() {
    try {
      removal = await command("saved_pairing");
      understood = false;
      if (!removal) message = "No saved pairing.";
    } catch (e) {
      message = String(e);
    }
  }
  async function forget() {
    if (!removal || !understood) return;
    busy = true;
    try {
      await command("forget_spark", {
        fileRevision: removal.file_revision,
        understandRevocation: understood,
      });
      removal = null;
      message =
        "Local pairing removed. Server revocation remains a separate operation.";
    } catch (e) {
      message = String(e);
    } finally {
      busy = false;
    }
  }
  async function reconnect() {
    busy = true;
    message = "";
    try {
      await command("connect_spark");
    } catch (e) {
      message = String(e);
    } finally {
      busy = false;
    }
  }
  async function pair() {
    if (!verified) return;
    busy = true;
    message = "";
    try {
      await command("pair_spark", {
        input: { url, certificate, fingerprint, code },
      });
      code = "";
      expanded = false;
    } catch (e) {
      message = String(e);
    } finally {
      busy = false;
    }
  }
  async function disconnect() {
    try {
      await command("disconnect_spark");
      message = "Your protected pairing is retained for the next app launch.";
    } catch (e) {
      message = String(e);
    }
  }
</script>

<div class="av-card p-4">
  <div class="row">
    <div>
      <h2>Single Spark</h2>
      <p class="av-hint mt-1">
Your saved Spark reconnects automatically when Avesra starts.
      </p>
    </div>
  </div>
  {#if !connected && !spark.connecting}
    <div class="mt-4 border-t border-white/10 pt-4">
      <div class="flex items-center gap-3">
        <div class="flex-1">
          <h2>Find your Spark</h2>
          <p class="av-hint mt-1">
            Your PC and Spark should be on the same network.
          </p>
        </div>
        <button
          class="av-btn av-btn-primary"
          disabled={!native || scanning || busy}
          onclick={scan}
          >{scanning
            ? "Searching…"
            : scanned
              ? "Scan again"
              : "Find my Spark"}</button
        >
      </div>
      {#if scanning}<p class="av-hint mt-4" role="status">
          Looking for nearby Avesra servers…
        </p>
      {:else if scanError}<p class="warning mt-4" role="alert">{scanError}</p>
      {:else if scanned && sparks.length === 0}<div
          class="mt-4 bg-white/[0.025] p-3 ring-1 ring-white/10 ring-inset"
        >
          <h2>No Spark found yet</h2>
          <p class="av-hint mt-1">
            Make sure Avesra is running on your Spark, then scan again. Older
            servers need the discovery update. You can also connect manually
            below.
          </p>
        </div>{/if}
      {#each sparks as spark}
        <div
          class="mt-3 flex items-center gap-3 bg-white/[0.03] p-3 ring-1 ring-white/10 ring-inset"
        >
          <div class="min-w-0 flex-1">
            <h2 class="truncate">{spark.name}</h2>
            <p class="caption mt-1 text-zinc-400">{spark.address}</p>
            <p class="av-hint mt-1">
              {spark.pairing_open
                ? "Available for this PC"
                : "Found · pairing is closed"}
            </p>
          </div>
          <button
            class="av-btn av-btn-primary"
            disabled={!spark.pairing_open || busy || scanning}
            onclick={() => useSpark(spark)}
            >{busy ? "Connecting…" : "Use this Spark"}</button
          >
        </div>
        {#if !spark.pairing_open}<p class="av-hint mt-2">
            Open pairing on this Spark, then scan again. Advanced pairing is
            also available.
          </p>{/if}
      {/each}
      {#if sparks.length}<p class="av-hint mt-3">
          Choose a device you recognize. Avesra remembers its certificate and
          keeps the connection encrypted.
        </p>{/if}
    </div>
  {/if}
  <button
    class="av-btn av-btn-ghost av-btn-sm mt-2"
    disabled={!native || busy || spark.connecting}
    onclick={reviewRemoval}>Manage saved pairing</button
  >
  {#if removal}
    <div class="mt-3 border-t border-white/10 pt-3 flex flex-col gap-3">
      <p class="av-hint">
        Removing this PC's saved credential disconnects it. It does not revoke
        the credential on Spark. Revoke the device through your authenticated
        Spark administration session.
      </p>
      <p class="caption break-all">
        Device: {removal.device_id ??
          "Unavailable — saved credential cannot be decrypted"}
      </p>
      <label class="flex items-start gap-2 text-[12px] text-zinc-300"
        ><input
          type="checkbox"
          bind:checked={understood}
          class="mt-0.5 accent-[#960b3f]"
        />I understand that server revocation must be completed separately.</label
      >
      <div class="flex gap-2">
        <button
          class="av-btn av-btn-secondary"
          disabled={!understood || busy}
          onclick={forget}>Remove local pairing</button
        ><button class="av-btn av-btn-ghost" onclick={() => (removal = null)}
          >Cancel</button
        >
      </div>
    </div>
  {/if}
  <div class="mt-3 flex gap-2">
    {#if connected || spark.connecting}<button class="av-btn av-btn-secondary" onclick={disconnect}
        >{spark.connecting ? "Cancel connection" : "Disconnect"}</button
      >{:else}<button
        class="av-btn av-btn-ghost"
        disabled={!native || busy || spark.connecting}
        aria-expanded={expanded}
        onclick={() => (expanded = !expanded)}
        >{expanded ? "Hide advanced" : "Advanced · manual pairing"}</button
      ><button
        class="av-btn av-btn-secondary"
        disabled={!native || busy || spark.connecting}
        onclick={reconnect}>Reconnect</button
      >{/if}
  </div>
  {#if expanded}<form
      class="mt-4 flex flex-col gap-3 border-t border-white/10 pt-3"
      onsubmit={(e) => {
        e.preventDefault();
        void pair();
      }}
    >
      <p class="av-hint">
        Read the public certificate and fingerprint through your authenticated
        Spark administration session. Verify the fingerprint here before
        entering the one-time code.
      </p>
      <label class="flex flex-col gap-1"
        ><span class="av-label">Spark address</span><input
          class="av-input"
          bind:value={url}
          autocomplete="off"
          spellcheck="false"
          required
        /></label
      >
      <label class="flex flex-col gap-1"
        ><span class="av-label">Server certificate · PEM</span><textarea
          class="av-input av-textarea h-24 font-mono text-[11px]"
          bind:value={certificate}
          oninput={() => (verified = false)}
          maxlength="16384"
          spellcheck="false"
          required></textarea></label
      >
      <label class="flex flex-col gap-1"
        ><span class="av-label">SHA-256 fingerprint shown on Spark</span><input
          class="av-input font-mono text-[11px]"
          bind:value={fingerprint}
          oninput={() => (verified = false)}
          autocomplete="off"
          spellcheck="false"
          maxlength="95"
          required
        /></label
      >
      <label class="flex items-start gap-2 text-[12px] text-zinc-300"
        ><input
          type="checkbox"
          bind:checked={verified}
          class="mt-0.5 accent-[#960b3f]"
        />I verified this fingerprint through my authenticated Spark session.</label
      >
      <label class="flex flex-col gap-1"
        ><span class="av-label">One-time pairing code</span><input
          type="password"
          class="av-input font-mono"
          bind:value={code}
          autocomplete="off"
          maxlength="64"
          required
        /></label
      >
      <button
        type="submit"
        class="av-btn av-btn-primary self-start"
        disabled={!verified || busy || spark.connecting}
        >{busy ? "Pairing…" : "Pair securely"}</button
      >
      <p class="av-hint">
        Pairing does not start listening or grant voice permissions. The device
        credential is protected for your Windows user.
      </p>
    </form>{/if}
  {#if message}<p class="av-hint mt-3" role="status">{message}</p>{/if}
</div>
