# Implementation Plan: Ghostty-Style GLSL Shader Support for Wezterm

## Goal
Support loading and rendering Ghostty-style GLSL shaders (ShaderToy format with `mainImage` entry point) in Wezterm, using Wezterm's existing `custom_shaders` config option. Support both WebGpu and OpenGL backends.

## High-Level Architecture

```
User GLSL shader (mainImage entry)
        ↓
[Prepend #define aliases for uniform renaming]
        ↓
[naga GLSL parser → IR]
        ↓
[naga WGSL output] ─→ WebGpu backend
[naga GLSL output] ─→ OpenGL backend
```

## Key Decisions

- **Parser**: naga GLSL parser (pure Rust, already bundled with wgpu)
- **Uniform renaming**: Preprocessor `#define` aliases (e.g., `#define iResolution wez_resolution`)
- **Backend support**: Both WebGpu and OpenGL from day one
- **Config UX**: Auto-detect GLSL by `.glsl` extension, `.wgsl` stays as-is
- **Error handling**: Graceful degradation like existing WGSL shader errors

## Implementation Steps

### Phase 1: GLSL Parsing Pipeline

1. **Create GLSL shader loader module**
   - Detect `.glsl` extension in shader paths
   - Read GLSL source from file
   - Prepend uniform alias defines
   - Parse via naga GLSL front-end

2. **Define uniform alias mapping**
   - Map each Ghostty uniform to Wezterm equivalent
   - `iResolution` → `wez_resolution`
   - `iTime` → `wez_time`
   - `iChannel0` → `wez_screen_texture`
   - (full list derived from ghostty's shadertoy_prefix.glsl)

3. **Wrap user shader with preamble**
   - Inject uniform struct definition (Wezterm naming)
   - Inject `#define` aliases for Ghostty names
   - Inject `main()` wrapper calling `mainImage()`
   - Similar to Ghostty's `shadertoy_prefix.glsl`

### Phase 2: Cross-Compilation Targets

4. **WebGpu target (WGSL output)**
   - Use naga's `back::wgsl::write()` to generate WGSL
   - Integrate with existing `PostProcessState` pipeline

5. **OpenGL target (GLSL output)**
   - Use naga's `back::glsl::write()` to generate GLSL
   - Requires OpenGL shader pipeline path (investigate existing OpenGL renderer)
   - May need vertex shader coordination

### Phase 3: Uniform Binding Integration

6. **Create uniform buffer struct**
   - Mirror Ghostty's `Globals` uniform block
   - Wire to Wezterm's render loop
   - Update per-frame (time, resolution, etc.)
   - Update per-change (cursor position, palette, focus, etc.)

7. **Provide terminal screen texture**
   - `iChannel0` / `wez_screen_texture` binds to terminal render target
   - Verify texture format matches shader expectations

### Phase 4: Config Integration

8. **Extend `custom_shaders` handling**
   - Detect `.glsl` vs `.wgsl` by extension
   - Route to appropriate compilation pipeline
   - Support mixed lists (both GLSL and WGSL shaders)

9. **Hot-reload support**
   - Add GLSL files to watch list (extend existing config watching)
   - Re-trigger compilation when GLSL shader changes

### Phase 5: Testing & Validation

10. **Test with ghostty-shaders repo**
    - Clone https://github.com/0xhckr/ghostty-shaders
    - Validate compilation on various shaders (crt.glsl, bettercrt.glsl, fireworks.glsl, etc.)
    - Verify uniform binding correctness

11. **Error handling**
    - Compilation errors: Show notification, graceful fallback
    - Missing uniforms: Preprocessor aliases only define what we support
    - Validation on shader load, not render loop

## Open Questions

_(None currently - all resolved during planning)_

## Dependencies

- **naga** (already bundled with wgpu) - GLSL parsing and WGSL/GLSL output
- No new external dependencies

## Files to Modify/Create

_(To be detailed during implementation)_