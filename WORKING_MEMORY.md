# Working Memory: Ghostty-Style GLSL Shader Support for Wezterm

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

## [2026-07-03 21:15] Phase 1: Design-focused implementation — Steps 1-4 complete
- **Did:** Oriented on PLAN.md Phase 1 and implementation-plan.md from -plan branch. Investigated existing WGSL pipeline (PostProcessState, PostProcessUniform, preamble, render loop, config handling), naga availability (dev-dep only, wgpu 25 transitive), Ghostty uniform set (from docs), ghostty-shaders repo (CRT/bloom/dither use only iResolution/iTime/iChannel0). Asked and answered 7 design questions with Bryan. Recorded DD-001 through DD-009. Proposed implementation plan in 3 chunks (config layer, shader import module, pipeline integration), all approved.
- **Files:** wezterm-gui/src/termwindow/webgpu.rs:17-89, wezterm-gui/src/termwindow/mod.rs:1854-1909, wezterm-gui/src/termwindow/render/draw.rs:52-269, config/src/config.rs:562-567, config/src/config.rs:1339-1344, config/src/lib.rs:636-640, config/src/frontend.rs:5-10, config/src/background.rs:6-46
- **Refs:** PLAN.md#phase-1, DD-001 through DD-009
- **Next:** Step 5 — implement Phase 1

## [2026-07-03 21:45] Phase 1: Implementation plan approved
- **Did:** Revised PLAN.md phases to reflect V1 decisions (WebGpu only, config-driven format detection, 4 phases instead of 5). Updated header description. Plan approved by Bryan.
- **Files:** PLAN.md
- **Refs:** PLAN.md#phase-1, DD-001 through DD-009
- **Next:** Step 5 — implement Phase 1 (config layer types, shader_import module, pipeline integration)

## [2026-07-03 22:30] Phase 1: Implementation complete
- **Did:** Implemented all three chunks of Phase 1. (1) Config layer: new `config/src/shader.rs` with `ShaderPathBuf`, `ImportedShaderPathBuf`, `GhosttyPathBuf` types, custom `FromDynamic`/`ToDynamic` for polymorphic deserialization (bare string → Native, tagged object → Imported). Updated `config.rs` custom_shaders field type and path resolution. Updated `lib.rs` file watching. (2) Added naga as runtime dep with `glsl-in` + `wgsl-out` features. (3) New `shader_import.rs` module: `import_shader(&ImportedShaderPathBuf) -> Result<String, ShaderImportError>` — GLSL preamble with shadertoy uniform declarations, naga GLSL parse with `iChannel0` define substitution, validation, WGSL emit. (4) Pipeline integration: `compile_postprocess_shader` routes `ShaderPathBuf` variants (Native → existing path, Imported → import_shader), `PostProcessState::new` accepts `&[ShaderPathBuf]`, new `POSTPROCESS_VERTEX_SHADER` for imported shaders, fragment entry point is dynamic (`fs_postprocess` for native, `main` for imported). All tests pass: 6 config tests, 5 shader_import tests, existing webgpu tests.
- **Files:** config/src/shader.rs (new), config/src/lib.rs:37-38,57-58, config/src/config.rs:563-569,1342-1349, wezterm-gui/Cargo.toml:67, wezterm-gui/src/termwindow/shader_import.rs (new), wezterm-gui/src/termwindow/mod.rs:84, wezterm-gui/src/termwindow/webgpu.rs:3,56-107,137-201,253,1385-1510
- **Refs:** PLAN.md#phase-1, DD-001 through DD-010
- **Next:** Phase 2 — Testing & Validation with real ghostty-shaders repo

