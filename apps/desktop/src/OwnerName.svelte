<script lang="ts">
  import { command, native, type Runtime } from "./runtime";

  let { runtime, ownerReady, blocked }: { runtime: Runtime | null; ownerReady: boolean; blocked: boolean } = $props();
  let name = $state("");
  let busy = $state(false);
  let error = $state("");
  let notice = $state("");
  const savedName = $derived(runtime?.settings.owner_name?.name ?? "");
  const available = $derived(native && ownerReady && !blocked && runtime?.connected && !runtime.locked);
  $effect(() => {
    name = savedName;
  });
  async function save() {
    busy = true;
    error = "";
    notice = "";
    try {
      await command("remember_owner_name", { name });
      notice = name.trim()
        ? "Name remembered for your next startup greeting."
        : "Name forgotten. Avesra will say Hello.";
    } catch (reason) {
      error = String(reason);
    } finally {
      busy = false;
    }
  }
</script>

<section class="section">
  <label class="av-kicker" for="remembered-owner-name">Your name</label>
  <p class="av-hint">
    Avesra says hello when it starts with a selected voice. Remember your name
    for a personal greeting, or leave it blank for Hello.
  </p>
  <form
    class="mt-3 flex items-center gap-3"
    onsubmit={(event) => {
      event.preventDefault();
      void save();
    }}
  >
    <input
      id="remembered-owner-name"
      class="av-input min-w-0 flex-1"
      autocomplete="given-name"
      maxlength="80"
      bind:value={name}
      disabled={!available || busy}
      placeholder="Your name"
    />
    <button
      class="av-btn av-btn-primary"
      type="submit"
      disabled={!available || busy}>{busy ? "Saving…" : "Remember"}</button
    >
  </form>
  {#if !ownerReady}<p class="av-hint mt-2">Check your owner account in owner setup before remembering your name.</p>{/if}
  {#if error}<p class="av-hint mt-2 text-red-300" role="alert">{error}</p>{/if}
  {#if notice}<p class="av-hint mt-2" role="status">{notice}</p>{/if}
</section>
