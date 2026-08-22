# Working Memory: Profiling Abstraction for Wezterm

<!--
Append-only chronological log of what was done.
- One entry per meaningful unit of work.
- "What" only — "why" goes in DESIGN_DECISIONS.md.
- Grep by date or phase; do not load the whole file.

Entry shape:
## [YYYY-MM-DD HH:MM] Phase <N>: <Short action title>
- **Did:** <concrete actions>
- **Files:** <file_path:line refs>
- **Refs:** <PLAN.md#phase-N, DD-NNN>
- **Next:** <optional breadcrumb>
-->

## [2026-08-22 ~14:00] Phase 1: Design investigation & planning
- **Did:** Investigated wezterm codebase idioms for feature-gated backend dispatch. Found `wezterm-ssh` uses cfg-gated enum variants (mutually exclusive backends) and dhat uses inline `#[cfg(feature)]` statements (additive optional instrumentation). Determined our case is additive (metrics + Tracy simultaneously), not mutually exclusive. Confirmed `tracy_client` API: `span!` for zones, `plot!` for values (no separate counter primitive — counters map to plots). Designed `ZoneBackend` trait with per-backend structs + tuple composition. Designed two-arm `profile_zone!` macro (fire-and-forget + bind). Recorded DD-003 (trait dispatch), DD-004 (two-arm macro). Updated PLAN.md phases.
- **Files:** wezterm-ssh/src/sessionwrap.rs, wezterm-ssh/src/filewrap.rs, wezterm-gui/Cargo.toml, wezterm-gui/src/main.rs:830, Cargo.toml:140
- **Refs:** PLAN.md#phase-1, DD-003, DD-004
- **Next:** Implement Phase 1 — create `wezterm-profiling` crate skeleton (trait, MetricsZone, macro)

## [2026-08-22 ~14:30] Phase 1: Implement wezterm-profiling crate
- **Did:** Created `wezterm-profiling` workspace crate. `lib.rs` defines `ZoneBackend` trait (begin/elapsed), `MetricsZone` (fields start+name, records to `metrics::histogram!` on Drop), tuple impl `(A,B)`, type alias `DefaultZone = MetricsZone`, and two-arm `profile_zone!` macro (fire-and-forget + bind). Added `metrics` dep. Registered in workspace `members` and `[workspace.dependencies]`. Added 2 unit tests (elapsed measurement, tuple delegation). `cargo check` and `cargo test` pass. Crate compiles standalone with zero behavior change elsewhere.
- **Files:** wezterm-profiling/Cargo.toml, wezterm-profiling/src/lib.rs, Cargo.toml:13,258
- **Refs:** PLAN.md#phase-1, DD-003, DD-004
- **Next:** Phase 2 — migrate the 13 duration marker sites

## [2026-08-22 ~15:00] Phase 1: Backend trait refactor + handle-caching fix
- **Did:** Renamed trait `ZoneBackend` → `Backend` → `ProfilingZoneBackend` to disambiguate as the two-trait design emerged. Split into two traits: `ProfilingZoneBackend` (RAII zones: begin/elapsed/Drop) and `Recorder` (fire-and-forget values/counters: record/increment) — DD-005. Discovered metrics crate only uses static cached `Key` for literal names; non-literal `expr` names (like a field access) fall to a heap-allocating `Key::from_name`/`from_parts` path. This meant routing the zone name through a `&'static str` field and calling `metrics::histogram!(self.name)` in Drop was a regression — it forced per-drop key allocation. Fixed by storing the resolved `metrics::Histogram` handle in `MetricsZone` (resolved once in `begin`) and calling `self.histogram.record(...)` in Drop — a single virtual dispatch, no key construction, no mutex. Value/counter backend will use a static handle cache (DD-006). Updated PLAN.md: inserted a value/counter phase (Phase 2), renumbered migration/Tracy/verify/profile/optimize to Phases 3-8.
- **Files:** wezterm-profiling/src/lib.rs, PLAN.md, DESIGN_DECISIONS.md
- **Refs:** PLAN.md#phase-1, DD-004, DD-005, DD-006
- **Next:** Design + implement `Recorder` trait and value/counter macros (labels question open) — Phase 2