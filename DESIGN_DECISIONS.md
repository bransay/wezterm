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

## DD-014: Replace Globals struct in naga IR, not strip
- **Date:** 2026-08-08
- **Phase:** Phase 1
- **Status:** Accepted
- **Context:** Ghostty's Globals block has 27 members; wezterm's `PostProcessUniform` has 4 (resolution, time, time_delta, frame). Stripping unused members isn't enough — field order and types differ (`iResolution` is vec3 in ghostty, vec2 in wezterm). The uniform buffer layout must match wezterm's exactly or runtime binding fails.
- **Decision:** Build a fresh replacement struct in naga IR with 4 members matching wezterm's `PostProcessUniform` layout: `iResolution` (vec2), `iTime` (f32), `iTimeDelta` (f32), `iFrame` (i32). Keep ghostty's field names so user shader code (`iResolution.xy`, `iTime`, etc.) works. Replace the Globals struct type handle in the global variable.
- **Rationale:** Building a fresh struct is simpler than strip+reorder+resize — one operation instead of three. Field names preserved for shader compatibility, types match wezterm's buffer layout. `iResolution` shrinks vec3→vec2 (shaders using `.z` for pixel ratio break — acceptable V1 limitation per DD-001).
- **Alternatives considered:** Strip unused members + reorder + resize — rejected as more complex for same result.
- **Consequences:** `replace_globals_struct` function in `shader_import.rs`. Shaders using `iResolution.z` break (V1 limitation). `iFrame` declared as `i32` in IR (wezterm writes `u32` — same bytes, non-negative values reinterpret cleanly).

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
- **Context:** DD-015's WGSL re-parse concat approach was criticized as cargo-culting — it re-parsed to "validate the combined module" when concat + wgpu's own validation does the job. Investigated alternatives: (1) naga has no linker and no module merge — handles are arena-local, cross-module pushes fail; (2) SPIR-V-level stage consolidation needs a linker glslang's Rust bindings don't expose (compile() consumes the program, one stage per program); (3) glslang can't merge stages — it's per-stage by design. Concluded any shared-module approach needs manual handle remapping. Then explored whether wezterm's pipeline even needs a single module. Found `compile_postprocess_shader` (webgpu.rs) currently passes the *same* module to both `VertexState` and `FragmentState`, but wgpu supports separate modules per stage.
- **Decision:** Imported shader formats emit a **pair** of WGSL sources — one vertex, one fragment — compiled as independent modules and wired into the pipeline as separate `ShaderModule`s. No merging, no handle remapping, no IR construction, no re-parse. Resource bindings are declared by the author in each stage; the pipeline layout is the shared contract. wgpu validates VS/FS binding agreement at pipeline creation. The merge/re-synthesis problem is explicitly deferred to a future binding-synthesis layer that ties a type to a slot once and generates the `@group/@binding` declarations into both stages.
- **Rationale:** Eliminates the entire shared-module merge problem class — the varying-interface hole, the binding-collision ambiguity, the handle-remapping fragility all stop mattering because nothing is auto-unified. Separate modules give identifier-name isolation (both stages can declare a global named `Globals`). wgpu's `ShaderStages` visibility on bind group layout entries lets slots be shared (`VERTEX|FRAGMENT`) or stage-restricted. Cost is binding boilerplate duplicated across both stages — accepted, and the synthesis layer is the future fix.
- **Alternatives considered:** (1) Single module + IR construction (DD-013) — works but verbose; user found it too hard to maintain. (2) Single module + WGSL re-parse concat (DD-015) — re-parse was cargo-culting. (3) Single module + generic handle-remap merger matching by `(set,binding)` + layout — rejected: layout matching is semantically weak, and the varying interface has no external contract to pin it, so cross-stage unification is unsound. (4) SPIR-V stage consolidation via glslang Program/link — rejected: Rust bindings expose one stage per compile() call, and it'd require writing a linker.
- **Consequences:** The future binding-synthesis layer is the enhancement path. Imported formats need to author/synthesize both a vertex and fragment WGSL file. `compile_postprocess_shader` changes to accept two `ShaderModule`s (or two resolved sources). This supersedes the concat-in-DD-015 direction. DD-013 and DD-015 both superseded by this direction.

