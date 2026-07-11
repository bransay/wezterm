use config::ImportedShaderPathBuf;
use std::path::Path;

/// Errors that can occur during shader import (cross-compilation from
/// a foreign format to WGSL).
#[derive(Debug, thiserror::Error)]
pub enum ShaderImportError {
    #[error("failed to read shader file {path}: {error}")]
    ReadError {
        path: String,
        error: std::io::Error,
    },
    #[error("shader file {path} is not valid UTF-8: {error}")]
    InvalidUtf8 {
        path: String,
        error: std::str::Utf8Error,
    },
    #[error("shader file {path} is empty")]
    EmptyShader { path: String },
    #[error("GLSL parse error in {path}: {error}")]
    ParseError {
        path: String,
        error: naga::front::glsl::ParseErrors,
    },
    #[error("validation error in {path}: {error}")]
    ValidationError {
        path: String,
        error: naga::WithSpan<naga::valid::ValidationError>,
    },
    #[error("WGSL emit error in {path}: {error}")]
    EmitError {
        path: String,
        error: naga::back::wgsl::Error,
    },
}

/// GLSL preamble prepended to every Ghostty/shadertoy shader before parsing.
/// Declares the shadertoy-convention uniforms with layout qualifiers that map
/// to wezterm's existing post-process bind group layout:
///   set 0, binding 0: screen texture
///   set 0, binding 1: screen sampler
///   set 1, binding 0: post-process uniform buffer
///
/// Also provides a `main()` entry point that calls the user's `mainImage`.
///
/// Note: `iChannel0` is not declared here — it's handled via naga's preprocessor
/// `defines` as a text substitution that replaces `iChannel0` with
/// `sampler2D(iChannel0_texture, iChannel0_sampler)` in the user's shader code.
/// This is because GLSL opaque types (sampler2D) cannot be declared as global
/// variables with initializers.
const GHOSTTY_PREAMBLE: &str = "\
#version 450 core

layout(set = 0, binding = 0) uniform texture2D iChannel0_texture;
layout(set = 0, binding = 1) uniform sampler iChannel0_sampler;

layout(set = 1, binding = 0) uniform PostProcessUniform {
    vec2  resolution;
    float time;
    float time_delta;
    uint  frame;
};

// Shadertoy-exposed globals derived from the uniform block.
vec3 iResolution = vec3(resolution, 1.0);
float iTime = time;
float iTimeDelta = time_delta;
int iFrame = int(frame);

layout(location = 0) out vec4 wez_frag_color;

// Forward declaration of the user's entry point
void mainImage(out vec4 fragColor, in vec2 fragCoord);

void main() {
    vec4 color;
    mainImage(color, gl_FragCoord.xy);
    wez_frag_color = color;
}
";

/// Import (cross-compile) a foreign shader to WGSL.
///
/// Takes an `ImportedShaderPathBuf` and produces WGSL source ready for
/// compilation by the existing post-process pipeline.
pub fn import_shader(shader: &ImportedShaderPathBuf) -> Result<String, ShaderImportError> {
    match shader {
        ImportedShaderPathBuf::Ghostty(path) => import_ghostty(path.as_path()),
    }
}

