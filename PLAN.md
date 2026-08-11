# Plan: Ghostty-Style GLSL Shader Support for Wezterm

Load and render Ghostty-style GLSL shaders (ShaderToy `mainImage` format) in Wezterm via the existing `custom_shaders` config, normalizing to WGSL via naga. V1 targets WebGpu only; OpenGL backend support deferred to a later phase.

**Created:** 2026-07-03

## Phases

<!--
Each phase = one commit. Vertical slices preferred.
Mark complete by checking the box and filling in Commit SHA.
Only modify this file on phase completion or plan changes.
-->

- [x] **Phase 1: GLSL Import Pipeline**
  - **Goal:** Normalize Ghostty/shadertoy GLSL shaders into WGSL via naga, integrate with existing WebGpu post-process pipeline.
  - **Deliverable:** (1) Config layer: `ShaderPathBuf` / `ImportedShaderPathBuf` / `GhosttyPathBuf` types with custom `FromDynamic` for polymorphic deserialization. (2) `shader_import.rs` module: `import_shader(&ImportedShaderPathBuf) -> Result<String, ShaderImportError>` — reads GLSL, injects `#define` uniform aliases, parses via `naga::front::glsl`, emits WGSL via `naga::back::wgsl`. (3) Pipeline integration: `compile_postprocess_shader` routes `ShaderPathBuf` variants, `PostProcessState::new` and file watching updated.
  - **Exit criteria:** A wezterm config with `custom_shaders = { { path = "crt.glsl", format = "Ghostty" } }` loads, cross-compiles to WGSL, and renders the CRT effect on the WebGpu backend. V1 uniforms: `iResolution`, `iTime`, `iTimeDelta`, `iFrame`, `iChannel0` only.
  - **Commit:** <SHA filled in on completion>

- [x] **Phase 2: Split Vertex and Fragment Shader Modules**
  - **Goal:** Decouple the vertex and fragment shader stages so each is its own independently-compiled module. Applies to native WGSL shaders too, so `ResolvedShader` and the pipeline know how to wire up separate VS/FS.
  - **Deliverable:** (1) `ResolvedShader` carries distinct vertex and fragment sources (e.g. `vertex: String`, `fragment: String`) rather than one concatenated blob. (2) Imported formats emit a pair of WGSL files — vertex + fragment — as independent naga modules. (3) `compile_postprocess_shader` creates two `ShaderModule`s (one per stage) and passes them to `VertexState`/`FragmentState` respectively. (4) Resource bindings are declared by the author in each stage; the pipeline layout is the shared contract, wgpu validates VS/FS binding agreement at pipeline creation.
  - **Exit criteria:** Native WGSL shaders and imported ghostty shaders both render with separate VS and FS modules. No module merging, handle remapping, or string concat between stages.
  - **Commit:** 8824471f3

- [x] **Phase 3: Testing & Validation**
  - **Goal:** Validate against real Ghostty shaders and harden error handling.
  - **Deliverable:** Vendor 6 shaders from https://github.com/0xhckr/ghostty-shaders (crt.glsl, bloom.glsl, dither.glsl, negative.glsl, vhs.glsl, starfield.glsl) as compile-time test fixtures; `compile_ghostty` split from `import_ghostty` so tests call the cross-compile core directly with `include_str!`'d sources; 6 tests asserting each shader imports end-to-end (glslang→SPIR-V→naga→WGSL). Full-path render test (`test_imported_shader_actually_renders`) takes `negative.glsl` through the real loading path (`resolve_shader`→`import_shader`→`import_ghostty`→`compile_ghostty`→`compile_postprocess_shader`→GPU render) and asserts pixel output. Compilation errors logged with graceful fallback; validation at shader load, not in render loop.
  - **Exit criteria:** Multiple shaders from the ghostty-shaders repo compile and render correctly on WebGpu; malformed shaders degrade gracefully.
  - **Commit:** 549d91fff

- [ ] **Phase 4: Cursor Uniform Support**
  - **Goal:** Extend the uniform set beyond V1 basics (DD-001) to expose Ghostty's cursor uniforms — position trio (`iCurrentCursor`, `iPreviousCursor`, `iTimeCursorChange`) and color pair (`iCurrentCursorColor`, `iPreviousCursorColor`) — unblocking cursor-reactive shaders and enabling cursor trails/lightning/glow effects.
  - **Deliverable:** (1) Extend `PostProcessUniform` (`webgpu.rs:27`) and the native WGSL preamble (`POSTPROCESS_PREAMBLE`) with 5 cursor fields: `current_cursor`/`previous_cursor`/`current_cursor_color`/`previous_cursor_color` (vec4 each) + `cursor_change_time` (f32), growing the struct from 32 to 112 bytes with std140 alignment; update the size assert at `webgpu.rs:1173`. (2) Extend `replace_globals_struct` (`shader_import.rs:187`) to emit the expanded struct in naga IR with Ghostty field names (`iCurrentCursor` etc.), matching the host buffer layout — no patch file changes needed (GLSL prefix already declares all cursor uniforms). (3) Add `CursorRenderState` struct (per-pane, mirroring `PrevCursorPos`'s change-detect mechanism) on `PaneState`, updated in `paint_pane` for every rendered pane with the cursor's pixel rect (reusing `update_text_cursor`'s geometry logic) + color (`palette.cursor_bg`); change detection fires on position OR color change, recording `cursor_change_time`. (4) Wire the uniform write in `call_draw_webgpu` (`draw.rs:199`) to read the active pane's `CursorRenderState`; share a single `time` value between `paint_pane`'s change-detection and the uniform's `time`/`cursor_change_time` for consistency. (5) Vendor 3 cursor shader fixtures (`cursor_blaze.glsl`, `cursor_lightning.glsl`, `in-game-crt-cursor.glsl`) + compile tests + a full-path render test asserting the shader renders without crash with the expanded uniform.
  - **Exit criteria:** `cursor_blaze.glsl`, `cursor_lightning.glsl`, and `in-game-crt-cursor.glsl` import end-to-end and render on WebGpu. The uniform struct grows to 112 bytes (assert passes). Existing 42 wezterm-gui + 15 config tests unaffected.
  - **Commit:** <SHA filled in on completion>

- [ ] **Phase 5: Decouple Struct Definitions from Dual Maintenance** (deferred)
  - **Goal:** Eliminate the manual lockstep between `PostProcessUniform` (host struct), `POSTPROCESS_PREAMBLE` (native WGSL), and `replace_globals_struct` (naga IR) so adding a uniform doesn't require updating three locations by hand.
  - **Deliverable:** TBD — investigate single-source-of-truth approaches (e.g., codegen from a shared schema, macro-driven struct + WGSL emission, or a build-time generator).
  - **Exit criteria:** A new uniform field requires editing exactly one location; the host struct, WGSL preamble, and naga IR struct all derive from it automatically.
  - **Commit:** <SHA filled in on completion>