## DD-017: ResolvedShader carries vertex+fragment ShaderSource pairs
- **Date:** 2026-08-08
- **Phase:** Phase 2
- **Status:** Accepted
- **Context:** DD-016 requires `compile_postprocess_shader` to build two modules. Native shaders currently rely on `POSTPROCESS_PREAMBLE` (one blob with both `vs_postprocess` and `fs_postprocess`). Considered splitting the native preamble into separate vertex/fragment consts so each resolved member carries only its own stage.
- **Decision:** `ResolvedShader` holds `vertex: Arc<ShaderSource>` and `fragment: Arc<ShaderSource>`, where `ShaderSource { source: String, path: PathBuf }` (path is a `#[cfg(debug_assertions)]`-gated label for diagnostics). `Arc` avoids deep-copying potentially large shader sources (a refcount bump instead of cloning the WGSL `String`). For native shaders, both members share the *same* preamble+user source via one shared `Arc` and same path — no preamble split. WGSL modules are validated wholesale at `create_shader_module`, so the vertex module containing the fragment entry point (and vice versa) still validates if the source is correct; malformed source fails either module, net outcome identical. The split pays off for imported shaders, where vertex (static WGSL file) and fragment (naga output) genuinely differ.
- **Rationale:** No preamble split = less duplication and no risk of the two halves drifting out of sync. The separate-modules win is real only when stages come from different origins, which is the imported case. Native keeps a single source blob reused for both stage members. Dead-but-malformed code correctly fails syntactic validation regardless of reachability.
- **Alternatives considered:** Split `POSTPROCESS_PREAMBLE` into `POSTPROCESS_VERTEX` and `POSTPROCESS_FRAGMENT_PREAMBLE` — rejected: unnecessary when both native stages share one source; risks struct/binding drift between the split halves.
- **Consequences:** `resolve_shader` native path assigns the same `ShaderSource` to both members. Imported path assigns the static vertex WGSL file to `vertex` and naga output to `fragment`. `compile_postprocess_shader` creates two `ShaderModule`s and wires `VertexState`→vertex, `FragmentState`→fragment.

## DD-018: Cursor uniform scope — position trio + color pair (5 uniforms)
- **Date:** 2026-08-10
- **Phase:** Phase 4
- **Status:** Accepted
- **Context:** DD-001 deferred terminal-specific uniforms (cursor, palette, focus) to V2. Phase 3 investigation found 5 shaders fail on out-of-scope cursor/iMouse uniforms. Phase 4 design session investigated ghostty-shaders repo and selected 3 representative cursor shaders: `cursor_blaze.glsl`, `cursor_lightning.glsl` (position/time only), and `in-game-crt-cursor.glsl` (position + color). Scope driven empirically by what the 3 shaders actually use, not the full Ghostty set.
- **Decision:** Support 5 cursor uniforms: `iCurrentCursor` (vec4), `iPreviousCursor` (vec4), `iTimeCursorChange` (float), `iCurrentCursorColor` (vec4), `iPreviousCursorColor` (vec4). Defer the remaining Ghostty extensions (cursor style, visibility, focus, palette, bg/fg colors, selection colors) to a later phase.
- **Rationale:** The 3 target shaders cover the common cursor-effect patterns (trails, lightning, glow-with-color-tint). Bringing colors along is ~20-30% more effort over position-only (marginal cost once the plumbing exists) and unlocks `in-game-crt-cursor` as a third fixture. The full 27-member Ghostty set would bloat the uniform buffer (iPalette alone is 4KB) for features no target shader uses.
- **Alternatives considered:** Position trio only (2 shaders) — rejected, marginal savings vs. the 5-uniform set. Full Ghostty uniform set — rejected as scope creep, no target shader needs it.
- **Consequences:** 5 shaders from the ghostty-shaders repo that use only these 5 uniforms (plus the V1 basics) will work. Shaders using `iCursorVisible`, `iFocus`, `iCurrentCursorStyle`, `iPalette`, etc. remain unsupported until a later phase.

