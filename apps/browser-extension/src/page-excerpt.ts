// Bundled fixed functions serialized into the exact document's ISOLATED world.
// No page-facing command or caller-selected code is accepted.
import type { Choice, Provider } from "./provider.js";
type Guard = {
  request: string; url: string; deadline: number; revision: number; settled: boolean;
  current(): boolean; finish(): boolean;
};
type Realm = typeof globalThis & { __avesraReadGuard1?: Guard };
export type ReadParameters = { request: string; url: string; maxBlocks: number; budgetMs: number; provider: Provider | null; xReady: boolean };
export type Extracted = { started: true; state: "excerpt"; dom_revision: number; title: string; blocks: string[]; truncated: boolean; excluded_content: boolean }
  | { started: true; state: "provider_inspection"; dom_revision: number; provider: Provider; complete: boolean; choices: Choice[] }
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
  if (input.provider !== null && ((input.provider !== "gmail" && input.provider !== "x")
    || location.origin !== (input.provider === "gmail" ? "https://mail.google.com" : "https://x.com")))
    return { started: false, state: "unavailable", request: input.request, url: input.url, guard_absent: true };

  // Separate closure scope: no extracted text/arrays live in retained callbacks.
  function ownGuard(request: string, url: string, lifetime: number): Guard {
    let dirty = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const guard: Guard = {
      request, url, deadline: performance.now() + lifetime, revision: 1, settled: false,
      current() {
        if (performance.now() >= guard.deadline || location.href !== url || document.readyState !== "complete") retire(true);
        if (!guard.settled && observer.takeRecords().some(relevant)) retire(true);
        return !guard.settled && !dirty;
      },
      finish() {
        const unchanged = guard.current();
        retire(!unchanged);
        return unchanged;
      },
    };
    // X readiness deliberately excludes the feed. Changes strictly within an
    // excluded article/feed cannot change the observed account/composer. All
    // structural changes outside those subtrees, including their replacement,
    // remain invalidating. Generic excerpt behavior is unchanged.
    function relevant(record: MutationRecord): boolean {
      if(!input.xReady)return true;
      const target=record.target instanceof Element?record.target:record.target.parentElement;
      return !target?.closest('article,[role="article"],[role="feed"]');
    }
    const observer = new MutationObserver(records => {if(records.some(relevant))retire(true);});
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
  if (input.provider !== null) {
    const choices: Choice[] = [];
    const observed = new Map<Element, string>();
    let complete = true;
    const bounded = (text: string, maximum: number) => text.length <= maximum
      && encoder.encode(text).length <= maximum && !/[\p{Cc}\p{Cs}]/u.test(text);
    const allowed = ["id", "role", "datetime", "aria-label", "aria-labelledby", "aria-controls", "aria-expanded", "aria-selected",
      "data-testid", "data-message-id", "data-legacy-message-id", "data-thread-id", "data-legacy-thread-id"];
    const visibility = new Map<HTMLElement,boolean>();
    const visibleMetadata = (node: HTMLElement) => {
      const pending: HTMLElement[] = [];
      let visible = true;
      for (let parent: HTMLElement | null = node, depth=0; parent; parent=parent.parentElement) {
        check(); if (++depth>64) throw limit;
        const cached=visibility.get(parent);if(cached!==undefined){visible=cached;break;}
        pending.push(parent);
        if (["SCRIPT","STYLE","NOSCRIPT","TEMPLATE","IFRAME","FRAME","OBJECT","EMBED","INPUT","TEXTAREA","SELECT"].includes(parent.tagName)
          || parent.isContentEditable || parent.hidden || parent.inert || parent.getAttribute("aria-hidden") === "true"
          || ["textbox","combobox","spinbutton"].includes(parent.getAttribute("role")??"")) {visible=false;break;}
        const style=getComputedStyle(parent);
        if(style.display==="none"||style.visibility!=="visible"||style.opacity==="0"||style.contentVisibility==="hidden"
          ||style.clipPath!=="none"||style.maskImage!=="none"){visible=false;break;}
      }
      for(const item of pending)visibility.set(item,visible);
      return visible;
    };
    const labelText = (element: HTMLElement) => {
      if (!visibleMetadata(element)) return "";
      let text=""; const textWalker=document.createTreeWalker(element,NodeFilter.SHOW_ELEMENT|NodeFilter.SHOW_TEXT,{acceptNode(node){
        check();
        return node instanceof HTMLElement&&!visibleMetadata(node)?NodeFilter.FILTER_REJECT:NodeFilter.FILTER_ACCEPT;
      }});
      for(let child=textWalker.nextNode();child;child=textWalker.nextNode()){
        check(); if(!(child instanceof Text))continue;
        if(child.length>128)throw limit;
        text+=child.data;if(!bounded(text,128))throw limit;
      }
      return text.trim();
    };
    // No arbitrary selector or provider field meaning is inferred here. These
    // are actual bounded element metadata for later independently reviewed binding.
    try {
      // Discover app chrome only. Message/feed/form/dialog subtrees cannot
      // contribute a forged account header or consume the header choice budget.
      const headers: HTMLElement[]=[];
      const roots=document.createTreeWalker(document.body,NodeFilter.SHOW_ELEMENT,{acceptNode(node){
        check();if(!(node instanceof HTMLElement)||!visibleMetadata(node))return NodeFilter.FILTER_REJECT;
        if (["MAIN","ARTICLE","FORM","DIALOG","TABLE","UL","OL"].includes(node.tagName)
          || ["main","article","feed","dialog","list","grid","table","tree"].includes(node.getAttribute("role")??""))return NodeFilter.FILTER_REJECT;
        if(node.tagName==="HEADER"||node.getAttribute("role")==="banner"){
          headers.push(node);return NodeFilter.FILTER_REJECT;
        }
        return NodeFilter.FILTER_ACCEPT;
      }});
      while(roots.nextNode()){check();}
      if(headers.length!==1)throw limit;
      const header=headers[0];
      const headerId=crypto.randomUUID();observed.set(header,headerId);
      choices.push({id:headerId,parent:null,role:"banner",label:"",attributes:[]});
      const walker = document.createTreeWalker(header, NodeFilter.SHOW_ELEMENT, { acceptNode(node) {
        check();
        if (!(node instanceof HTMLElement) || ["SCRIPT","STYLE","NOSCRIPT","TEMPLATE","IFRAME","FRAME","OBJECT","EMBED","INPUT","TEXTAREA","SELECT"].includes(node.tagName)
          || node.isContentEditable || node.hidden || node.inert || node.getAttribute("aria-hidden") === "true") return NodeFilter.FILTER_REJECT;
        if (!visibleMetadata(node)) return NodeFilter.FILTER_REJECT;
        if (node.shadowRoot) complete = false;
        return NodeFilter.FILTER_ACCEPT;
      }});
      if (document.body.hidden || document.body.inert || document.body.isContentEditable
        || document.body.getAttribute("aria-hidden") === "true") throw limit;
      for (let node = walker.nextNode(); node; node = walker.nextNode()) {
        check();
        if (!(node instanceof HTMLElement)) continue;
        const nativeRole: Record<string,Choice["role"]> = { BUTTON:"button",A:"link",ARTICLE:"article",UL:"list",OL:"list",LI:"list_item",H1:"heading",H2:"heading",H3:"heading",H4:"heading",H5:"heading",H6:"heading",MAIN:"main",NAV:"navigation",HEADER:"banner" };
        const explicit = node.getAttribute("role");
        const role = explicit === "listitem" ? "list_item" : explicit;
        const resolved = (["button","link","article","list","list_item","heading","group","banner","main","navigation"].includes(role ?? "") ? role : nativeRole[node.tagName]) as Choice["role"] | undefined;
        if (!resolved) continue;
        let label = (node.getAttribute("aria-label") ?? "").replace(/[\n\r\t]+/g," ");
        const labelled = node.getAttribute("aria-labelledby");
        if (!label && labelled) {
          const ids = labelled.split(/\s+/); if (ids.length > 4) { complete = false; continue; }
          const labels: string[] = [];
          for (const id of ids) {
            const element = document.getElementById(id);
            if (!element || element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement || element.isContentEditable
              || element.hidden || element.getAttribute("aria-hidden") === "true") { complete = false; continue; }
            // textContent can allocate a page-sized string; bound the actual text
            // nodes and refuse nested controls rather than reading their values.
            labels.push(labelText(element));
          }
          label = labels.join(" ");
        }
        if (!label && ["button","link","heading"].includes(resolved)) {
          label = labelText(node);
        }
        label = label.trim();
        if (!bounded(label,128)) { complete = false; continue; }
        const attributes: Choice["attributes"] = [];
        for (const name of allowed) {
          let value = node.getAttribute(name); if (value === null) continue;
          if(name==="aria-label")value=value.replace(/[\n\r\t]+/g," ");
          if (!bounded(value,96) || attributes.length === 4) { complete = false; continue; }
          attributes.push({name,value});
        }
        let parent: string | null = null, depth = 0;
        for (let ancestor = node.parentElement; ancestor; ancestor = ancestor.parentElement) {
          check(); if (++depth > 64) throw limit;
          const id = observed.get(ancestor); if (id) { parent=id;break; }
        }
        if (choices.length === 64) throw limit;
        const id=crypto.randomUUID(); observed.set(node,id);
        choices.push({id,parent,role:resolved,label,attributes});
      }
    } catch { complete = false; }
    if (!guard.current()) return {started:true,state:"changed"};
    return {started:true,state:"provider_inspection",dom_revision:guard.revision,provider:input.provider,complete,choices};
  }
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