## [2026-08-08 14:00] Phase 1: DD-011 pivot — glslang→SPIR-V→naga pipeline rewrite
- **Did:** Switched from naga GLSL-in to glslang→SPIR-V→naga spv-in pipeline (DD-011). Added `glslang = "0.8"` runtime dep, changed naga features to `spv-in` + `wgsl-out`. Rewrote `shader_import.rs` with glslang compile → SPIR-V → naga parse → IR manipulation → WGSL emit. Added `ResolvedShader` newtype and `resolve_shader` funnel in `webgpu.rs`. Simplified `compile_postprocess_shader` to take `&ResolvedShader`. Hit `InvalidId(49)` error — naga's SPV frontend can't handle `OpLoad` of `OpTypeSampledImage` (combined image sampler). Diagnosed via SPIR-V dump test: glslang emits `OpLoad` of combined `sampler2D` variable, but naga expects separate `OpTypeImage` + `OpTypeSampler` combined via `OpSampledImage`.
- **Files:** wezterm-gui/Cargo.toml:56,69, wezterm-gui/src/termwindow/shader_import.rs (rewritten), wezterm-gui/src/termwindow/webgpu.rs:96-103,116-161
- **Refs:** PLAN.md#phase-1, DD-011
- **Next:** Resolve combined sampler issue

## [2026-08-08 15:30] Phase 1: DD-012 through DD-014 — patch file + IR redesign
- **Did:** Design-focused implementation session. Diagnosed combined sampler limitation fully. Proposed patch file approach: keep verbatim ghostty prefix, apply build-time `.patch` to split `sampler2D iChannel0` into separate `texture2D` + `sampler` + `#define` macro, move Globals to `set = 1`. Patch matches wezterm's bind group layout exactly (texture: set 0 binding 0, sampler: set 0 binding 1, uniform: set 1 binding 0). IR manipulation reduced to: replace Globals struct (DD-014, build fresh 4-member struct matching `PostProcessUniform`), add vertex shader (DD-013, programmatic naga IR not string concat), rename entry point. Recorded DD-012, DD-013, DD-014. Created patch file `ghostty_shadertoy_prefix.patch`. Started build.rs integration.
- **Files:** wezterm-gui/src/termwindow/shaders/ghostty_shadertoy_prefix.patch (new), DESIGN_DECISIONS.md (DD-012 through DD-014)
- **Refs:** PLAN.md#phase-1, DD-012 through DD-014
- **Next:** Finish build.rs patch integration, implement IR manipulation functions, test

## [2026-08-08 16:30] Phase 1: Implementation complete — patch + IR + tests all passing
- **Did:** Implemented all three chunks. (1) Patch file `ghostty_shadertoy_prefix.patch` created — splits `sampler2D iChannel0` into `texture2D` + `sampler` + `#define` macro, moves Globals to `set = 1, binding = 0`. build.rs applies patch at build time via `patch` command, writes patched file to `OUT_DIR`, `include_str!` picks it up. (2) `shader_import.rs` rewritten: deleted `remap_bindings` (patch handles bindings), deleted `strip_unused_uniforms` (replaced by `replace_globals_struct`), implemented `replace_globals_struct` (builds fresh 4-member struct matching `PostProcessUniform`: iResolution vec2, iTime f32, iTimeDelta f32, iFrame i32), implemented `add_vertex_shader` (constructs `vs_postprocess` fullscreen triangle in naga IR — pre-emitted expressions appended outside emitter, computed expressions inside), `rename_entry_point` kept. (3) Deleted `POSTPROCESS_VERTEX_SHADER` const from `webgpu.rs`, removed `spirv` dev-dep, deleted `test_dump_spirv` debug test. All tests pass: 35 wezterm-gui, 15 config.
- **Files:** wezterm-gui/src/termwindow/shaders/ghostty_shadertoy_prefix.patch (new), wezterm-gui/build.rs:3-7,196-241 (patch_shadertoy_prefix), wezterm-gui/src/termwindow/shader_import.rs (rewritten IR functions), wezterm-gui/src/termwindow/webgpu.rs (deleted POSTPROCESS_VERTEX_SHADER), wezterm-gui/Cargo.toml (removed spirv dev-dep)
- **Refs:** PLAN.md#phase-1, DD-012 through DD-014
- **Next:** Phase 2 — Testing & Validation with real ghostty-shaders repo

