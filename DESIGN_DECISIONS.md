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