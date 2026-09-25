// Bundled fixed functions serialized into the exact document's ISOLATED world.
// No page-facing command or caller-selected code is accepted.
type Guard = {
  request: string; url: string; deadline: number; revision: number; settled: boolean;
  current(): boolean; finish(): boolean;
};
type Realm = typeof globalThis & { __avesraReadGuard1?: Guard };
export type ReadParameters = { request: string; url: string; maxBlocks: number; budgetMs: number };
export type Extracted = { started: true; state: "excerpt"; dom_revision: number; title: string; blocks: string[]; truncated: boolean; excluded_content: boolean }
  | { started: true; state: "empty"; dom_revision: number }
  | { started: true; state: "changed" | "unavailable" }
  | { started: false; state: "unavailable"; request: string; url: string; guard_absent: true }
  | { started: "unknown"; state: "unavailable" };

// Must remain self-contained: Chrome serializes this function, not imports.
export function beginPageExcerpt(input: ReadParameters): Extracted {
  const realm = globalThis as Realm;
  // A same-request tombstone is still owned work. It never proves absence.
  if (realm.__avesraReadGuard1) return { started: "unknown", state: "unavailable" };
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(input.request) ||
    input.request === "00000000-0000-0000-0000-000000000000" ||
    typeof input.url !== "string" || input.url.length > 2048 || location.href !== input.url ||
    location.protocol !== "https:" || self !== top || document.contentType !== "text/html" || document.readyState !== "complete" ||
    !Number.isSafeInteger(input.maxBlocks) || input.maxBlocks < 1 || input.maxBlocks > 16 ||
    !Number.isSafeInteger(input.budgetMs) || input.budgetMs < 1 || input.budgetMs > 10_000 ||
    !document.body) return { started: false, state: "unavailable", request: input.request, url: input.url, guard_absent: true };

  // Separate closure scope: no extracted text/arrays live in retained callbacks.
  function ownGuard(request: string, url: string, lifetime: number): Guard {
    let dirty = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const guard: Guard = {
      request, url, deadline: performance.now() + lifetime, revision: 1, settled: false,
      current() {
        if (performance.now() >= guard.deadline || location.href !== url || document.readyState !== "complete") retire(true);
        if (!guard.settled && observer.takeRecords().length) retire(true);
        return !guard.settled && !dirty;
      },
      finish() {
        const unchanged = guard.current();
        retire(!unchanged);
        return unchanged;
      },
    };
    const observer = new MutationObserver(() => retire(true));
    function changed() { retire(true); }
    function retire(changed: boolean) {
      if (changed) { dirty = true; guard.revision = 2; }
      if (guard.settled) return;
      guard.settled = true;
      observer.disconnect();
      observer.takeRecords();
      document.removeEventListener("input", changedEvent, true);
      document.removeEventListener("beforeinput", changedEvent, true);
      document.removeEventListener("pointerdown", changedEvent, true);
      document.removeEventListener("keydown", changedEvent, true);
      window.removeEventListener("pagehide", changedEvent, true);
      if (timer !== undefined) clearTimeout(timer);
    }
    const changedEvent = changed;
    observer.observe(document, { subtree: true, childList: true, characterData: true, attributes: true });
    document.addEventListener("input", changedEvent, true);
    document.addEventListener("beforeinput", changedEvent, true);
    document.addEventListener("pointerdown", changedEvent, true);
    document.addEventListener("keydown", changedEvent, true);
    window.addEventListener("pagehide", changedEvent, true);
    timer = setTimeout(changedEvent, lifetime);
    return guard;
  }

  const guard = ownGuard(input.request, input.url, input.budgetMs);
  realm.__avesraReadGuard1 = guard;
  const begun = performance.now();
  const cutoff = Math.min(guard.deadline, begun + 50);
  const limit = Symbol("bounded traversal exhausted");
  const encoder = new TextEncoder();
  let visited = 0, bytes = 0, truncated = false, excluded = false;
  const blocks: string[] = [];
  function check() { if (++visited > 2048 || performance.now() >= cutoff) throw limit; }
  function clipped(value: string, maximum: number) {
    let result = "", size = 0;
    // Caller already bounds this prefix before invoking the encoder/iteration.
    for (const character of value) {
      if (/\p{Cc}/u.test(character) && character !== "\n" && character !== "\t") continue;
      const next = encoder.encode(character).length;
      if (size + next > maximum) { truncated = true; break; }
      result += character; size += next;
    }
    return result.trim();
  }
  try {
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT, {
      acceptNode(node) {
        check();
        if (node.nodeType === Node.TEXT_NODE) return NodeFilter.FILTER_ACCEPT;
        if (!(node instanceof HTMLElement)) { excluded = true; return NodeFilter.FILTER_REJECT; }
        let depth = 0;
        for (let parent = node.parentElement; parent; parent = parent.parentElement) {
          if (++depth > 64) { excluded = true; return NodeFilter.FILTER_REJECT; }
          if (performance.now() >= cutoff) throw limit;
        }
        if (["SCRIPT", "STYLE", "NOSCRIPT", "TEMPLATE", "IFRAME", "FRAME", "OBJECT", "EMBED", "CANVAS", "VIDEO", "AUDIO", "METER", "PROGRESS", "FORM", "INPUT", "TEXTAREA", "SELECT", "BUTTON"].includes(node.tagName) ||
          node.isContentEditable || node.hidden || node.inert || node.getAttribute("aria-hidden") === "true" ||
          (node.tagName === "DETAILS" && !node.hasAttribute("open")) || node.getAttribute("aria-expanded") === "false" ||
          ["textbox", "combobox", "spinbutton"].includes(node.getAttribute("role") ?? "") || node.hasAttribute("autocomplete")) {
          excluded = true; return NodeFilter.FILTER_REJECT;
        }
        if (node.shadowRoot) excluded = true;
        const style = getComputedStyle(node);
        if (performance.now() >= cutoff) throw limit;
        if (style.display === "none" || style.visibility !== "visible" || style.opacity === "0" || style.contentVisibility === "hidden" || style.clipPath !== "none" || style.maskImage !== "none") {
          excluded = true; return NodeFilter.FILTER_REJECT;
        }
        return NodeFilter.FILTER_ACCEPT;
      },
    });
    // TreeWalker does not filter its root. Sensitive/hidden root is unsupported.
    if (document.body.isContentEditable || document.body.hidden || document.body.inert || document.body.getAttribute("aria-hidden") === "true") {
      guard.finish(); return { started: true, state: "unavailable" };
    }
    for (let root: HTMLElement | null = document.body; root; root = root.parentElement) {
      check();
      const style = getComputedStyle(root);
      if (performance.now() >= cutoff) throw limit;
      if (root.hidden || root.inert || root.isContentEditable || root.getAttribute("aria-hidden") === "true" ||
        ["textbox", "combobox", "spinbutton"].includes(root.getAttribute("role") ?? "") || root.hasAttribute("autocomplete") ||
        style.display === "none" || style.visibility !== "visible" || style.opacity === "0" || style.contentVisibility === "hidden" || style.clipPath !== "none" || style.maskImage !== "none") {
        guard.finish(); return { started: true, state: "unavailable" };
      }
    }
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      if (performance.now() >= cutoff) throw limit;
      if (!(node instanceof Text)) continue;
      if (node.length > 512) truncated = true;
      let prefix = node.substringData(0, 512);
      // Do not fabricate U+FFFD by clipping a valid surrogate pair in half.
      if (/[\uD800-\uDBFF]$/.test(prefix)) prefix = prefix.slice(0, -1);
      if (/[\uD800-\uDFFF]/u.test(prefix)) { excluded = true; continue; }
      const value = clipped(prefix, Math.min(512, 4096 - bytes));
      if (!value) continue;
      blocks.push(value); bytes += encoder.encode(value).length;
      if (blocks.length >= input.maxBlocks || bytes >= 4096) { truncated = true; break; }
    }
  } catch (error) {
    if (error !== limit) { guard.finish(); return { started: true, state: "unavailable" }; }
    truncated = true;
  }
  if (performance.now() >= cutoff || !guard.current()) { guard.finish(); return { started: true, state: "changed" }; }
  if (!blocks.length) return { started: true, state: "empty", dom_revision: guard.revision };
  // Generic excerpts do not collect a title; URL/document provenance is separate.
  const title = "";
  if (performance.now() >= cutoff || !guard.current()) { guard.finish(); return { started: true, state: "changed" }; }
  return { started: true, state: "excerpt", dom_revision: guard.revision, title, blocks, truncated, excluded_content: excluded };
}

// Runs only after all permission/document revalidation awaits. It returns no text.
export function finishPageExcerpt(request: string, url: string): { state: "settled"; request: string; url: string; dom_revision: number; unchanged: boolean } | { state: "unknown" } {
  const realm = globalThis as Realm;
  const guard = realm.__avesraReadGuard1;
  if (!guard || guard.request !== request || guard.url !== url) return { state: "unknown" };
  const unchanged = guard.finish();
  const revision = guard.revision;
  delete realm.__avesraReadGuard1;
  return { state: "settled", request, url, dom_revision: revision, unchanged };
}
