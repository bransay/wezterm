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

- [ ] **Phase 3: Testing & Validation**
  - **Goal:** Validate against real Ghostty shaders and harden error handling.
  - **Deliverable:** Test against https://github.com/0xhckr/ghostty-shaders (crt.glsl, bettercrt.glsl, bloom.glsl, dither.glsl, etc.); compilation errors logged with graceful fallback; validation at shader load, not in render loop.
  - **Exit criteria:** Multiple shaders from the ghostty-shaders repo compile and render correctly on WebGpu; malformed shaders degrade gracefully.
  - **Commit:** <SHA filled in on completion>