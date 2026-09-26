import { listen } from "@tauri-apps/api/event";
import { command, native, type Runtime, type Settings } from "./runtime";

export const preferenceFields = ["microphone", "speaker", "learning_chime", "action_chime", "learning_chime_volume", "action_chime_volume", "profile", "interface_scale", "always_on_top"] as const;
export type PreferenceField = typeof preferenceFields[number];
type Value = Settings[PreferenceField];
type Edit = { expected: Value; value: Value };
type Changes = Partial<Record<PreferenceField, Edit>>;
type CloseRequest = { sequence: number; editor: string; request: string; intent: "hide" | "quit" };
type Registration = { sequence: number; editor: string; close: CloseRequest | null };
type Applied = { kind: "applied"; snapshot: Runtime; durable: boolean; problems: string[] };
type Result = Applied | { kind: "conflict"; snapshot: Runtime; fields: PreferenceField[] };

export class PreferenceDraft {
  changes = $state<Changes>({});
  editor = $state<string | null>(null);
  busy = $state(false);
  closing = $state(false);
  closeRequest = $state<CloseRequest | null>(null);
  message = $state("");
  concern = $state(false);
  reviewed = $state(false);
  saved = $state(false);
  review = $state<Settings | null>(null);
  private mounted = false;
  private generation = 0;
  private registering = false;
  private eventsReady = false;
  private visible = false;
  private registerQueued = false;
  private context = "";
  private page = crypto.randomUUID();
  private earlyClose: CloseRequest | null = null;
  private closeSequence = 0;
  constructor(private runtime: () => Runtime | null, private accept: (value: Runtime) => void) {}
  get rows() { return preferenceFields.filter(field => this.changes[field]).map(field => ({ field, ...this.changes[field]! })); }
  get dirty() { return this.rows.length > 0 || this.concern; }
  get blocked() { return !native || !this.editor || !this.runtime() || !!this.runtime()?.locked || this.busy || this.closing || !!this.closeRequest || this.concern; }
  get settings(): Settings | undefined {
    const current = this.runtime()?.settings;
    if (!current) return;
    return { ...current, ...Object.fromEntries(this.rows.map(row => [row.field, row.value])) };
  }
  get conflicts() {
    const current = this.runtime()?.settings;
    return current ? this.rows.filter(row => current[row.field] !== row.expected).map(row => row.field) : [];
  }
  edit(patch: Partial<Pick<Settings, PreferenceField>>) {
    if (this.blocked) return;
    const current = this.runtime()!.settings;
    const next = { ...this.changes };
    for (const field of preferenceFields) if (Object.hasOwn(patch, field)) {
      const value = patch[field]!;
      const expected = next[field] ? next[field]!.expected : current[field];
      if (value === expected) delete next[field]; else next[field] = { expected, value };
    }
    this.changes = next; this.saved = false; this.message = ""; this.review = null; this.reviewed = false;
  }
  discard() {
    if (this.busy || this.closing || this.concern) return;
    this.changes = {}; this.message = ""; this.review = null; this.reviewed = false; this.saved = false;
  }
  private clear() {
    const previous = this.editor;
    this.generation++; this.editor = null; this.page = crypto.randomUUID(); this.earlyClose = null; this.closeSequence = 0;
    if (previous && native) void command("retire_preferences_editor", { editor: previous }).catch(() => {}); this.changes = {}; this.concern = false;
    this.message = ""; this.saved = false; this.review = null; this.reviewed = false; this.closeRequest = null;
  }
  observe() {
    const value = this.runtime();
    const next = `${value?.locked}`;
    if (next !== this.context) { this.context = next; this.clear(); }
    if (value && !value.locked && this.mounted && this.eventsReady && !this.editor && this.visible) void this.register();
  }
  private async register(explicit = false) {
    if (!this.mounted || !this.eventsReady || this.editor || (!explicit && !this.visible) || this.runtime()?.locked) return;
    if (this.registering) { this.registerQueued = true; return; }
    this.registering = true; const token = this.generation;
    try {
      const registration = await command<Registration>("begin_preferences_editor", { page: this.page });
      const editor = registration.editor;
      if (this.mounted && token === this.generation && !this.runtime()?.locked) {
        this.editor = editor; this.visible = true; this.message = "";
        const pending = this.earlyClose?.editor === editor && this.earlyClose.sequence > registration.sequence ? this.earlyClose : registration.close;
        this.closeSequence = Math.max(registration.sequence, pending?.sequence ?? 0);
        this.earlyClose = null;
        if (pending?.editor === editor) {
          this.closeRequest = pending;
          if (!this.dirty && !this.busy) void this.answer(true);
        }
      } else {
        await command("retire_preferences_editor", { editor });
      }
    } catch (error) { if (this.mounted && token === this.generation) this.message = `Preference editor unavailable: ${String(error)}`; }
    finally {
      this.registering = false;
      const queued = this.registerQueued; this.registerQueued = false;
      if (queued && this.mounted && this.visible && !this.runtime()?.locked) void this.register();
    }
  }
  retryEditor() { void this.register(true); }
  async save() {
    if (!this.editor || this.busy || this.closing || this.closeRequest || !this.dirty || this.runtime()?.locked || this.conflicts.length || (this.concern && !this.reviewed)) return;
    const editor = this.editor, token = this.generation, changes = this.rows;
    this.busy = true; this.message = ""; this.saved = false;
    try {
      const result = await command<Result>("apply_preferences", { editor, changes });
      if (!this.mounted || token !== this.generation) return;
      this.accept(result.snapshot);
      this.review = null; this.reviewed = false;
      if (result.kind === "conflict") { this.message = "These preferences changed elsewhere. Review current values before saving."; return; }
      if (result.durable && !result.problems.length) {
        this.changes = {}; this.concern = false; this.saved = true;
        this.message = "Saved on this PC. Changes apply immediately.";
      } else {
        this.changes = Object.fromEntries(changes.map(row => [row.field, { expected: row.value, value: row.value }]));
        this.concern = true;
        this.message = result.durable ? "Preferences saved, but a window update failed. Review current values and retry; changes are already live." : "Preferences applied, but saving failed. Review current values and retry; Discard cannot undo live changes.";
      }
    } catch (error) {
      if (this.mounted && token === this.generation) {
        this.concern = true; this.review = null;
        this.message = `Save could not be confirmed. Changes may already be live. Review current values before retrying. ${String(error)}`;
      }
    } finally {
      this.busy = false;
      if (this.mounted && token === this.generation && this.closeRequest && !this.dirty) void this.answer(true);
    }
  }
  async inspect() {
    if (this.busy || this.closing || this.closeRequest || !this.editor) return;
    this.busy = true; this.review = null; this.reviewed = false; const token = this.generation;
    try {
      const current = await command<Runtime>("runtime_snapshot");
      if (this.mounted && token === this.generation) { this.accept(current); this.review = current.settings; }
    } catch (error) { if (this.mounted && token === this.generation) this.message = String(error); }
    finally { this.busy = false; }
  }
  keepDraft() {
    const current = this.runtime()?.settings;
    if (!this.review || !current || this.busy || this.closing || this.closeRequest) return;
    if (this.rows.some(row => current[row.field] !== this.review![row.field])) { this.review = null; this.message = "Preferences changed again. Review the current values."; return; }
    this.changes = Object.fromEntries(this.rows.map(row => [row.field, { expected: current[row.field], value: row.value }]));
    // An applied/uncertain save still needs an explicit acknowledged retry.
    this.reviewed = true; this.review = null; this.message = this.concern ? "Current values reviewed. Retry to confirm saving; live changes are not undone." : "Draft reviewed against current values. Save to apply it.";
  }
  async refreshClose() {
    if (this.busy || this.closing || !this.editor || !this.closeRequest) return;
    this.closing = true; const token = this.generation;
    try {
      const registration = await command<Registration>("begin_preferences_editor", { page: this.page });
      if (!this.mounted || token !== this.generation) return;
      if (registration.editor !== this.editor) { this.message = "Settings editor changed. Reopen Settings to continue."; return; }
      if (registration.sequence >= this.closeSequence) {
        this.closeSequence = registration.sequence; this.closeRequest = registration.close;
        this.message = registration.close ? "Close decision refreshed." : "Keep editing. Your draft is preserved.";
      }
    } catch (error) { if (this.mounted && token === this.generation) this.message = String(error); }
    finally { this.closing = false; }
  }
  async answer(close: boolean) {
    const pending = this.closeRequest;
    if (!pending || this.busy || this.closing) return;
    this.closing = true; const token = this.generation;
    try {
      await command("answer_preferences_close", { editor: pending.editor, request: pending.request, close });
      if (this.mounted && token === this.generation && this.closeRequest?.request === pending.request) {
        this.closeRequest = null;
        if (close) this.clear();
      }
    } catch (error) { if (this.mounted && token === this.generation) this.message = String(error); }
    finally { this.closing = false; }
  }
  mount() {
    this.mounted = true;
    const stops: (() => void)[] = [];
    if (native) void (async () => {
      const close = await listen<CloseRequest>("settings-close-requested", event => {
        if (!this.mounted) return;
        if (event.payload.editor !== this.editor) {
          if (this.registering && (!this.earlyClose || event.payload.sequence > this.earlyClose.sequence)) this.earlyClose = event.payload;
          return;
        }
        if (event.payload.sequence <= this.closeSequence) return;
        this.closeSequence = event.payload.sequence;
        this.closeRequest = event.payload;
        if (!this.concern) this.message = "";
        if (!this.dirty && !this.busy) void this.answer(true);
      });
      if (!this.mounted) { close(); return; } stops.push(close);
      const hidden = await listen("settings-hidden", () => { this.visible = false; this.registerQueued = false; this.clear(); });
      if (!this.mounted) { hidden(); return; } stops.push(hidden);
      const shown = await listen("settings-shown", () => { this.visible = true; void this.register(); });
      if (!this.mounted) { shown(); return; } stops.push(shown);
      this.eventsReady = true; void this.register(true);
    })().catch(error => { if (this.mounted) this.message = `Preference close notifications unavailable: ${String(error)}`; });
    return () => { this.mounted = false; this.eventsReady = false; this.clear(); stops.forEach(stop => stop()); };
  }
}