/// Cross-compile a Ghostty/shadertoy GLSL shader to WGSL.
fn import_ghostty(path: &Path) -> Result<String, ShaderImportError> {
    let path_str = path.display().to_string();

    let raw_bytes = std::fs::read(path).map_err(|e| ShaderImportError::ReadError {
        path: path_str.clone(),
        error: e,
    })?;

    // Strip UTF-8 BOM if present
    let source_str = if raw_bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        std::str::from_utf8(&raw_bytes[3..]).map_err(|e| ShaderImportError::InvalidUtf8 {
            path: path_str.clone(),
            error: e,
        })?
    } else {
        std::str::from_utf8(&raw_bytes).map_err(|e| ShaderImportError::InvalidUtf8 {
            path: path_str.clone(),
            error: e,
        })?
    };

    if source_str.trim().is_empty() {
        return Err(ShaderImportError::EmptyShader {
            path: path_str,
        });
    }

    // Prepend the preamble with shadertoy uniform declarations and main() shim
    let full_source = format!("{}\n{}", GHOSTTY_PREAMBLE, source_str);

    // Parse GLSL → naga IR
    // Use naga's preprocessor defines to alias `iChannel0` to a combined
    // sampler2D constructor call. GLSL opaque types can't be global variables
    // with initializers, so we do text substitution instead.
    let mut frontend = naga::front::glsl::Frontend::default();
    let mut options = naga::front::glsl::Options::from(naga::ShaderStage::Fragment);
    options.defines.insert(
        "iChannel0".to_string(),
        "sampler2D(iChannel0_texture, iChannel0_sampler)".to_string(),
    );
    let module = frontend
        .parse(&options, &full_source)
        .map_err(|e| ShaderImportError::ParseError {
            path: path_str.clone(),
            error: e,
        })?;

    // Validate the module
    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    );
    let info = validator
        .validate(&module)
        .map_err(|e| ShaderImportError::ValidationError {
            path: path_str.clone(),
            error: e,
        })?;

    // Emit WGSL
    let wgsl = naga::back::wgsl::write_string(&module, &info, naga::back::wgsl::WriterFlags::empty())
        .map_err(|e| ShaderImportError::EmitError {
            path: path_str,
            error: e,
        })?;

    Ok(wgsl)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_import_error_missing_file() {
        let result = import_ghostty(&PathBuf::from("/nonexistent/shader.glsl"));
        assert!(matches!(result, Err(ShaderImportError::ReadError { .. })));
    }

    #[test]
    fn test_import_error_empty_shader() {
        let temp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(temp.path(), b"   \n  \t  ").unwrap();
        let result = import_ghostty(temp.path());
        assert!(matches!(result, Err(ShaderImportError::EmptyShader { .. })));
    }

    #[test]
    fn test_import_simple_shader() {
        let glsl = r#"
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    fragColor = texture(iChannel0, uv);
}
"#;
        let temp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(temp.path(), glsl.as_bytes()).unwrap();
        let result = import_ghostty(temp.path());
        assert!(
            result.is_ok(),
            "Simple shader should import: {:?}",
            result.err()
        );
        let wgsl = result.unwrap();
        assert!(!wgsl.is_empty());
    }

    #[test]
    fn test_import_crt_like_shader() {
        // Simplified CRT-style shader using iResolution, iTime, iChannel0
        let glsl = r#"
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    // Warp the UV coordinates slightly (CRT curvature effect)
    vec2 dc = abs(0.5 - uv);
    dc *= dc;
    uv.x -= 0.5; uv.x *= 1.0 + (dc.y * 0.1); uv.x += 0.5;
    uv.y -= 0.5; uv.y *= 1.0 + (dc.x * 0.1); uv.y += 0.5;

    // Scanline effect
    float scan = abs(sin(fragCoord.y) * 0.1);

    vec3 color = texture(iChannel0, uv).rgb;
    fragColor = vec4(mix(color, vec3(0.0), scan), 1.0);
}
"#;
        let temp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(temp.path(), glsl.as_bytes()).unwrap();
        let result = import_ghostty(temp.path());
        assert!(
            result.is_ok(),
            "CRT-like shader should import: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_import_bom_shader() {
        let glsl = b"\xEF\xBB\xBFvoid mainImage(out vec4 fragColor, in vec2 fragCoord) {\n    fragColor = texture(iChannel0, fragCoord / iResolution.xy);\n}";
        let temp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(temp.path(), glsl).unwrap();
        let result = import_ghostty(temp.path());
        assert!(result.is_ok(), "BOM shader should import: {:?}", result.err());
    }
}