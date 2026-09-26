// Semantic provider data only. This module cannot read DOM, click, navigate,
// mint an account binding, or replace C4's actual Chrome ownership checks.
import * as p from "./protocol.js";

export type Message = {
  id: string; thread: string; timestamp_ms: number; sender: string;
  subject: string; reference: string; body: string;
  body_complete: true; thread_expanded: true;
};
export type Batch = {
  binding_revision: string; account: string; scope: "inbox";
  document: string; dom_revision: number; ordinal: number;
  cursor: string | null; next: string | null; end_of_inbox: boolean;
  order: "individual_messages_newest_first"; messages: Message[];
};

const encoder = new TextEncoder();
function identity(value: unknown, maximum: number): value is string {
  return typeof value === "string" && value.length > 0 && value.length <= maximum
    && /^[\x21-\x7e]+$/.test(value);
}
function text(value: unknown, maximum: number, multiline = false): value is string {
  return typeof value === "string" && value.length <= maximum
    && encoder.encode(value).length <= maximum
    && !/[\p{Cc}\p{Cs}]/u.test(multiline ? value.replace(/[\n\t]/g, "") : value);
}
function message(value: unknown): value is Message {
  return p.object(value, ["id", "thread", "timestamp_ms", "sender", "subject", "reference", "body", "body_complete", "thread_expanded"])
    && identity(value.id, 128) && identity(value.thread, 128)
    && p.counter(value.timestamp_ms) && value.timestamp_ms <= 8_640_000_000_000_000
    && text(value.sender, 320) && !!value.sender.trim() && text(value.subject, 512)
    && identity(value.reference, 2048) && text(value.body, 16384, true)
    && value.body_complete === true && value.thread_expanded === true;
}

export function batch(value: unknown, requested: number): value is Batch {
  if (!Number.isInteger(requested) || requested < 1 || requested > 100
    || !p.object(value, ["binding_revision", "account", "scope", "document", "dom_revision", "ordinal", "cursor", "next", "end_of_inbox", "order", "messages"])
    || !p.id(value.binding_revision) || !identity(value.account, 320) || value.scope !== "inbox"
    || !identity(value.document, 128) || !p.counter(value.dom_revision)
    || !Number.isInteger(value.ordinal) || (value.ordinal as number) < 0 || (value.ordinal as number) >= 32
    || !(value.cursor === null || identity(value.cursor, 256))
    || !(value.next === null || identity(value.next, 256))
    || typeof value.end_of_inbox !== "boolean" || value.end_of_inbox === (value.next !== null)
    || value.order !== "individual_messages_newest_first" || !Array.isArray(value.messages)
    || value.messages.length > requested || (!value.end_of_inbox && value.messages.length === 0)) return false;
  const ids = new Set<string>();
  let previous = Number.MAX_SAFE_INTEGER, bytes = 0;
  for (const item of value.messages) {
    if (!message(item) || ids.has(item.id) || item.timestamp_ms > previous) return false;
    previous = item.timestamp_ms; ids.add(item.id); bytes += encoder.encode(item.body).length;
    if (bytes > 262144) return false;
  }
  return true;
}

// Readiness is a separate provider observation. A URL, profile selection or
// installed extension cannot set authenticated/composeEnabled on its own.
export type Readiness = {
  binding_revision: string; document: string; dom_revision: number;
  origin: "https://x.com"; account: string;
  state: "ready" | "login_required" | "challenge" | "unsupported";
};
export function readiness(value: unknown): value is Readiness {
  return p.object(value, ["binding_revision", "document", "dom_revision", "origin", "account", "state"])
    && p.id(value.binding_revision) && identity(value.document, 128) && p.counter(value.dom_revision)
    && value.origin === "https://x.com" && identity(value.account, 320)
    && ["ready", "login_required", "challenge", "unsupported"].includes(String(value.state));
}
