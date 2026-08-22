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