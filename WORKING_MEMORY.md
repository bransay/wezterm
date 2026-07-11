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