## [2026-08-08 17:00] Phase 1: ResolvedShader.path String→PathBuf
- **Did:** Switched `ResolvedShader.path` from `String` to `std::path::PathBuf` for semantic correctness — a path is a path. Constructor now takes `impl Into<PathBuf>`. `compile_postprocess_shader` uses `path.display()` matching original code exactly (minimal diff). Updated call sites: `resolve_shader` passes `path.to_path_buf()`, `shader_import.rs` wraps `PathBuf::from(path_str)`. All 35 wezterm-gui tests pass.
- **Files:** wezterm-gui/src/termwindow/webgpu.rs, wezterm-gui/src/termwindow/shader_import.rs
- **Refs:** DD-014
- **Next:** Phase 2 — Testing & Validation with real ghostty-shaders repo

## [2026-08-08 21:30] Phase 1: DD-015 — vertex shader WGSL source file replaces IR construction (REVERTED)
- **Did:** (Superseded direction — concat flow reverted before Phase 2.) Replaced 200-line `add_vertex_shader` IR construction with a WGSL source + re-parse concat approach. Re-parse flagged as cargo-culting; the concat direction was reverted in favor of separate modules (DD-016). Kept as history.
- **Refs:** DD-015
- **Next:** Superseded by DD-016

## [2026-08-08 22:15] Phase 2: Design — separate vertex+fragment modules (DD-016, DD-017)
- **Did:** Design-focused implementation pass for Phase 2. Concluded separate vertex+fragment modules with user-owned bindings (DD-016) eliminates the merge problem entirely; wgpu validates VS/FS binding agreement at pipeline creation. Recorded DD-016 (proposed) and PLAN.md Phase 2 (Split VS/FS) + renumbered old Phase 2 to Phase 3. Reworked DD-015's concat flow was reverted by Bryan (working tree clean).

## [2026-08-08 23:00] Phase 2: Implemented — ResolvedShader vertex/fragment ShaderSource split
- **Did:** Implemented Phase 2 (DD-016/DD-017). (1) `ResolvedShader` now holds `vertex: Arc<ShaderSource>` + `fragment: Arc<ShaderSource>`; `ShaderSource { source: String, path: PathBuf }` (path is `#[cfg(debug_assertions)]` label). (2) Native path: both members share the same preamble+user source and same path via one shared `Arc` — no preamble split. (3) Imported path: `vertex` = `include_str!("shaders/ghostty_fullscreen_vertex.wgsl")` (recreated static file, fullscreen triangle, never through naga), `fragment` = naga output; `add_vertex_shader` IR construction deleted entirely from `shader_import.rs`. (4) `compile_postprocess_shader` creates two `ShaderModule`s and wires `VertexState`→vertex module, `FragmentState`→fragment module; layout/bindings unchanged. All 35 wezterm-gui tests pass (incl. GPU render tests).

## [2026-08-09 10:24] Phase 2: Completion — committed, post-refinement cleanup
- **Did:** Phase 2 committed as `8824471f3` ("okay vertex shader split out, ghostty import imports vs source"). Post-implementation refinements from review: (1) native path builds `vs_source` Arc directly then clones into `fs_source` (readable aliases, locals are zero-cost). (2) `ShaderSource` no longer needs `Clone` — Arc clone is the refcount bump. (3) `compile_postprocess_shader` module creation DRY'd into an inline `create_module` closure capturing `device` by reference (inlines to zero cost); label/fallback/error derive from a `kind` param producing `<unnamed vertex shader>` / `<unnamed fragment shader>` in release builds. (4) Uniform bind group layout still `VERTEX | FRAGMENT` (unused vertex access kept, no effect). Working tree clean after commit. PLAN.md Phase 2 commit SHA filled in.
- **Files:** wezterm-gui/src/termwindow/webgpu.rs (ResolvedShader Arc split, create_module closure, kind-based labels), wezterm-gui/src/termwindow/shader_import.rs (deleted add_vertex_shader, returns ResolvedShader with Arc sources), wezterm-gui/src/termwindow/shaders/ghostty_fullscreen_vertex.wgsl (new)
- **Refs:** DD-016, DD-017, PLAN.md#phase-2
- **Next:** Phase 3 — Testing & Validation with real ghostty-shaders repo