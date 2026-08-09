# Design Decisions: Ghostty-Style GLSL Shader Support for Wezterm

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

## DD-001: V1 uniform scope — ShaderToy basics only
- **Date:** 2026-07-03
- **Phase:** Phase 1
- **Status:** Accepted
- **Context:** Ghostty exposes a large uniform set including terminal-specific ones (cursor position, palette, focus). Need to decide V1 scope.
- **Decision:** V1 supports `iResolution`, `iTime`, `iChannel0` only. `iTimeDelta` and `iFrame` included since they're already free in the existing `PostProcessUniform` struct.
- **Rationale:** V1 goal is something demonstrable. Basic effects (CRT, bloom, dither, VHS, water) only need these. Terminal-specific uniforms (cursor, palette, focus) are V2.
- **Alternatives considered:** Full Ghostty uniform set from day one — rejected as too much lift for V1.
- **Consequences:** Shaders using `iCurrentCursor`, `iPreviousCursor`, `iTimeCursorChange`, `iFocus` etc. won't work until V2.

## DD-002: WebGpu only for V1, OpenGL deferred to V2
- **Date:** 2026-07-03
- **Phase:** Phase 1
- **Status:** Accepted
- **Context:** Implementation plan says both backends from day one. OpenGL is the default frontend but has zero post-process infrastructure (no intermediate texture, no shader chain, no uniform buffer). WebGpu already has all of it.
- **Decision:** V1 targets WebGpu only. OpenGL backend support deferred to V2.
- **Rationale:** Building the OpenGL path is a significant lift — new render-to-texture setup, GLSL shader compilation, uniform binding, ping-pong. V1 should be demonstrable without that cost.
- **Alternatives considered:** Both backends from day one — rejected as too much scope for V1.
- **Consequences:** Users on OpenGL frontend (the default) won't have shader support until V2. WebGpu must be explicitly configured.

## DD-003: naga as runtime dependency
- **Date:** 2026-07-03
- **Phase:** Phase 1
- **Status:** Accepted
- **Context:** naga is currently only a dev-dependency with `wgsl-in` feature, used in tests. GLSL cross-compilation requires `glsl-in` (parse) and `wgsl-out` (emit) at runtime.
- **Decision:** Move naga to a runtime dependency with `glsl-in` and `wgsl-out` features.
- **Rationale:** naga is already pulled transitively by wgpu. Adding the extra features is modest build cost. No feature flag needed — wezterm-gui's existing features are about build/distribution config, not gating user-facing functionality.
- **Alternatives considered:** Feature flag for ghostty shader support — rejected as not idiomatic for this codebase.
- **Consequences:** All wezterm-gui builds will include naga's GLSL frontend and WGSL backend.

## DD-004: No feature flag for shader support
- **Date:** 2026-07-03
- **Phase:** Phase 1
- **Status:** Accepted
- **Context:** Considered gating GLSL/ghostty shader support behind a Cargo feature flag so maintainers could choose whether to bundle it.
- **Decision:** Unconditional — no feature flag.
- **Rationale:** wezterm-gui's existing features (`wayland`, `vendored-fonts`, `distro-defaults`, `dhat-heap`) are about build/distribution, not gating functionality. naga is already transitive via wgpu, so the extra cost is cheap. Flagging GLSL separately from WGSL would create inconsistency.
- **Alternatives considered:** `custom-glsl-shaders = ["naga/glsl-in", "naga/wgsl-out"]` feature — rejected as non-idiomatic.
- **Consequences:** Ghostty shader support is always available when building wezterm-gui.

## DD-005: Polymorphic config via `ShaderPathBuf` enum
- **Date:** 2026-07-03
- **Phase:** Phase 1
- **Status:** Accepted
- **Context:** `custom_shaders` is currently `Vec<PathBuf>` (WGSL only). Need to support both native WGSL and imported ghostty GLSL in the same list. Considered extension-based detection, content sniffing, MIME types, and tagged config objects.
- **Decision:** `custom_shaders` becomes `Vec<ShaderPathBuf>` where `ShaderPathBuf` is an enum: `Native(PathBuf)` for bare paths (WGSL, backwards compatible) and `Ghostty(PathBuf)` for ghostty/shadertoy GLSL. Custom `FromDynamic` impl handles polymorphic deserialization — bare strings → `Native`, tagged objects with `format = "Ghostty"` → `Ghostty`. Follows the `ImageFileSourceWrap` precedent in the codebase.
- **Rationale:** Type system encodes the format — no separate `ShaderFormat` enum needed. Adding a new format = adding a variant. The variant itself carries the format. Idiomatic to wezterm's `FromDynamic` polymorphic deserialization pattern.
- **Alternatives considered:** Content sniffing for `mainImage(` — rejected as unreliable. Extension-based detection — rejected as not data-driven. Separate `ShaderFormat` enum — rejected as redundant when the variant encodes the format.
- **Consequences:** Existing configs with bare paths keep working. New configs can mix native and imported shaders. Config schema is extensible for future shader formats.

