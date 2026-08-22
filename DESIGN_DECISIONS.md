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