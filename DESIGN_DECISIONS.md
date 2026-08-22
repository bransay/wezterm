# Design Decisions: Profiling Abstraction for Wezterm

<!--
The "why" log. Derived from conversation.
- Monotonic IDs: DD-001, DD-002, …
- Amend/reconcile freely when themes consolidate or decisions reverse.
- One entry per meaningful fork — skip micro-choices.

Entry shape:
## DD-NNN: <Short title>
- **Date:** YYYY-MM-DD
- **Phase:** <PLAN.md phase ref>
- **Status:** Accepted | Reversed | Superseded
- **Context:** <what prompted this>
- **Decision:** <the choice>
- **Rationale:** <why, including tradeoffs>
- **Alternatives considered:** <if non-trivial>
- **Consequences:** <what this locks in>
-->

## DD-001: RAII guard over manual start/end timing
- **Date:** 2026-08-22
- **Phase:** pre-Phase 1
- **Status:** Accepted
- **Context:** Wezterm's 13 duration marker sites use manual `let start = Instant::now(); ... metrics::histogram!("name").record(start.elapsed())`. This is less error-prone than raw start/end pairs (compiler warns on unused `start`) but conflates intent ("time this region") with backend ("histogram"), hardcodes the metrics crate at every call site, and prevents swapping in a zone-based profiler like Tracy.
- **Decision:** Introduce a RAII guard abstraction (`profile_zone!` macro) that captures `Instant::now()` on construction and records elapsed on Drop. The default specialization emits to `metrics::histogram!` (preserving current behavior). A Tracy specialization emits `tracy_client::span!` on construction and finish on Drop, behind a feature flag. Both backends can fire simultaneously.
- **Rationale:** The `metrics` crate is a telemetry facade with no concept of temporal regions — it receives a scalar duration, not a start/end timestamp. Tracy zones need the actual start timestamp to place the span on the timeline. A `metrics`→Tracy bridge is functionally impossible for zone data. A dedicated abstraction captures the `Instant` at construction, giving both backends what they need. Zero runtime overhead after monomorphization and inlining.
- **Alternatives considered:**
  - Implement a `metrics::Recorder` that routes to Tracy — rejected: the recorder only receives `f64` durations, not timestamps, so Tracy can't place zones on the timeline.
  - Keep manual markers, add Tracy separately — rejected: duplicates instrumentation effort, two marker systems to maintain, human error on both.
