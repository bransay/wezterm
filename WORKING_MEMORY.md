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

## [2026-08-22] Phase 2: Recorder scope reconsidered and cut
- **Did:** Reconsidered the value/counter recorder after investigation. Investigated `metrics` crate internals: `Key::from_static_parts` is `const` so literal names build a static `Key` at compile time, but `Stats::register_histogram` (stats.rs:324) still does a mutex+hashmap lookup per call — the per-call cost handle-caching would eliminate. Found `lfucache/src/lib.rs:245` uses `metrics::histogram!(self.hit)` — a runtime `&'static str` field, not a literal, so it hits the allocating `Key::from_name` path (genuinely hot). Labeled sites (codec/src/lib.rs:353, client.rs:86) use `stringify!($name)` inside macros, which expands to literals → static key path preserved. Explored labels: they're the metrics crate's multi-dimension data model (`"method" => m`), wezterm never uses more than one label per site, and Tracy has no label primitive (flat plots only, spans have emit_value/emit_text). Discussed trait shapes: two associated handles vs one enum handle, label value types, handle-cache mechanism (macro-level static `LazyLock` won over recorder-held map). **Decision: cut the entire recorder out of scope for now** — ripped `ProfilingRecorder`, `MetricsRecorder`, `DefaultRecorder`, `profile_value!`, `profile_counter!` out of `wezterm-profiling/src/lib.rs`. Crate is zones-only again. Recorder + labels deferred until value/counter migration.
- **Files:** wezterm-profiling/src/lib.rs, ~/.cargo/registry/.../metrics-0.23.1/src/{key.rs,handles.rs,macros.rs}, wezterm-gui/src/stats.rs:324, lfucache/src/lib.rs:245, codec/src/lib.rs:353, wezterm-client/src/client.rs:86
- **Refs:** PLAN.md#phase-2, DD-006
- **Next:** Phase 3 — migrate the 13 duration (zone) marker sites to `profile_zone!`

## [2026-08-22 ~14:45] Phase 3: Migrate 12 duration marker sites to `profile_zone!`
- **Did:** Migrated all 12 duration sites (count was 12, not 13) to `profile_zone!`. Wired `wezterm-profiling.workspace = true` into mux, wezterm-font, wezterm-gui, window Cargo.tomls. Site-by-site:
  - **Fire-and-forget** (guard drops at scope end, no elapsed read): `shape.harfbuzz` (harfbuzz.rs:584), `quad_buffer_apply` (quad.rs:326), `render_screen_line` (screen_line.rs), `window.atlas.allocate.latency` (atlas.rs:96).
  - **Bind arm + explicit `drop`** (elapsed read for a trace/log, then early terminate to match original emit point): `gui.paint.impl` (paint.rs — bind `_paint_zone` at top, `drop(_paint_zone)` after `call_draw` before the rate line; `start` kept for fps `duration_since` math, `gui.paint.impl.rate` left raw as deferred value site), `quad.map` (paint.rs — bind `_quad_zone`, `drop(_quad_zone)` right after the trace, matching original emit point before the rest of `paint_pass`).
  - **Bind arm, guard scoped to block** (elapsed read for a log, negligible divergence at function end): `cached_cluster_shape` (render/mod.rs:788 `_shape_zone`), `paint_pane.lines` (pane.rs:297 `_lines_zone`), `font.compute.codepoint.coverage` (parser.rs:543 `_coverage_zone`, scoped inside `if cov.is_empty()` block).
  - **mux `send_actions_to_mux`** (mux/src/lib.rs:123): bind `_latency_zone` at function top (captures `upgrade()` cost), `drop(_latency_zone)` in Some arm after `perform_actions` (before notify), `std::mem::forget(_latency_zone)` in None arm — parity with original which recorded only on the Some path.
  - **spawn.rs** (`InstrumentedSpawnFunc`): holds `zone: ProfilingZone` (RAII-in-a-struct, `#[allow(dead_code)]` — field exists only for its Drop side-effect, never read so the lint fires), created in `queue_func` (name picked by priority), dropped in `pop_func`. Removed unused `Instant` import; added `use wezterm_profiling::ProfilingZoneBackend`.
- **Crate API change during migration:** Macro now expands with fully-qualified `<$crate::ProfilingZone as $crate::ProfilingZoneBackend>::begin(...)` so consumer call sites don't need the trait in scope for the macro. `.elapsed()` call sites (4 files) require `use wezterm_profiling::ProfilingZoneBackend;` because `.elapsed()` is a trait method (Rust rule: trait must be in scope for method-call syntax). Initially added an inherent `elapsed` on `MetricsZone` to dodge the import — **reversed**: ripped it out as redundant duplication (trait already has `elapsed`; the inherent was dead-for-MetricsZone cruft). Added the trait import at the 4 consumer files instead (parser.rs, render/mod.rs, render/pane.rs, render/paint.rs).
- **Verification:** Full `cargo check` (whole workspace) passes; no unused-import warnings; no raw `record(start.elapsed())` patterns remain.
- **Files:** wezterm-profiling/src/lib.rs (macro FQN, inherent-elapsed add+revert), mux/src/lib.rs:123, wezterm-gui/src/termwindow/render/{paint.rs,mod.rs,pane.rs,screen_line.rs}, wezterm-gui/src/quad.rs, wezterm-font/src/{parser.rs,shaper/harfbuzz.rs}, window/src/{spawn.rs,bitmaps/atlas.rs}, {mux,wezterm-font,wezterm-gui,window}/Cargo.toml
- **Refs:** PLAN.md#phase-3, DD-001, DD-003, DD-004, DD-006
- **Next:** Phase 5 (Tracy backend) — Phase 4 (value/counter migration) cut from scope as YAGNI, same as Phase 2/DD-008