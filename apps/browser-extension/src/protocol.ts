export type Pairing = { id: string; revision: string };
export type Challenge = { version: 2; installation: string; connection: string; session: string; challenge: string; nonce: string; pairing: Pairing | null };
export type Saved = { version: 1; installation: string; pairing: Pairing; credential: string };
export function object(value: unknown, keys: string[]): value is Record<string, unknown> {
  return !!value && typeof value === "object" && !Array.isArray(value) && Object.keys(value).length === keys.length && keys.every(k => Object.hasOwn(value, k));
}
export function id(value: unknown): value is string { return typeof value === "string" && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value) && value !== "00000000-0000-0000-0000-000000000000"; }
export function hex(value: unknown): value is string { return typeof value === "string" && /^[0-9a-f]{64}$/.test(value); }
export function pairing(value: unknown): value is Pairing { return object(value, ["id", "revision"]) && id(value.id) && id(value.revision); }
export function equal(a: Pairing | null, b: Pairing | null) { return a === null ? b === null : b !== null && a.id === b.id && a.revision === b.revision; }
export function challenge(value: unknown): value is Challenge {
  return object(value, ["version", "installation", "connection", "session", "challenge", "nonce", "pairing"]) && value.version === 2 && id(value.installation) && id(value.connection) && id(value.session) && id(value.challenge) && hex(value.nonce) && (value.pairing === null || pairing(value.pairing));
}
export function saved(value: unknown): value is Saved { return object(value, ["version", "installation", "pairing", "credential"]) && value.version === 1 && id(value.installation) && pairing(value.pairing) && hex(value.credential); }
export function bytes(value: string): Uint8Array<ArrayBuffer> { return Uint8Array.from(value.match(/../g) ?? [], v => Number.parseInt(v, 16)); }
export function toHex(value: Uint8Array) { return Array.from(value, v => v.toString(16).padStart(2, "0")).join(""); }
export function nonce() { return toHex(crypto.getRandomValues(new Uint8Array(32))); }
export async function comparison(value: Challenge) {
  const text = ["AVESRA-BROWSER-COMPARE-2", value.installation, value.connection, value.session, value.challenge, value.nonce, ""].join("\n");
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
  return (new DataView(digest).getUint32(0, false) % 1_000_000).toString().padStart(6, "0");
}
export async function proof(value: Challenge, record: Saved, clientNonce: string) {
  if (!equal(value.pairing, record.pairing) || value.installation !== record.installation) throw new Error("Pairing identity changed");
  const raw = bytes(record.credential);
  try {
    const key = await crypto.subtle.importKey("raw", raw, { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
    const text = ["AVESRA-BROWSER-AUTH-2", record.pairing.id, record.pairing.revision, value.installation, value.connection, value.session, clientNonce, value.nonce, chrome.runtime.id, ""].join("\n");
    return toHex(new Uint8Array(await crypto.subtle.sign("HMAC", key, new TextEncoder().encode(text))));
  } finally { raw.fill(0); }
}

// Every storage entry point establishes this restriction before reading/writing.
async function trusted() { await chrome.storage.local.setAccessLevel({ accessLevel: "TRUSTED_CONTEXTS" }); }
let initialization: Promise<{ installation: string; record: Saved | null }> | null = null;
let writing = false;
export function isWriting() { return writing; }
export function load() {
  if (initialization) return initialization;
  const work = (async () => {
    await trusted();
    const value = await chrome.storage.local.get(["avesraInstallation", "avesraPairing"]);
    if (value.avesraInstallation !== undefined && !id(value.avesraInstallation)) throw new Error("Stored installation is invalid; no identity was replaced");
    const installation = value.avesraInstallation ?? crypto.randomUUID();
    if (value.avesraInstallation === undefined) {
      if (value.avesraPairing !== undefined) throw new Error("Stored pairing has no installation; no identity was replaced");
      await trusted(); await chrome.storage.local.set({ avesraInstallation: installation });
    }
    const record = value.avesraPairing === undefined ? null : value.avesraPairing;
    if (record !== null && (!saved(record) || record.installation !== installation)) throw new Error("Stored pairing is unavailable; explicit recovery is required");
    return { installation, record: record as Saved | null };
  })();
  initialization = work;
  void work.finally(() => { if (initialization === work) initialization = null; }).catch(() => {});
  return work;
}
export async function persist(record: Saved) {
  if (writing) throw new Error("Credential storage is busy");
  writing = true;
  try {
  if (!saved(record)) throw new Error("Invalid issued pairing");
  await trusted();
  const current = await chrome.storage.local.get(["avesraInstallation", "avesraPairing"]);
  if (current.avesraInstallation !== record.installation || current.avesraPairing !== undefined) throw new Error("Pairing storage changed; no credential was replaced");
  await trusted(); await chrome.storage.local.set({ avesraPairing: record });
  initialization = null;
  const reloaded = await load();
  if (!reloaded.record || !equal(reloaded.record.pairing, record.pairing) || reloaded.record.credential !== record.credential) throw new Error("Pairing persistence is uncertain");
  } finally { writing = false; initialization = null; }
}
export async function forget(expected: Pairing) {
  if (writing) throw new Error("Credential storage is busy");
  writing = true;
  try {
  await trusted();
  const current = await chrome.storage.local.get("avesraPairing");
  const value: unknown = current.avesraPairing;
  if (!value || typeof value !== "object" || !("pairing" in value) || !pairing(value.pairing) || !equal(value.pairing, expected)) throw new Error("Saved pairing revision changed");
  await trusted(); await chrome.storage.local.remove("avesraPairing"); initialization = null;
  } finally { writing = false; initialization = null; }
}
export async function recoveryIdentity(): Promise<Pairing | null> {
  await trusted(); const current = await chrome.storage.local.get("avesraPairing");
  const value: unknown = current.avesraPairing;
  return value && typeof value === "object" && "pairing" in value && pairing(value.pairing) ? value.pairing : null;
}
