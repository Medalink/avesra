import * as p from "./protocol.js";
import { ObservationAuthority } from "./authority.js";
export type View = { reference: p.Pairing; origin: string; operations: p.ScopeOperation[]; session: string; generation: number; selection: string; action_epoch: number; requested: boolean; remaining_ms: number };
export function view(value: unknown): value is View {
  return p.object(value, ["reference", "origin", "operations", "session", "generation", "selection", "action_epoch", "requested", "remaining_ms"])
    && p.pairing(value.reference) && p.origin(value.origin) && p.operations(value.operations) && p.id(value.session) && p.counter(value.generation)
    && p.id(value.selection) && p.counter(value.action_epoch) && typeof value.requested === "boolean" && p.counter(value.remaining_ms) && value.remaining_ms <= 45000;
}
type Pending = { value: View; deadline: number; owner: ReturnType<ObservationAuthority["snapshot"]>; checking: boolean; recheck: boolean; decision: boolean | null; submitted: boolean };
export class PermissionProposal {
  private pending: Pending | null = null;
  private disposed = false;
  constructor(private readonly authority: ObservationAuthority, private readonly connected: () => boolean) {}
  private current(value: Pending) {
    return !this.disposed && this.connected() && this.pending === value && performance.now() < value.deadline && this.authority.current(value.owner);
  }
  observe(status: p.Status) {
    const scope = status.scope, context = this.authority.snapshot();
    if (!scope || scope.state !== "pending" || !context) { this.pending = null; return; }
    if (this.pending && p.equal(this.pending.value.reference, scope.reference)) {
      const old = this.pending;
      if (!this.current(old) || old.value.origin !== scope.origin || JSON.stringify(old.value.operations) !== JSON.stringify(scope.operations) || old.value.session !== status.session || old.value.generation !== status.generation) throw new Error("Scope proposal changed");
      old.deadline = Math.min(old.deadline, performance.now() + scope.remaining_ms);
      return;
    }
    this.pending = { value: { reference: scope.reference, origin: scope.origin, operations: [...scope.operations], session: status.session, generation: status.generation, selection: context.selection, action_epoch: context.action_epoch, requested: false, remaining_ms: scope.remaining_ms }, deadline: performance.now() + scope.remaining_ms, owner: context, checking: false, recheck: false, decision: null, submitted: false };
  }
  snapshot(): View | null {
    const value = this.pending;
    if (!value || !this.current(value)) return null;
    return { ...value.value, reference: { ...value.value.reference }, operations: [...value.value.operations], remaining_ms: Math.max(1, Math.floor(value.deadline - performance.now())) };
  }
  private matches(value: View, pending: Pending) {
    const actual = pending.value;
    return p.equal(value.reference, actual.reference) && value.origin === actual.origin && JSON.stringify(value.operations) === JSON.stringify(actual.operations) && value.session === actual.session && value.generation === actual.generation && value.selection === actual.selection && value.action_epoch === actual.action_epoch;
  }
  intent(value: View) {
    const pending = this.pending;
    if (!pending || !this.current(pending) || !this.matches(value, pending) || pending.value.requested) return false;
    pending.value.requested = true;
    void this.reconcile();
    return true;
  }
  decline(value: View) {
    const pending = this.pending;
    if (!pending || !this.current(pending) || !this.matches(value, pending) || pending.value.requested) return;
    pending.value.requested = true; pending.decision = false;
  }
  completed(value: View, permitted: boolean) {
    const pending = this.pending;
    if (!pending || !this.current(pending) || !this.matches(value, pending) || !pending.value.requested || pending.submitted) return;
    if (!permitted) pending.decision = false;
    else void this.reconcile();
  }
  async reconcile() {
    const pending = this.pending;
    // Events and status are not popup intent. Never adopt an old grant into an
    // unclicked replacement proposal with the same origin.
    if (!pending || !this.current(pending) || !pending.value.requested || pending.submitted || pending.decision !== null) return;
    if (pending.checking) { pending.recheck = true; return; }
    pending.checking = true; pending.recheck = false;
    try {
      const allowed = await chrome.permissions.contains({ origins: [pending.value.origin + "/*"] });
      if (this.current(pending) && pending.decision === null && allowed) pending.decision = true;
    } catch { /* Unknown, not a grant or automatic permission-request retry. */ }
    finally { pending.checking = false; if (pending.recheck && this.current(pending)) void this.reconcile(); }
  }
  takeDecision() {
    const pending = this.pending;
    if (!pending || !this.current(pending) || pending.decision === null || pending.submitted) return null;
    pending.submitted = true;
    return { reference: pending.value.reference, action_epoch: pending.value.action_epoch, permitted: pending.decision };
  }
  dispose() { this.disposed = true; this.pending = null; }
}
