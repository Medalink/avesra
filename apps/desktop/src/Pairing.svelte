<script lang="ts">
  import { command, native } from "./runtime";
  let { connected = false }: { connected?: boolean } = $props();
  let expanded = $state(false);
  let url = $state("https://spark2:9474");
  let certificate = $state("");
  let fingerprint = $state("");
  let code = $state("");
  let verified = $state(false);
  let busy = $state(false);
  let message = $state("");
  async function reconnect() {
    busy = true;
    message = "";
    try {
      await command("connect_spark");
      message = "Connecting securely…";
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
      message = "Paired. Connecting securely…";
    } catch (e) {
      message = String(e);
    } finally {
      busy = false;
    }
  }
  async function disconnect() {
    try {
      await command("disconnect_spark");
      message = "Disconnected. Your protected pairing is retained.";
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
        {connected
          ? "Authenticated encrypted control connection."
          : "Pair using the certificate from your Spark."}
      </p>
    </div>
    <span
      class="av-chip ring-white/15"
      class:text-amber-200={!connected}
      class:text-zinc-300={connected}
      >{connected ? "Connected" : "Disconnected"}</span
    >
  </div>
  <div class="mt-3 flex gap-2">
    {#if connected}<button class="av-btn av-btn-secondary" onclick={disconnect}
        >Disconnect</button
      >{:else}<button
        class="av-btn av-btn-primary"
        disabled={!native || busy}
        onclick={() => (expanded = !expanded)}>Pair Spark</button
      ><button
        class="av-btn av-btn-secondary"
        disabled={!native || busy}
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
        disabled={!verified || busy}
        >{busy ? "Pairing…" : "Pair securely"}</button
      >
      <p class="av-hint">
        Pairing does not start listening or grant voice permissions. The device
        credential is protected for your Windows user.
      </p>
    </form>{/if}
  {#if message}<p class="av-hint mt-3" role="status">{message}</p>{/if}
</div>