## DD-019: Per-pane cursor state tracking via `CursorRenderState` on `PaneState`
- **Date:** 2026-08-10
- **Phase:** Phase 4
- **Status:** Accepted
- **Context:** Ghostty tracks `previous_cursor`/`cursor_change_time` per-surface (single-surface model). Wezterm's post-process is window-global (one uniform buffer, one render pass over the composited surface), but panes have independent cursors. On pane switch, a window-global "previous cursor" would be pane A's last position and "current" would be pane B's — producing a spurious giant trail spanning panes. `PrevCursorPos` (`prevcursor.rs`) already does position change-detection for blink timing but is window-global (acceptable for blink, not for trails).
- **Decision:** Add a `CursorRenderState` struct on `PaneState` (per-pane), mirroring `PrevCursorPos`'s change-detect mechanism. Updated in `paint_pane` for every rendered pane. The uniform write in `call_draw_webgpu` reads the active pane's `CursorRenderState`. The shader sees a single global cursor (Ghostty-faithful) — per-pane is host-side bookkeeping only.
- **Rationale:** Investigation found that `TermWindow` mixes interaction and render state freely on the same struct (no category boundary enforced), so co-locating `CursorRenderState` (render-derived) with `PaneState`'s interaction fields follows the codebase's established pattern. Per-pane addressing fixes the pane-switch spurious-trail edge case that `PrevCursorPos`'s window-global model can't handle. The limitation (inactive panes show no effect) is invisible for these shaders — trails/lightning only animate on a moving cursor, and inactive cursors are static.
- **Alternatives considered:** Window-global on `TermWindow` (like `PrevCursorPos`) — rejected: spurious trails on pane switch. New `HashMap<PaneId, CursorRenderState>` on `TermWindow` — rejected: more plumbing for a single field when `PaneState` already provides per-pane addressing. Splitting the uniform buffer per-pane — rejected: would require per-pane render passes (architectural lift) and the effects don't need it. Structured/storage buffer with per-pane records — rejected: breaks Ghostty's single-surface shader contract.
- **Consequences:** Only the active pane's cursor uniforms are bound each frame. Inactive panes render with no cursor effect (correct for these shaders). Pane switch uses the new pane's own cursor history — no spurious trail. `PrevCursorPos` remains untouched (blink timing stays window-global).

## DD-020: Cursor color source — `palette.cursor_bg` (config `cursor_color`)
- **Date:** 2026-08-10
- **Phase:** Phase 4
- **Status:** Accepted
- **Context:** `iCurrentCursorColor`/`iPreviousCursorColor` (vec4 RGBA) need a source. Wezterm has `cursor_border_color` in `ComputeCellFgBgResult` (a render result, shape-dependent) and `palette.cursor_bg` (config-level, shape-agnostic). Ghostty's `iCurrentCursorColor` is the resolved cursor paint color (`generic.zig:2475-2511`: OSC 12 → config → cell fg/bg → foreground fallback), shape-agnostic — one color regardless of block/bar/underline.
- **Decision:** Map `iCurrentCursorColor`/`iPreviousCursorColor` to `palette.cursor_bg` (sourced from config `cursor_color`), normalized to `[0,1]` RGBA. Shape-agnostic, matching Ghostty's single resolved color model.
- **Rationale:** `palette.cursor_bg` is wezterm's closest equivalent to Ghostty's resolved cursor paint color — it's the fill for block cursors and the stroke for bar/underline (`render/mod.rs:617-663`), exactly mirroring Ghostty's shape-agnostic behavior. `cursor_border_color` is a render *result* alias for `cursor_bg` (line 706) and is computed per-cell; using it would be less faithful (per-cell variation Ghostty doesn't expose) and more wiring. `palette.cursor_bg` is stable, plumbed once per frame, and matches what the user sees.
- **Alternatives considered:** `cursor_border_color` from `ComputeCellFgBgResult` — rejected: it's the render result, not the config source, and is shape-dependent (Ghostty's value is neither). Per-cell computed cursor color — rejected as unnecessary fidelity for V1; `in-game-crt-cursor`'s `colorOverride` logic is robust to a config-level input.
- **Consequences:** Shaders see the config cursor color, not per-cell overrides. If a cell's attributes shift the rendered cursor color, the shader won't reflect that — acceptable V1 limitation. Color change (e.g., via OSC 12 or config reload) triggers `cursor_change_time` update.