## DD-006: Error handling — log and skip, no toast
- **Date:** 2026-07-03
- **Phase:** Phase 1
- **Status:** Accepted
- **Context:** When a GLSL shader fails to compile, need to decide UX. Current WGSL path logs and skips. Wezterm uses toast notifications for fatal/user-facing errors.
- **Decision:** Match existing WGSL pattern — log the error, skip the shader, continue rendering.
- **Rationale:** Shader failures are developer-facing config issues, not user-facing errors. Wezterm reserves toasts for fatal/error-level stuff. Consistent with existing shader error handling.
- **Alternatives considered:** Toast notification on shader failure — rejected as not idiomatic for this class of error.
- **Consequences:** Malformed GLSL shaders silently fail (logged only). User sees no shader effect, not a crash or notification.

## DD-007: Entry point parameterized per format
- **Date:** 2026-07-03
- **Phase:** Phase 1
- **Status:** Accepted
- **Context:** Ghostty shaders define `mainImage`, not `main`. naga's GLSL parser needs an entry point. Could hardcode `mainImage` or parameterize.
- **Decision:** The translation layer parameterizes the entry point name per shader format. Try `mainImage` directly with naga first; fall back to a `main()` shim only if naga requires `main`.
- **Rationale:** YAGNI — avoid source code manipulation if naga can be persuaded to use `mainImage` directly. Parameterization keeps the door open for future formats with different entry points without redesign.
- **Alternatives considered:** Always wrap with `main()` shim — rejected as unnecessary if naga accepts `mainImage`.
- **Consequences:** If naga's GLSL frontend requires `main`, we'll need a shim. Decision deferred to implementation.

## DD-008: ShaderPathBuf nested type hierarchy
- **Date:** 2026-07-03
- **Phase:** Phase 1
- **Status:** Accepted
- **Context:** Need to represent native WGSL paths and imported shader paths in a type-safe, extensible way. Considered flat enum, separate ShaderFormat enum, and polymorphic config objects.
- **Decision:** Nested type hierarchy: `ShaderPathBuf` enum with `Native(PathBuf)` and `Imported(ImportedShaderPathBuf)` variants. `ImportedShaderPathBuf` enum with `Ghostty(GhosttyPathBuf)` variant. `GhosttyPathBuf` is a strong-typed newtype around `PathBuf`. Import module (`shader_import.rs`) accepts `ImportedShaderPathBuf`; caller routes `Native` as noop.
- **Rationale:** Type graph is self-documenting. `ShaderPathBuf` tells you native vs imported. `ImportedShaderPathBuf` tells you it needs translation. `GhosttyPathBuf` tells you the convention. No runtime validation, no format strings. Adding a format = new newtype + new variant. Idiomatic Rust newtype pattern, analogous to C++ strong typedef.
- **Alternatives considered:** Flat enum `Native(PathBuf)` / `Ghostty(PathBuf)` — rejected as less scalable. Separate `ShaderFormat` enum — rejected as redundant. Polymorphic config objects without strong types — rejected as looser type safety.
- **Consequences:** Every new format needs a newtype + variant. Strong-typed pathbufs need `Deref` or accessor for ergonomics.

## DD-009: YAGNI for import module internal structure
- **Date:** 2026-07-03
- **Phase:** Phase 1
- **Status:** Accepted
- **Context:** Considered building more scalable internal structure in the import module to support multiple transcompiler pipelines. Tension between YAGNI and idiomacy/extensibility.
- **Decision:** Keep V1 simple — one match in `import_shader` routing to format-specific logic. No abstract pipeline trait or strategy pattern yet.
- **Rationale:** The extensible enum design (`ImportedShaderPathBuf`) already provides structural scalability at the type level. Adding a second format is a new variant + new match arm. If a second format reveals a deeper pattern (fundamentally different pipelines), refactor then. YAGNI without violating the extensible enum design.
- **Alternatives considered:** Trait-based transcompiler strategy — rejected as premature. Separate module per format — rejected as over-structuring for one variant.
- **Consequences:** Second format addition may require refactoring if its pipeline differs fundamentally from ghostty's. Acceptable risk — we'll know more after V1.