- **Consequences:** All 13 duration sites get refactored to use the guard. The 26 value/counter markers stay as-is (they're not regions). Tracy becomes an opt-in build feature. The abstraction is the extension point for future profiling backends.

## DD-002: Tracy as profiling backend (not samply)
- **Date:** 2026-08-22
- **Phase:** pre-Phase 1
- **Status:** Accepted
- **Context:** Wezterm has a render loop with frame-level variance (spikes, stutter). The existing histogram recorder gives distributional data (p50/p75/p95) but no chronological view — you can't see *which* frame spiked or *why*. Sampling profilers (samply, perf) give aggregate heat but not per-frame attribution.
- **Decision:** Use Tracy as the profiling backend. Tracy's chronological frame view shows each frame on a timeline, lets you spot outlier frames, and drill into which zone blew up. Zones map directly to the existing marker hierarchy (frame → pane → line → cluster).
- **Rationale:** The perf questions wezterm needs answered are "which frame spiked and why," not just "which function is hot on average." Tracy answers the first; histograms and sampling profilers answer the second. User (Bryan) is already fluent in Tracy from gamedev. The existing marker points are ready-made zone insertion sites.
- **Alternatives considered:**
  - samply only — rejected: sampling gives aggregate heat, not per-frame variance. Useful for discovery but doesn't answer the spike question.
  - Both samply and Tracy — rejected as unnecessary: the existing marker points already cover the "where is time spent" question via histograms. Tracy adds the chronological view. samply would be redundant.
- **Consequences:** Tracy client lib added as an optional dependency behind a feature flag. Tracy viewer is an external tool (run separately). Non-Tracy builds have zero overhead (cfg-gated).

## DD-003: Trait-based backend dispatch (not cfg-gated struct)
- **Date:** 2026-08-22
- **Phase:** Phase 1
- **Status:** Superseded by DD-005 (two traits), DD-007 (handle caching)
- **Context:** The profiling abstraction needs to support multiple backends (metrics always-on, Tracy opt-in) with backend-specific state. Two approaches considered: a single struct with `#[cfg]`-gated fields (wezterm's idiom for optional features like dhat), or a trait with per-backend impls.
- **Decision:** Use a `ZoneBackend` trait. Each backend is a type with its own fields, implementing `ZoneBackend` (begin/elapsed) and `Drop` (end/recording). The guard is eliminated entirely — the macro returns the backend type directly. Composition of multiple backends uses a tuple impl `(A, B)` where `begin` constructs both and each element's `Drop` fires independently. A type alias `Default` swaps between `MetricsZone` and `(MetricsZone, TracyZone)` via `#[cfg(feature = "tracy")]`.
- **Rationale:** Traits are the idiomatic Rust way to abstract over backends. Each backend owns its own state (metrics needs `Instant` + `&'static str`, Tracy needs `Span`) — a trait lets each type carry exactly what it needs without leaking backend-specific fields into a shared struct. The tuple composition pattern is idiomatic Rust for combining behaviors without wrapper types. No cfg on the guard itself, only on the type alias. Monomorphized to zero overhead.
- **Alternatives considered:**
  - Cfg-gated concrete struct with backend-specific fields behind `#[cfg(feature = "tracy")]` — matches wezterm's dhat pattern but couples all backend state into one struct, requiring cfg on individual fields. Less clean when backends have divergent state shapes.
  - Cfg-gated enum (wezterm-ssh's `SessionWrap` pattern) — rejected: that pattern is for mutually-exclusive backends chosen at runtime. Our backends are additive (metrics + Tracy fire simultaneously), not exclusive.
- **Consequences:** Superseded by DD-005 (split into zone vs value/counter traits) and DD-007 (handle caching). The trait-composition idea survives; the single-trait surface did not.

## DD-004: Two-arm macro for bind vs fire-and-forget
- **Date:** 2026-08-22
- **Phase:** Phase 1
- **Status:** Accepted
- **Context:** The `profile_zone!` macro needs to support two usage patterns: fire-and-forget (most sites, guard dropped at scope end) and bind (the `paint.rs:115` site, which calls `elapsed()` before drop). A bare `expr;` statement in Rust drops the temporary at end of statement, not end of enclosing scope — so fire-and-forget still needs `let _zone =` inside the macro.
- **Decision:** Two macro arms: `profile_zone!("name")` for fire-and-forget (uses `let _zone =` internally), and `profile_zone!("name", var)` for bind (uses `let var =` so caller can call `elapsed()`). Rust has no stable way to synthesize unique identifier names inside macros without the `paste` crate (an extra dep).
- **Rationale:** The macro is justified by precedent — `scopeguard::guard!`, `tracing::info_span!`, and `metrics::histogram!` itself are all macros that create RAII guards. Replacing `metrics::histogram!("name").record(start.elapsed())` (a macro call) with `profile_zone!("name")` (a macro call) is lateral, not a new pattern. The `let _zone =` binding carries no information — it's just a vehicle for Drop. Hiding it is exactly what `scopeguard` does.
- **Alternatives considered:**
  - `paste` crate for hygienic unique names — rejected: adding a dep purely for syntax sugar is overkill. The bind arm is only used once.
  - No macro, plain constructor call (`let _zone = DefaultZone::begin("name")`) — rejected: more verbose at 13 call sites.
- **Consequences:** Two arms in the macro. The bind arm is used once (paint.rs). If more sites need `elapsed()` later, the arm is already there.

## DD-005: Two traits — ProfilingZoneBackend (RAII) and Recorder (fire-and-forget)
- **Date:** 2026-08-22
- **Phase:** Phase 1
- **Status:** Accepted
- **Context:** The profiling abstraction covers three marker kinds: zones (RAII regions), values (point-in-time scalar samples), and counters (accumulating event counts). Zones are structurally different from values/counters — zones are guards with begin/elapsed/Drop; values/counters are fire-and-forget calls with no scope. Cramming both into one trait would force incongruous method shapes.
- **Decision:** Two traits. `ProfilingZoneBackend` (begin/elapsed, RAII guard via `Drop`) for zones. `Recorder` (record/increment) for fire-and-forget values and counters. Zone names stay `profile_zone!`; value/counter names are `profile_value!`/`profile_counter!`.
- **Rationale:** Zones and events are genuinely different shapes. A counter is not a histogram in disguise — `metrics::CounterFn` (stats.rs:125) is an `AtomicUsize` that accumulates via `fetch_add`, stored in a separate `counters` HashMap, and consumed as a raw total, whereas a histogram records a distribution of values. Collapsing them loses the semantic distinction. Two traits map cleanly onto the two shapes.
- **Alternatives considered:**
  - One trait with a single `record` and a flag — rejected: would misrecord counter events as histogram distributions.
  - Collapse value and counter into one method — rejected: they do different things on the metrics backend.
- **Consequences:** The name `Backend` alone became ambiguous with two traits — renamed to `ProfilingZoneBackend` (zone path) and `Recorder` (value/counter path).

## DD-006: Handle-caching backends (not per-call key construction)
- **Date:** 2026-08-22
- **Phase:** Phase 1
- **Status:** Accepted
- **Context:** The `metrics` crate's `histogram!`/`counter!` macros only use a static cached `Key` (via `key_var!`) when the name is a compile-time literal. When the name reaches the macro as an expression (e.g. a field access like `self.name`), it falls back to `Key::from_name`/`from_parts` which heap-allocate a fresh `Key` and label `Vec` per event. Additionally, wezterm's `Stats::register_histogram` (stats.rs:324) takes a mutex + HashMap lookup on every macro invocation. My initial refactor routed the zone name through a `&'static str` field and called `metrics::histogram!(self.name)` in Drop — a field access is an expr, not a literal, so it forced the allocating path on every drop, a regression over the original literal sites.
- **Decision:** Backends resolve the `metrics::Histogram`/`metrics::Counter` handle once and reuse it. `MetricsZone` stores `histogram: metrics::Histogram` (resolved in `begin`), and `Drop` calls `self.histogram.record(...)` — a single virtual dispatch, no key construction, no mutex. Value/counter backend uses a static handle cache (e.g. `LazyLock`) keyed by name, resolving each `metrics` handle once and reusing it.
- **Rationale:** `metrics::Histogram::record` (handles.rs:142) is a single virtual dispatch through a stored `Arc`; it never needs the key again. `Histogram::from_arc` wraps the `Arc` already returned by `register_histogram`. So resolving once and holding the handle is strictly cheaper than re-invoking the macro per event — it skips both the key allocation and the mutex lock. This is the "work with handles/hashes, not strings" design.
- **Alternatives considered:**
  - Keep calling `metrics::histogram!(name)` in Drop with a field-access name — rejected: hits the non-literal allocating path, a regression.
  - Store the raw `&'static str` and rely on the crate's literal fast path — impossible once the name passes through a struct field.
- **Consequences:** Zone/value/counter backends must hold `metrics` handles, not names. Resolving the handle costs one mutex + HashMap lookup at `begin`/first-use; recording is a single dispatch afterward.

## DD-007: Handle-cache mechanism is macro-level static, not recorder-held map
- **Date:** 2026-08-22
- **Phase:** Phase 2
- **Status:** Accepted (recorder deferred by DD-008; principle recorded for when the recorder lands)
- **Context:** Once backends resolve `metrics` handles (DD-006), the value/counter recorder needs somewhere to cache them. A stateless unit struct has nowhere to store per-(name,labels) handles.
- **Decision:** Caching happens at the macro level via a `static HANDLE: LazyLock<...>` per call site, resolved once on first use. The recorder receives the pre-resolved handle, not a name. No map, no mutex after first init.
- **Rationale:** All wezterm value/counter label values are `stringify!($token)`, which is a fixed `&'static str` per macro-expansion site, so the (name,labels) tuple is fully known at compile time per call site — a per-call-site `static` is sound. A recorder-held `Mutex<HashMap<...>>` would re-introduce a per-call lock+lookup, the same class of cost DD-006 eliminated from zones.
- **Consequences:** The trait takes a pre-resolved handle (associated types). This couples resolution into the macro/backend, not the recorder. Deferred with the recorder (DD-008).
- **Alternatives considered:**
  - Recorder holds `Mutex<HashMap<(name,labels), Handle>>` — rejected: per-call mutex+lookup reintroduces the cost DD-006 removed.
  - Single enum handle type (`Histogram | Counter`) — rejected (DD-008 discussion): forces a `match`/panic path on every record/increment when call sites are statically one kind.
  - Two associated handle types (`ValueHandle`, `CounterHandle`) — chosen: each macro site is either a value or a counter, never both, so each static is statically the right type; no enum.

## DD-008: Value/counter recorder cut from scope (deferred)
- **Date:** 2026-08-22
- **Phase:** Phase 2
- **Status:** Accepted
- **Context:** The value/counter recorder design kept expanding (labels, handle types, label value types, backend-agnostic label type). Reached the labeled rpc/pdu sites and realized the `metrics` label model (multi-dimension key-value) and Tracy's flat-plot model are fundamentally different, and labels are a migration-time concern, not a crate-time one.
- **Decision:** Cut the entire recorder (`ProfilingRecorder`, `MetricsRecorder`, `profile_value!`, `profile_counter!`) out of current scope and rip it from `wezterm-profiling/src/lib.rs`. The crate is zones-only. Recorder + labels are deferred until the value/counter migration phase (Phase 4), when the concrete sites are in front of us.
- **Rationale:** YAGNI. Zones are the point of the whole effort (frame-level spike analysis); values/counters are secondary. Designing the recorder and label threading before the actual migration sites are being touched was speculative — every label/type question got harder to answer in the abstract. Deferring lets the real call sites drive the design.
- **Alternatives considered:**
  - Land a label-less recorder now — rejected: would force rework when labels are needed at migration.
  - Design the full label-threaded recorder now — rejected: the design questions (DD-007) ballooned with no immediate consumer.
- **Consequences:** Phase 2 is effectively empty of new work; the crate is zones-only. The next real work is Phase 3 (migrate the 13 duration sites). The recorder design must be revisited fresh before Phase 4, informed by DD-006/DD-007.

## DD-011: Value/counter migration (Phase 4) cut from scope
- **Date:** 2026-08-22
- **Phase:** Phase 4
- **Status:** Accepted
  - **Context:** Phase 4 (migrate the 26 value/counter sites) depends on the recorder design that DD-008 already deferred. The value/counter markers don't serve the Tracy zone timeline — they're point-in-time scalar samples and accumulating counts, not temporal regions. The whole point of the effort is frame-level spike analysis (DD-002), which zones deliver. Values/counters are secondary.
  - **Decision:** Cut Phase 4 from scope, same YAGNI reasoning as DD-008. The crate stays zones-only. Values/counters keep their raw `metrics::histogram!`/`metrics::counter!` calls as-is.
  - **Rationale:** Designing and migrating a recorder for 26 sites that don't serve the spike-analysis goal is speculative work. If a concrete need arrives (e.g. Tracy plots become desirable), the deferred DD-006/DD-007/DD-008 design can be picked up then, informed by real sites.
  - **Consequences:** Phase 4 is empty. Next real work is Phase 5 (Tracy backend). The recorder design (DD-007) stays on the shelf.

## DD-009: No inherent `elapsed` on `MetricsZone`; trait import at call sites
- **Date:** 2026-08-22
- **Phase:** Phase 3
- **Status:** Accepted
- **Context:** During the zone migration, bind-arm sites that call `_zone.elapsed()` failed to compile because `elapsed` is a trait method and `ProfilingZoneBackend` wasn't in scope at the consumer. Two fixes were on the table: add `use wezterm_profiling::ProfilingZoneBackend;` at each consumer file, or add an inherent `elapsed` on `MetricsZone` (inherent methods win over trait methods in name resolution, so no import needed).
- **Decision:** No inherent method. Add the trait import at the 4 consumer files that call `.elapsed()` (parser.rs, render/mod.rs, render/pane.rs, render/paint.rs). The trait's `elapsed` is the single implementation; the tuple `(A,B)` impl delegates to it.
- **Rationale:** An inherent `elapsed` would duplicate the trait method purely to dodge a one-line import. The trait's `elapsed` default (`Duration::ZERO`) would become dead for `MetricsZone`, and two bodies would need to stay in sync. Importing a trait to call its methods is the idiomatic Rust pattern — every trait method works this way. The one-line `use` is the honest price of the trait-based design (DD-003). Fully-qualified call syntax (`<T as Trait>::method(&v)`) could also avoid the import but reads terribly at hand-written call sites and was rejected for readability.
- **Alternatives considered:**
  - Inherent `elapsed` on `MetricsZone` — initially added, then **reversed** as redundant duplication.
  - Fully-qualified `<ProfilingZone as ProfilingZoneBackend>::elapsed(&_zone)` at call sites — works without import but verbose and unreadable; suitable for macro expansions (see DD-010), not hand-written code.
- **Consequences:** Consumer files calling `.elapsed()` carry a one-line trait import. The macro path is exempt (DD-010). The trait remains the single source of truth for `elapsed`.

## DD-010: Macro uses fully-qualified trait syntax to avoid consumer import
- **Date:** 2026-08-22
- **Phase:** Phase 3
- **Status:** Accepted
- **Context:** The `profile_zone!` macro expands to a `begin` call. `begin` is a trait method, so a plain `ProfilingZone::begin(name)` in the expansion would require every consumer of the macro to import `ProfilingZoneBackend` — a poor ergonomics tax for a macro that's meant to be a one-liner.
- **Decision:** The macro emits `<$crate::ProfilingZone as $crate::ProfilingZoneBackend>::begin($name)` — fully-qualified syntax naming both the type and the trait via `$crate`-relative paths. Consumers need no trait import to use the macro.
- **Rationale:** Macro hygiene means the macro can't assume the trait is in scope at the expansion site. Fully-qualified syntax is the standard Rust solution and is readable enough inside a macro body (unlike at hand-written call sites, per DD-009). The `$crate` prefix ensures the paths resolve regardless of the consumer's module.
- **Alternatives considered:**
  - Plain `ProfilingZone::begin(name)` and require the import — rejected: ergonomic tax on every consumer for a macro meant to be trivial to call.
- **Consequences:** The macro is self-contained (no consumer import for `begin`). The `.elapsed()` method calls at bind-arm sites still need the import (DD-009) — that's unavoidable for method-call syntax and accepted as the idiomatic price.