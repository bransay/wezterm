# Implementation Plan: Ghostty-Style GLSL Shader Support for Wezterm

## Goal
Support loading and rendering Ghostty-style GLSL shaders (ShaderToy format with `mainImage` entry point) in Wezterm, using Wezterm's existing `custom_shaders` config option. Support both WebGpu and OpenGL backends.

## Existing Code Flow (WGSL Shaders - WebGpu Only)

### Config Loading
```
config/src/lib.rs:637-640
│
└─► custom_shaders paths added to file watch list (hot-reload)

config/src/config.rs:1339-1344
│
└─► Relative paths resolved to absolute (relative to config dir)
```

### Shader Initialization (on window create or config reload)
```
termwindow/mod.rs:894,1848
│
└─► reload_post_process_shaders()
    │
    ├─► if custom_shaders is empty: clear post_process state, return
    │
    ├─► if frontend != WebGpu: warn and return (shaders need WebGpu)
    │
    └─► PostProcessState::new(device, format, width, height, shader_paths)
        │
        ├─► Create bind group layouts:
        │   • texture_bind_group_layout (binding 0: texture_2d, binding 1: sampler)
        │   • uniform_bind_group_layout (binding 0: uniform buffer)
        │
        ├─► FOR EACH shader_path:
        │   │
        │   └─► compile_postprocess_shader(device, path, format, layouts)
        │         │
        │         ├─► std::fs::read(path) → raw bytes
        │         │
        │         ├─► prepare_shader_source(bytes, path)
        │         │   • Strip UTF-8 BOM if present
        │         │   • Validate UTF-8 encoding
        │         │   • Check non-empty
        │         │   • Prepend POSTPROCESS_PREAMBLE
        │         │   • Return full WGSL source
        │         │
        │         ├─► device.create_shader_module(Wgsl(source))
        │         │   (error scope catches validation errors)
        │         │
        │         ├─► device.create_pipeline_layout(bind_group_layouts)
        │         │
        │         └─► device.create_render_pipeline()
        │               • vertex: vs_postprocess (fullscreen triangle)
        │               • fragment: fs_postprocess (user entry)
        │               └─► Option<RenderPipeline> (None on error)
        │
        ├─► Create intermediate texture (terminal render target)
        │
        ├─► Create ping-pong texture (if >1 shaders)
        │
        ├─► Create sampler, bind groups, uniform buffer
        │
        └─► Return PostProcessState or None if all shaders failed
```

### Render Loop (every frame)
```
termwindow/render/draw.rs:52-269 :: call_draw_webgpu()
│
├─► webgpu.surface.get_current_texture() → output
│
├─► render_target = post_process.is_some()
│                    ? intermediate_view
│                    : surface_view
│
├─► [Render terminal layers → render_target]
│
└─► if post_process.is_some():
      │
      ├─► Build PostProcessUniform:
      │   • resolution: [width, height]
      │   • time: elapsed seconds since window creation
      │   • time_delta: frame delta
      │   • frame: incrementing counter
      │
      ├─► queue.write_buffer(&uniform_buffer, uniform)
      │
      └─► FOR EACH (i, pipeline) in pipelines:
            │
            ├─► ping_pong_targets(i, count) → (read_src, write_dst)
            │   • i=0, last:     read Intermediate → write Surface
            │   • i=0, not last: read Intermediate → write PingPong
            │   • i odd, last:   read PingPong    → write Surface
            │   • i odd, !last:  read PingPong    → write Intermediate
            │   • i even, !last: read Intermediate → write PingPong
            │
            ├─► render_pass.set_pipeline(pipeline)
            ├─► render_pass.set_bind_group(0, read_bind_group)
            ├─► render_pass.set_bind_group(1, uniform_bind_group)
            └─► render_pass.draw(0..3, 0..1)  // fullscreen triangle

    output.present()
```

### Texture Ping-Pong (2 shader chain example)
```
┌─────────────────┐         ┌─────────────┐         ┌───────────┐         ┌─────────┐
│ Terminal Layers │         │ INTERMEDIATE│         │  PINGPONG │         │ SURFACE │
└────────┬────────┘         └──────┬──────┘         └─────┬─────┘         └────┬────┘
         │                         │                      │                    │
         │ Render to               │                      │                    │
         └────────────────────────►│                      │                    │
                                   │                      │                    │
                                   ▼                      │                    │
                            ┌──────┴──────┐               │                    │
                            │  Shader 0   │               │                    │
                            └──────┬──────┘               │                    │
                                   │                      │                    │
                                   │ write                │                    │
                                   └─────────────────────►│                    │
                                                          │                    │
                                                          ▼                    │
                                                   ┌──────┴──────┐             │
                                                   │  Shader 1   │             │
                                                   └──────┬──────┘             │
                                                          │                    │
                                                          │ write              │
                                                          └───────────────────►│
                                                                               │
                                                                               ▼
                                                                        ┌──────┴──────┐
                                                                        │   present   │
                                                                        └──────┬──────┘
                                                                               │
                                                                               ▼
                                                                        ┌─────────┐
                                                                        │  Screen │
                                                                        └─────────┘
```

For N shaders: alternate between INTERMEDIATE and PINGPONG,
last shader always writes to SURFACE

### Shader Preamble (webgpu.rs:58-89 POSTPROCESS_PREAMBLE)
```
const POSTPROCESS_PREAMBLE: &str = "
    struct PostProcessUniform {
        resolution: vec2<f32>,
        time: f32,
        time_delta: f32,
        frame: u32,
        _padding: [u32; 3]
    }

    struct VertexOutput {
        @builtin(position) position: vec4<f32>,
        @location(0) uv: vec2<f32>
    }

    @group(0) @binding(0) var screen_texture: texture_2d<f32>
    @group(0) @binding(1) var screen_sampler: sampler
    @group(1) @binding(0) var<uniform> pp: PostProcessUniform

    @vertex
    fn vs_postprocess(vertex_index: u32) -> VertexOutput {
        // Fullscreen triangle from vertex_index
        // x = (vertex_index & 1) * 4 - 1  -> [-1, 3]
        // y = (vertex_index >> 1) * 4 - 1 -> [-1, 3]
    }
";

User shader provides ONLY:
    fn fs_postprocess(in: VertexOutput) -> @location(0) vec4<f32>
```

### Key Files
```
config/src/config.rs          - custom_shaders config field, path resolution
config/src/lib.rs             - file watcher setup for hot-reload
termwindow/mod.rs             - PostProcessState storage, reload trigger
termwindow/webgpu.rs          - PostProcessState, preamble, compilation
termwindow/render/draw.rs     - Render loop, ping-pong execution
```

## High-Level Architecture (GLSL Support)

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
   - Map Ghostty uniforms → Wezterm equivalents
   - Full list from ghostty's shadertoy_prefix.glsl

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
   - Investigate existing OpenGL renderer path
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

## Dependencies

- **naga** (already bundled with wgpu) - GLSL parsing and WGSL/GLSL output
- No new external dependencies

## Files to Modify/Create

TBD