## DD-010: Module naming — `shader_import` not `shader_translate`
- **Date:** 2026-07-03
- **Phase:** Phase 1
- **Status:** Accepted
- **Context:** Considered `shader_translate.rs` as module name. Realized translation implies cross-compilation which conflates two concerns: (1) normalizing imported formats to native WGSL, (2) cross-compiling WGSL to backend GLSL for OpenGL. SRP violation.
- **Decision:** Module is `shader_import.rs` — it imports foreign shader formats into native WGSL. Cross-compilation to GLSL is an OpenGL backend implementation detail, not this module's concern.
- **Rationale:** SRP. The import module normalizes imported shaders to WGSL, full stop. The OpenGL backend owns its own cross-compilation logic. Calling "import" not "translate" keeps the module's responsibility clear.
- **Alternatives considered:** `shader_translate` — rejected as conflating normalization with cross-compilation. `shader_normalize` — rejected, Bryan preferred "import."
- **Consequences:** Future OpenGL backend cross-compilation lives in a separate module, not here.

## DD-011: Switch from naga GLSL-in to glslang + naga SPIR-V-in pipeline
- **Date:** 2026-07-03
- **Phase:** Phase 1
- **Status:** Accepted
- **Context:** naga's GLSL frontend is too opinionated — can't handle combined `sampler2D` globals with initializers, requires `layout(set)` qualifiers, can't use ghostty's `shadertoy_prefix.glsl` verbatim. Required source-level workarounds (text substitution via `defines`, separate texture/sampler declarations, modified preamble). Two layers of string-mashing: GLSL preamble concat before parse, WGSL vertex shader concat after emit.
- **Decision:** Replace naga GLSL-in with glslang (GLSL → SPIR-V) → naga (SPIR-V-in → WGSL-out). Use ghostty's `shadertoy_prefix.glsl` verbatim (or near-verbatim) as the GLSL prefix — glslang handles combined `sampler2D`, `layout(binding)` without `set`, and all other GLSL features the prefix requires.
- **Rationale:** Eliminates all string-mashing workarounds. glslang is a full GLSL compiler (same tool ghostty uses). naga's SPIR-V front-end is more robust than its GLSL front-end. The pipeline mirrors ghostty's own approach (glslang → SPIR-V → target). Using ghostty's prefix verbatim means we can pull it as a reference/patch from the ghostty repo rather than maintaining our own modified copy.
- **Alternatives considered:** (1) naga GLSL-in with modified prefix (current V1 implementation) — rejected due to parser limitations and string-mashing. (2) glslang → SPIR-V → wgpu `ShaderSource::SpirV` directly (skip WGSL) — rejected because it would require a parallel pipeline separate from the existing WGSL-based `PostProcessState`, doubling maintenance. (3) glslang + spirv-cross (ghostty's full pipeline) — rejected because spirv-cross has no WGSL backend.
- **Consequences:** Adds glslang as a build dependency (C++ library, needs build integration). Adds naga `spv-in` feature. `shader_import.rs` rewritten: prefix GLSL with ghostty's `shadertoy_prefix.glsl`, compile via glslang to SPIR-V, parse SPIR-V via naga, emit WGSL via naga. The `POSTPROCESS_VERTEX_SHADER` concat in `webgpu.rs` may still be needed (naga emits a complete WGSL module from SPIR-V). The `iChannel0` `defines` workaround is removed. shadertoy_prefix.glsl can be sourced from ghostty repo via submodule/patch.

## DD-012: Patch file for prefix divergence instead of verbatim modification
- **Date:** 2026-08-08
- **Phase:** Phase 1
- **Status:** Accepted
- **Context:** DD-011 intended to use ghostty's prefix verbatim. However, naga's SPIR-V frontend cannot handle combined image samplers (`OpTypeSampledImage` loaded via `OpLoad`) — it expects separate `OpTypeImage` + `OpTypeSampler` combined via `OpSampledImage`. GLSL's `sampler2D` is a combined type; glslang emits the combined SPIR-V pattern naga can't parse. Need to split `sampler2D iChannel0` into separate `texture2D` + `sampler` at the GLSL level using GLSL 4.2+ separate texture/sampler syntax. Also need to move Globals block to `set = 1` to match wezterm's bind group layout (group 0 = texture+sampler, group 1 = uniform).
- **Decision:** Keep the verbatim `ghostty_shadertoy_prefix.glsl` file syncable from ghostty. Apply a `.patch` file at build time (in `build.rs`) to produce a patched prefix with: (1) `sampler2D iChannel0` split into `texture2D iChannel0_tex` + `sampler iChannel0_samp` + `#define iChannel0 sampler2D(iChannel0_tex, iChannel0_samp)`, (2) Globals block moved to `layout(set = 1, binding = 0)`. The patched file is generated into `OUT_DIR` and `include_str!`'d.
- **Rationale:** Explicit, reviewable divergence from upstream. When ghostty updates the prefix, re-sync the `.glsl` and re-apply the patch — if it conflicts, `patch` fails loudly. Keeps the verbatim file clean for diffing against upstream. The patch captures the wezterm-specific layout mapping (separate samplers for naga compatibility, set/binding placement for wezterm's pipeline).
- **Alternatives considered:** (1) Runtime string transformation in Rust — rejected as less explicit/reviewable. (2) Modify the prefix file directly — rejected as losing syncability with ghostty. (3) naga IR binding remap instead of patch-level set placement — rejected as mixing concerns (GLSL layout vs IR manipulation).
- **Consequences:** Build-time dependency on `patch` utility. Patch file must be maintained when ghostty prefix changes. The patched prefix matches wezterm's bind group layout exactly — naga IR no longer needs `remap_bindings`. IR manipulation reduces to: replace Globals struct (shrink to 4 members matching `PostProcessUniform`), add vertex shader (programmatic in naga IR), rename entry point.

## DD-013: Vertex shader as WGSL source, not naga IR construction
- **Date:** 2026-08-08
- **Phase:** Phase 1
- **Status:** Superseded by DD-015
- **Context:** The existing `POSTPROCESS_VERTEX_SHADER` const string in `webgpu.rs` was a WGSL string prepended to naga's emitted fragment WGSL. Considered keeping this approach (mirrors ghostty's separate vertex+fragment shader sources). User requested programmatic IR construction instead of string concat.
- **Decision:** Construct `vs_postprocess` vertex entry point programmatically in naga IR as part of `add_vertex_shader`. Delete `POSTPROCESS_VERTEX_SHADER` const from `webgpu.rs`. The emitted WGSL is one self-contained module with both entry points.
- **Rationale:** Cleaner — no string manipulation, one module, type-checked by naga validation. Avoids mixing levels (IR manipulation vs WGSL string). User preference for programmatic approach over string hacks.
- **Alternatives considered:** Keep `POSTPROCESS_VERTEX_SHADER` string prepend (ghostty's approach) — rejected per user preference for programmatic IR.
- **Consequences:** `add_vertex_shader` must construct naga IR for a fullscreen triangle vertex shader — types, expressions, function body built programmatically. More complex than string concat but more robust. `POSTPROCESS_VERTEX_SHADER` const deleted.

## DD-015: Vertex shader as WGSL source file, parsed via naga wgsl-in
- **Date:** 2026-08-08
- **Phase:** Phase 1
- **Status:** Accepted
- **Context:** DD-013 built the vertex shader programmatically in naga IR (~200 lines of arena/expression/emitter manipulation). User found this too verbose to maintain. Wanted WGSL source file included via `include_str!` (same pattern as the ghostty prefix). Initial attempt to parse WGSL vertex source and push the entry point across module arenas failed — naga handles are arena-local, can't cross modules.
- **Decision:** Emit fragment WGSL after IR manipulation (replace_globals_struct + rename_entry_point only), concat with standalone WGSL vertex shader source (`ghostty_fullscreen_vertex.wgsl`), re-parse the combined string via `naga::front::wgsl::parse_str` into a unified module, validate, emit final WGSL. The vertex shader file lives alongside the prefix in `shaders/`. Per-import-format (ghostty's fragment input is `@builtin(position)` from `gl_FragCoord`; other formats may differ). Added `wgsl-in` as runtime naga feature.
- **Rationale:** WGSL source file is readable, editable, diffable — same pattern as the GLSL prefix. naga handles module merging via re-parse — no manual handle remapping. Double parse+emit at startup is acceptable (runs once per shader). Validation on the unified module catches interface mismatches.
- **Alternatives considered:** (1) Push entry point across module arenas — rejected, handles are arena-local. (2) Manual handle remapping from vertex module into fragment module — rejected, exactly the fragility we're eliminating. (3) Post-emit string concat without re-parse — rejected, no validation of combined module.
- **Consequences:** `add_vertex_shader` IR function deleted. `wgsl-in` moves from dev-dep to runtime dep. Import pipeline: GLSL → SPIR-V → IR → replace_globals + rename → emit fragment WGSL → concat vertex WGSL → re-parse → validate → emit final WGSL. `WgslParseError` added to `ShaderImportError`. DD-013 superseded.

## DD-016: Separate vertex+fragment modules — user-owned bindings, no IR merging
- **Date:** 2026-08-08
- **Phase:** Phase 2
- **Status:** Accepted
- **Context:** DD-015's WGSL re-parse concat approach was criticized as cargo-culting — it re-parsed to "validate the combined module" when concat + wgpu's own validation does the job. Investigated alternatives: (1) naga has no linker and no module merge — handles are arena-local, cross-module pushes fail; (2) SPIR-V-level stage consolidation needs a linker glslang's Rust bindings don't expose (compile() consumes the program, one stage per program); (3) glslang can't merge stages — it's per-stage by design. Concluded any shared-module approach needs manual handle remapping. Then explored whether wezterm's pipeline even needs a single module. Found `compile_postprocess_shader` (webgpu.rs:243-258) currently passes the *same* module to both `VertexState` and `FragmentState`, but wgpu supports separate modules per stage.
- **Decision:** Imported shader formats emit a **pair** of WGSL files — one vertex, one fragment — compiled as independent naga modules and wired into the pipeline as separate `ShaderModule`s. No merging, no handle remapping, no IR construction, no re-parse. Resource bindings are declared by the author in each stage; the pipeline layout is the shared contract. wgpu validates VS/FS binding agreement at pipeline creation. The merge/re-synthesis problem is explicitly deferred to a future binding-synthesis layer that ties a type to a slot once and generates the `@group/@binding` declarations into both stages.
- **Rationale:** Eliminates the entire shared-module merge problem class — the varying-interface hole, the binding-collision ambiguity, the handle-remapping fragility all stop mattering because nothing is auto-unified. Separate modules give identifier-name isolation (both stages can declare a global named `Globals`). wgpu's `ShaderStages` visibility on bind group layout entries lets slots be shared (`VERTEX|FRAGMENT`) or stage-restricted. Cost is binding boilerplate duplicated across both stages — accepted, and the synthesis layer is the future fix.
- **Alternatives considered:** (1) Single module + IR construction (DD-013) — works but verbose; user found it too hard to maintain. (2) Single module + WGSL re-parse concat (DD-015) — re-parse was cargo-culting. (3) Single module + generic handle-remap merger matching by `(set,binding)` + layout — rejected: layout matching is semantically weak, and the varying interface has no external contract to pin it, so cross-stage unification is unsound. (4) SPIR-V stage consolidation via glslang Program/link — rejected: Rust bindings expose one stage per compile() call, and it'd require writing a linker.
- **Consequences:** The future binding-synthesis layer is the enhancement path. Imported formats need to author/synthesize both a vertex and fragment WGSL file. `compile_postprocess_shader` changes to accept two `ShaderModule`s (or two resolved sources). This supersedes the concat-in-DD-015 direction. DD-013 and DD-015 both superseded by this direction.

## DD-014: Replace Globals struct in naga IR, not strip
- **Date:** 2026-08-08
- **Phase:** Phase 1
- **Status:** Accepted
- **Context:** Ghostty's Globals block has 27 members; wezterm's `PostProcessUniform` has 4 (resolution, time, time_delta, frame). Stripping unused members isn't enough — field order and types differ (`iResolution` is vec3 in ghostty, vec2 in wezterm). The uniform buffer layout must match wezterm's exactly or runtime binding fails.
- **Decision:** Build a fresh replacement struct in naga IR with 4 members matching wezterm's `PostProcessUniform` layout: `iResolution` (vec2), `iTime` (f32), `iTimeDelta` (f32), `iFrame` (i32). Keep ghostty's field names so user shader code (`iResolution.xy`, `iTime`, etc.) works. Replace the Globals struct type handle in the global variable.
- **Rationale:** Building a fresh struct is simpler than strip+reorder+resize — one operation instead of three. Field names preserved for shader compatibility, types match wezterm's buffer layout. `iResolution` shrinks vec3→vec2 (shaders using `.z` for pixel ratio break — acceptable V1 limitation per DD-001).
- **Alternatives considered:** Strip unused members + reorder + resize — rejected as more complex for same result.
- **Consequences:** `replace_globals_struct` function in `shader_import.rs`. Shaders using `iResolution.z` break (V1 limitation). `iFrame` declared as `i32` in IR (wezterm writes `u32` — same bytes, non-negative values reinterpret cleanly).