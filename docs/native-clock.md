# Native local clock answers

An accepted owner question about the current local time or date is a built-in,
non-sensitive read. It requires no Settings permission, action grant, application
target, model inference, or fabricated successful task. Existing voice admission,
paired owner registration, accepted-turn cancellation and reply ownership remain
mandatory.

The closed grammar is case-insensitive, collapses whitespace and permits trailing
sentence punctuation: `what time is it`, `what is the time`, `what's the time`,
`tell me the time`, `what is today's date`, `what's today's date`, `what is the date`,
`what's the date`, `what day is it`, and `tell me the date`. One leading or trailing
`please` is permitted. Other questions continue through ordinary resolution.
One exact leading `Avesra ` or `Avesra, ` addressing prefix is permitted before
the optional `please`; names are not fuzzy matched or inferred from ASR spelling.

The native effects worker reads Windows `GetLocalTime` only after receiving the
genuine planner claim. The numeric calendar/time fields, observed Unix milliseconds
and native monotonic observation instant form a typed reading; no frontend text
or historical row can create one. The original reply transaction checks the exact
question again, validates the calendar fields and derives the answer itself.
Observation-to-commit preparation is limited to two seconds, inside the original
accepted-turn lifetime. The answer says `Your PC clock says ...`; it does not
invent a timezone name or claim the PC clock is externally synchronized.

`NativeClock` speech provenance uses `kind: "native_clock"`, numeric `reading`
and `clock_kind: "time" | "date"`. Stored reply validation recomputes its exact answer. The existing
authenticated native-derived speech reservation consumes the original ordinal,
exact source digest and one output opportunity. This is a paired-native assertion,
not evidence of completed model inference. Old peers reject the unknown source
variant. The next store compatibility marker rejects older readers; existing
historical reply records remain unchanged.

Clock replies remain immutable history, but are excluded from future planner
dialogue retrieval so an old observation cannot masquerade as the current clock.
Legacy model replies to this exact clock grammar are also excluded from retrieval.
Ordinary speech cancellation, output epochs, selected voice and real retirement
apply unchanged. There is no clock Tauri command, preview bridge or alternate
accepted-turn ingress.

Entry points: accepted-task resolution routes the closed grammar; the retained
native effects command owns the OS read and reply transaction; the core constructor
accepts only a genuine claim and typed reading; stored reply/source validation
checks provenance; normal speech uses its existing native-derived reservation.
Settings previews, action execution, history reads and remote model replies cannot
construct this native source. Static/build checks are performed by the coordinating
reviewer; no automated tests or live audio operations are part of this source slice.
Actual spoken clock proof remains pending.