## DD-021: Uniform struct growth — extend DD-014 pattern, 32→112 bytes
- **Date:** 2026-08-10
- **Phase:** Phase 4
- **Status:** Accepted
- **Context:** `PostProcessUniform` (`webgpu.rs:27`) is currently 32 bytes (4 fields + padding), hard-asserted at `webgpu.rs:1173`. Adding 5 cursor uniforms (4×vec4 + 1×f32) requires growing the struct. Three locations must stay in lockstep: the host struct, the native WGSL preamble (`POSTPROCESS_PREAMBLE`), and `replace_globals_struct` (naga IR). The GLSL prefix (`ghostty_shadertoy_prefix.glsl`) already declares all 27 Ghostty uniforms — no patch changes needed.
- **Decision:** Extend all three in lockstep (DD-014 pattern): `PostProcessUniform` grows to 112 bytes (keep existing 4 fields + `_padding: [u32;3]` at 0–32, add 4×vec4 at 32–96, add `cursor_change_time: f32` at 96, trailing pad to 112). `POSTPROCESS_PREAMBLE` mirrors the struct. `replace_globals_struct` emits the 9-member struct with Ghostty field names. Update the size assert from 32 to 112. Minimal wezterm-native layout — not the full 27-member Ghostty struct.
- **Rationale:** The existing `_padding: [u32;3]` already aligns the first vec4 to a 16-byte boundary (offset 32). Minimal layout keeps the buffer small (112 bytes vs. 4KB+ for the full Ghostty set) and avoids populating unused fields. No patch file changes needed because `replace_globals_struct` replaces the entire Globals type after glslang compilation — the GLSL prefix's 27-member declaration is discarded in IR.
- **Alternatives considered:** Keep the full 27-member Ghostty struct and zero-fill unused fields — rejected: buffer bloat (iPalette = 4KB), std140 offset matching complexity, and we'd still need the host struct to match. Separate uniform buffer for cursor fields — rejected: adds a bind group, no benefit over one growing struct.
- **Consequences:** Adding a uniform requires editing three locations by hand (host struct, WGSL preamble, naga IR) — this lockstep burden is deferred to Phase 5 (decouple struct definitions from dual maintenance). The `webgpu.rs:1173` assert must be updated or the build breaks. Existing tests using `PostProcessUniform::default()` get zeroed cursor fields (safe — shaders handle zero cursor gracefully).

## DD-022: Remap Globals `AccessIndex` expressions in naga IR
- **Date:** 2026-08-10
- **Phase:** Phase 4
- **Status:** Accepted
- **Context:** Extending `replace_globals_struct` (DD-021) to a 9-member struct broke cursor shaders. naga tracks struct member access by **index**, not name. The SPIR-V Globals struct has 27 members; `iCurrentCursor` sits at index 10, `iPreviousCursor` at 11, `iCurrentCursorColor` at 12, `iPreviousCursorColor` at 13, `iTimeCursorChange` at 17. After replacing the type with the 9-member struct, `AccessIndex` expressions referencing those old indices were out of bounds (`OutOfBoundsIndex { index: 10 }`). The existing 4-member replacement only worked because tested shaders touch `iResolution`/`iTime` (indices 0/1, preserved).
- **Decision:** After replacing the Globals struct type, remap `AccessIndex` expressions whose base is the Globals global from old member indices to new ones. The Globals global variable has an **empty name** — identify it by its type being the Globals struct handle, not by name. The `GlobalVariable` expression lives in the function's expressions arena (not `global_expressions`). Remap table: 0→0 (iResolution), 1→1 (iTime), 2→2 (iTimeDelta), 4→3 (iFrame), 10→4 (iCurrentCursor), 11→5 (iPreviousCursor), 12→6 (iCurrentCursorColor), 13→7 (iPreviousCursorColor), 17→8 (iTimeCursorChange).
- **Rationale:** naga's `Expression::AccessIndex { base, index }` uses a numeric index into the struct's member list; there is no name-based access. Since we replace the type wholesale, the indices must be rewritten to match the new member order. The remap is a fixed table derived from the GLSL prefix's member order.
- **Alternatives considered:** Keep the full 27-member struct (no remap needed) — rejected in DD-021 (buffer bloat). Name-based member resolution — naga doesn't support it for `AccessIndex`.
- **Consequences:** `remap_globals_access_indices` is a new function in `shader_import.rs`, called from `replace_globals_struct`. The remap table is coupled to the GLSL prefix's member order — if the prefix changes, the table must be updated. This is another instance of the Phase 5 dual-maintenance concern.

