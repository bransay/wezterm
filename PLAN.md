# Plan: Profiling Abstraction for Wezterm

Introduce a RAII-based profiling zone abstraction over wezterm's existing `metrics::histogram!` timing markers, with a Tracy backend behind a feature flag, to enable frame-level perf analysis and spike identification.

**Created:** 2026-08-22

## Phases

<!--
Each phase = one commit. Vertical slices preferred.
Mark complete by checking the box and filling in Commit SHA.
Only modify this file on phase completion or plan changes.
-->

- [x] **Phase 1: Create the profiling crate**
  - **Goal:** A new `profiling` workspace crate with a RAII guard struct + `profile_zone!` macro, metrics backend only, preserving current behavior exactly.
  - **Deliverable:** Compiling crate, ready to consume from marker sites.
  - **Exit criteria:** Crate compiles, guard records to `metrics::histogram!` on Drop, no behavior change anywhere yet.
  - **Commit:** `e85ccd019`

- [ ] **Phase 2: Add value/counter backends to the profiling crate**
  - **Goal:** Extend the crate with a `Recorder` trait (record/increment) and a `MetricsRecorder` backend handling fire-and-forget value and counter markers, with handle caching.
  - **Deliverable:** `profile_value!` and `profile_counter!` macros with metrics backend.
  - **Exit criteria:** Value/counter macros compile, backends cache handles, no behavior change elsewhere yet.
  - **Commit:** <SHA filled in on completion>

- [ ] **Phase 3: Migrate the 13 duration marker sites**
  - **Goal:** Replace every `let start = Instant::now(); ... metrics::histogram!("name").record(start.elapsed())` with `profile_zone!("name")` across all 13 duration sites.
  - **Deliverable:** All duration sites routed through the abstraction.
  - **Exit criteria:** Histograms record identical data before/after (diff stderr dumps). No `metrics::histogram!().record(start.elapsed())` patterns remain in the codebase.
  - **Commit:** <SHA filled in on completion>

- [ ] **Phase 4: Migrate the 26 value/counter marker sites**
  - **Goal:** Replace `metrics::histogram!().record(value)` and `metrics::counter!().increment(n)` with `profile_value!`/`profile_counter!`.
  - **Deliverable:** All value/counter sites routed through the abstraction.
  - **Exit criteria:** Value/counter data identical before/after. No raw `metrics::histogram!`/`metrics::counter!` timing/value calls remain.
  - **Commit:** <SHA filled in on completion>

- [ ] **Phase 5: Add Tracy backend**
  - **Goal:** Add `tracy_client` as an optional dep behind a `tracy` feature flag. Guard dual-emits: metrics always, Tracy when feature enabled. Wire the feature into `wezterm-gui/Cargo.toml`.
  - **Deliverable:** `cargo build --features tracy` produces a binary Tracy can connect to.
  - **Exit criteria:** Tracy client lib compiles behind feature flag, guard emits Tracy zones when feature is on, zero overhead when off (cfg-gated).
  - **Commit:** <SHA filled in on completion>

- [ ] **Phase 6: Verify Tracy captures the zone hierarchy**
  - **Goal:** Build with Tracy feature, run wezterm, connect Tracy profiler, confirm zone nesting (frame → pane → line → cluster) appears on the timeline.
  - **Deliverable:** Screenshot/recording of Tracy showing wezterm zones.
  - **Exit criteria:** Zone hierarchy visible in Tracy, zones correspond to real wezterm work, frame markers present.
  - **Commit:** <SHA filled in on completion>

- [ ] **Phase 7: Profile a real workload**
  - **Goal:** Run wezterm under Tracy with representative load (kitten benchmark, large scrollback, image protocols). Identify spikes and their causes.
  - **Deliverable:** Actionable list of perf hotspots with zone-level evidence.
  - **Exit criteria:** At least 3 identified spikes with root-cause analysis from Tracy zones.
  - **Commit:** <SHA filled in on completion>

- [ ] **Phase 8: Optimize**
  - **Goal:** One commit per optimization, guided by Tracy findings. Measurable frame time improvement.
  - **Deliverable:** Shipped optimization PRs.
  - **Exit criteria:** Measurable frame time improvement on the workloads identified in Phase 7.
  - **Commit:** <SHA filled in on completion>