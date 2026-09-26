// Strict correlation data only. Parsing never admits a Chrome operation.
import * as p from "./protocol.js";
import { candidate, type Candidate } from "./documents.js";
import { origin as providerOrigin, type Provider, type Probe } from "./provider.js";
export type Source = { device: string; session: string; capture_epoch: number; action_epoch: number };
export type Context = {
  request: string; dispatch: string; task: string; step: string; actor: string;
  action_revision: string; intent_revision: string; grant: string;
  target: p.Pairing; scope: p.Pairing; pairing: p.Pairing; selection: string;
  browser_app: p.Pairing; browser_session: string; browser_generation: number;
  observation_revision: number; source: Source;
};
export type Mode = { kind: "excerpt" } | { kind: "provider_inspection"; provider: Provider };
export type Request = { context: Context; origin: string; document: Candidate; message_limit: number; mode: Mode; remaining_ms: number };
export type Settlement = { context: Context; kind: "actual_job_settled" };
export type Outcome = { state: "excerpt"; excerpt: { coverage: "partial"; document: Candidate; dom_revision: number; title: string; blocks: string[]; truncated: boolean; excluded_content: boolean } }
  | { state: "provider_inspection"; probe: Probe }
  | { state: "empty" | "changed" | "expired" | "unavailable" | "unsupported" };
export type Reply = { context: Context; outcome: Outcome };
const ids = ["request", "dispatch", "task", "step", "actor", "action_revision", "intent_revision", "grant", "selection", "browser_session"];
const references = ["target", "scope", "pairing", "browser_app"];
export function context(value: unknown): value is Context {
  return p.object(value, [...ids, ...references, "browser_generation", "observation_revision", "source"])
    && ids.every(key => p.id(value[key])) && references.every(key => p.pairing(value[key]))
    && p.counter(value.browser_generation) && p.counter(value.observation_revision)
    && p.object(value.source, ["device", "session", "capture_epoch", "action_epoch"])
    && p.id(value.source.device) && p.id(value.source.session)
    && p.counter(value.source.capture_epoch) && p.counter(value.source.action_epoch);
}
export function sameContext(a: Context, b: Context): boolean {
  return a.request === b.request && a.dispatch === b.dispatch && a.task === b.task
    && a.step === b.step && a.actor === b.actor && a.action_revision === b.action_revision
    && a.intent_revision === b.intent_revision && a.grant === b.grant
    && p.equal(a.target, b.target) && p.equal(a.scope, b.scope) && p.equal(a.pairing, b.pairing)
    && a.selection === b.selection && p.equal(a.browser_app, b.browser_app)
    && a.browser_session === b.browser_session && a.browser_generation === b.browser_generation
    && a.observation_revision === b.observation_revision
    && a.source.device === b.source.device && a.source.session === b.source.session
    && a.source.capture_epoch === b.source.capture_epoch && a.source.action_epoch === b.source.action_epoch;
}
export function request(value: unknown): value is Request {
  return p.object(value, ["context", "origin", "document", "message_limit", "mode", "remaining_ms"])
    && context(value.context) && p.origin(value.origin) && candidate(value.document, value.origin)
    && p.counter(value.message_limit) && value.message_limit <= 100
    && ((p.object(value.mode,["kind"]) && value.mode.kind === "excerpt")
      || (p.object(value.mode,["kind","provider"]) && value.mode.kind === "provider_inspection"
        && (value.mode.provider === "gmail" || value.mode.provider === "x") && value.origin === providerOrigin(value.mode.provider) && value.message_limit === 1))
    && p.counter(value.remaining_ms) && value.remaining_ms <= 10000
    && new TextEncoder().encode(JSON.stringify(value)).length <= 65536 - 2048;
}
export function settlement(value: unknown): value is Settlement {
  return p.object(value, ["context", "kind"]) && context(value.context) && value.kind === "actual_job_settled";
}