## DD-023: `iCurrentCursor.xy` is the bottom-left corner (Y-down space)
- **Date:** 2026-08-10
- **Phase:** Phase 4
- **Status:** Accepted
- **Context:** Visual testing with `cursor_blaze.glsl` showed the blaze trail rendered one cell-height above the actual cursor. The magenta-box diagnostic shader (drawing `xy`→`xy+zw`) couldn't distinguish the corner convention because it covers the cell either way. Ghostty's `generic.zig:2140-2182` computes `pixel_y` as the **bottom edge** of the cursor glyph in Y-down space (`pixel_y += cell.height - bearing + glyph_height`), and the blaze shader's `offsetFactor = (-0.5, +0.5)` with `center = xy - (zw * offsetFactor)` only yields the center if `xy.y` is the bottom edge.
- **Decision:** `iCurrentCursor.xy` is the **bottom-left** corner in Y-down space (matching `fragCoord`). When populating the rect from wezterm's top-left `cursor_pixel_rect`, pass `y + h` as the Y component.
- **Rationale:** Ghostty's shader contract defines `xy` as the "-X, +Y corner" (`Config.zig:2954`), and in Y-down space +Y is down, so that's the bottom-left corner. The blaze shader's center math confirms it. Wezterm's `cursor_pixel_rect` returns top-left; the +h offset converts it.
- **Alternatives considered:** Keep top-left and let shaders handle it — rejected: breaks Ghostty shader compat (blaze/lightning assume bottom-left). Flip the whole texture Y — rejected: would break the existing post-process pipeline.
- **Consequences:** `paint_pane` passes `y + h` for the cursor rect Y. The magenta-box diagnostic shader is insufficient to verify corner convention — a shader that computes a center from the rect (like blaze) is needed.

## DD-024: Reflection-based uniform synthesis — proc-macro emits data, use sites render
- **Date:** 2026-08-13
- **Phase:** Phase 5
- **Status:** Accepted
- **Context:** Phase 5 goal is single-source-of-truth for the uniform struct. Investigated three approaches: (1) build.rs + syn — can't see field attributes without unstable `#![register_attr]`; (2) proc-macro emitting WGSL+GLSL strings directly — centralizes target-language knowledge in the codegen crate, not scalable when new shader formats are added; (3) proc-macro emitting reflection data + use sites rendering locally. Also considered DX-style "constant buffer" naming — settled on `UniformBuffer` derive.
- **Decision:** Two crates. `wezterm-shader-types` (plain crate): `UniformType` enum (`Vec2`/`Float`/`UInt`/`Vec4`) + `UniformField { name, ty }`. `wezterm-shader-codegen` (proc-macro): `#[derive(UniformBuffer)]` emits `UNIFORM_FIELDS: &[UniformField]` — pure reflection, no target-language knowledge. Field annotations: `#[uniform_type(Vec2)]` (required on every non-ignored field, error if missing) + `#[uniform_ignore]` (padding; `#[ignore]` collides with a builtin attribute). Each use site (native WGSL preamble, ghostty GLSL block) renders the field list into its own language at runtime, cached in `OnceLock`. The wezterm↔ghostty mapping (names, Y-flip, type conversions) lives in the ghostty patch's `populate_globals()`.
- **Rationale:** Decouples the codegen crate from any specific shader language — adding a format is a new renderer at the use site, not a codegen crate change. The struct stays wezterm-native (no ghostty leakage). Runtime rendering is one-time (OnceLock), negligible cost. Splitting types into a plain crate lets use sites reference `UniformField`/`UniformType` without pulling proc-macro deps (idiomatic `serde`/`serde_derive` split).
- **Alternatives considered:** (1) build.rs + syn — rejected: no field attributes without unstable feature. (2) Proc-macro emitting WGSL+GLSL strings — rejected: centralizes language knowledge, not scalable. (3) Attribute macro — rejected: can't register per-field helper attributes. (4) Single crate with types in the proc-macro crate — rejected: pulls proc-macro deps into use sites.
- **Consequences:** Adding a uniform = add field + `#[uniform_type(...)]` annotation to `PostProcessUniform`; WGSL preamble and GLSL block render automatically. The naga IR surgery (`replace_globals_struct`/`remap_globals_access_indices`) is deleted. The `y + h` Y-flip moves into `populate_globals()`. The ghostty patch's `populate_globals()` is the single place wezterm↔ghostty conventions are encoded.