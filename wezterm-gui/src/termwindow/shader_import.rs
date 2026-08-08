use config::ImportedShaderPathBuf;
use std::path::Path;

use crate::termwindow::webgpu::ResolvedShader;

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
    #[error("glslang compilation error in {path}: {error}")]
    GlslangError {
        path: String,
        error: glslang::error::GlslangError,
    },
    #[error("SPIR-V parse error in {path}: {error}")]
    SpvParseError {
        path: String,
        error: naga::front::spv::Error,
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

/// Patched shadertoy prefix, generated at build time from the verbatim
/// ghostty source + `ghostty_shadertoy_prefix.patch`.
const GHOSTTY_SHADERTOY_PREFIX: &str = include_str!(concat!(
    env!("OUT_DIR"),
    "/ghostty_shadertoy_prefix_patched.glsl"
));

/// Import (cross-compile) a foreign shader to a resolved WGSL shader.
pub fn import_shader(shader: &ImportedShaderPathBuf) -> Result<ResolvedShader, ShaderImportError> {
    match shader {
        ImportedShaderPathBuf::Ghostty(path) => import_ghostty(path.as_path()),
    }
}

/// GLSL → glslang → SPIR-V → naga IR → WGSL
fn import_ghostty(path: &Path) -> Result<ResolvedShader, ShaderImportError> {
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

    let full_source = format!("{}\n{}", GHOSTTY_SHADERTOY_PREFIX, source_str);

    let spirv_bytes = compile_glsl_to_spirv(&full_source, &path_str)?;

    let spv_options = naga::front::spv::Options::default();
    let mut module = naga::front::spv::parse_u8_slice(&spirv_bytes, &spv_options)
        .map_err(|e| ShaderImportError::SpvParseError {
            path: path_str.clone(),
            error: e,
        })?;

    replace_globals_struct(&mut module);
    add_vertex_shader(&mut module);
    rename_entry_point(&mut module);

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

    let wgsl = naga::back::wgsl::write_string(&module, &info, naga::back::wgsl::WriterFlags::empty())
        .map_err(|e| ShaderImportError::EmitError {
            path: path_str.clone(),
            error: e,
        })?;

    Ok(ResolvedShader::new(wgsl, std::path::PathBuf::from(path_str)))
}

fn compile_glsl_to_spirv(
    source: &str,
    path_str: &str,
) -> Result<Vec<u8>, ShaderImportError> {
    let compiler = glslang::Compiler::acquire().ok_or_else(|| {
        ShaderImportError::GlslangError {
            path: path_str.to_string(),
            error: glslang::error::GlslangError::NoLanguageTarget,
        }
    })?;

    let shader_source = glslang::ShaderSource::from(source.to_string());
    let options = glslang::CompilerOptions {
        source_language: glslang::SourceLanguage::GLSL,
        target: glslang::Target::Vulkan {
            version: glslang::VulkanVersion::Vulkan1_2,
            spirv_version: glslang::SpirvVersion::SPIRV1_5,
        },
        version_profile: None,
        messages: glslang::ShaderMessage::DEFAULT,
    };
    let defines: Option<&[(&str, Option<&str>)]> = None;
    let input = glslang::ShaderInput::new(
        &shader_source,
        glslang::ShaderStage::Fragment,
        &options,
        defines,
        None,
    )
    .map_err(|e| ShaderImportError::GlslangError {
        path: path_str.to_string(),
        error: e,
    })?;

    let shader = compiler
        .create_shader(input)
        .map_err(|e| ShaderImportError::GlslangError {
            path: path_str.to_string(),
            error: e,
        })?;

    shader
        .compile()
        .map(|words: Vec<u32>| words.iter().flat_map(|w| w.to_le_bytes()).collect())
        .map_err(|e| ShaderImportError::GlslangError {
            path: path_str.to_string(),
            error: e,
        })
}

/// Replace the Globals uniform block with a 4-member struct matching
/// wezterm's `PostProcessUniform` layout.
fn replace_globals_struct(module: &mut naga::Module) {
    let globals_handle = module
        .types
        .iter()
        .find(|(_, ty)| {
            ty.name.as_deref() == Some("Globals")
                && matches!(ty.inner, naga::TypeInner::Struct { .. })
        })
        .map(|(handle, _)| handle);

    let Some(globals_handle) = globals_handle else {
        return;
    };

    let f32_scalar = naga::Scalar::F32;
    let i32_scalar = naga::Scalar::I32;

    let vec2f = module.types.insert(
        naga::Type {
            name: None,
            inner: naga::TypeInner::Vector {
                size: naga::VectorSize::Bi,
                scalar: f32_scalar,
            },
        },
        naga::Span::default(),
    );
    let f32_ty = module.types.insert(
        naga::Type {
            name: None,
            inner: naga::TypeInner::Scalar(f32_scalar),
        },
        naga::Span::default(),
    );
    let i32_ty = module.types.insert(
        naga::Type {
            name: None,
            inner: naga::TypeInner::Scalar(i32_scalar),
        },
        naga::Span::default(),
    );

    let new_struct = naga::Type {
        name: Some("Globals".to_string()),
        inner: naga::TypeInner::Struct {
            members: vec![
                naga::StructMember {
                    name: Some("iResolution".to_string()),
                    ty: vec2f,
                    binding: None,
                    offset: 0,
                },
                naga::StructMember {
                    name: Some("iTime".to_string()),
                    ty: f32_ty,
                    binding: None,
                    offset: 8,
                },
                naga::StructMember {
                    name: Some("iTimeDelta".to_string()),
                    ty: f32_ty,
                    binding: None,
                    offset: 12,
                },
                naga::StructMember {
                    name: Some("iFrame".to_string()),
                    ty: i32_ty,
                    binding: None,
                    offset: 16,
                },
            ],
            span: 32,
        },
    };

    module.types.replace(globals_handle, new_struct);
}

/// Fullscreen triangle vertex shader from `@builtin(vertex_index)`.
fn add_vertex_shader(module: &mut naga::Module) {
    let span = naga::Span::default();

    let u32_ty = module.types.insert(
        naga::Type {
            name: None,
            inner: naga::TypeInner::Scalar(naga::Scalar::U32),
        },
        span,
    );
    let i32_ty = module.types.insert(
        naga::Type {
            name: None,
            inner: naga::TypeInner::Scalar(naga::Scalar::I32),
        },
        span,
    );
    let f32_ty = module.types.insert(
        naga::Type {
            name: None,
            inner: naga::TypeInner::Scalar(naga::Scalar::F32),
        },
        span,
    );
    let vec4f = module.types.insert(
        naga::Type {
            name: None,
            inner: naga::TypeInner::Vector {
                size: naga::VectorSize::Quad,
                scalar: naga::Scalar::F32,
            },
        },
        span,
    );

    let mut expressions = naga::Arena::new();

    // Pre-emitted expressions don't need Emit statements.
    let vertex_index_expr = expressions.append(
        naga::Expression::FunctionArgument(0),
        span,
    );
    let one_u32 = expressions.append(
        naga::Expression::Literal(naga::Literal::U32(1)),
        span,
    );
    let four_i32 = expressions.append(
        naga::Expression::Literal(naga::Literal::I32(4)),
        span,
    );
    let one_i32 = expressions.append(
        naga::Expression::Literal(naga::Literal::I32(1)),
        span,
    );
    let zero_f32 = expressions.append(
        naga::Expression::Literal(naga::Literal::F32(0.0)),
        span,
    );
    let one_f32 = expressions.append(
        naga::Expression::Literal(naga::Literal::F32(1.0)),
        span,
    );

    let mut emitter = naga::proc::Emitter::default();
    emitter.start(&expressions);

    let and_expr = expressions.append(
        naga::Expression::Binary {
            op: naga::BinaryOperator::And,
            left: vertex_index_expr,
            right: one_u32,
        },
        span,
    );

    let and_as_i32 = expressions.append(
        naga::Expression::As {
            expr: and_expr,
            kind: naga::ScalarKind::Sint,
            convert: Some(4),
        },
        span,
    );

    let mul_x = expressions.append(
        naga::Expression::Binary {
            op: naga::BinaryOperator::Multiply,
            left: and_as_i32,
            right: four_i32,
        },
        span,
    );

    let sub_x = expressions.append(
        naga::Expression::Binary {
            op: naga::BinaryOperator::Subtract,
            left: mul_x,
            right: one_i32,
        },
        span,
    );

    let x_expr = expressions.append(
        naga::Expression::As {
            expr: sub_x,
            kind: naga::ScalarKind::Float,
            convert: Some(4),
        },
        span,
    );

    let shr_expr = expressions.append(
        naga::Expression::Binary {
            op: naga::BinaryOperator::ShiftRight,
            left: vertex_index_expr,
            right: one_u32,
        },
        span,
    );

    let shr_as_i32 = expressions.append(
        naga::Expression::As {
            expr: shr_expr,
            kind: naga::ScalarKind::Sint,
            convert: Some(4),
        },
        span,
    );

    let mul_y = expressions.append(
        naga::Expression::Binary {
            op: naga::BinaryOperator::Multiply,
            left: shr_as_i32,
            right: four_i32,
        },
        span,
    );

    let sub_y = expressions.append(
        naga::Expression::Binary {
            op: naga::BinaryOperator::Subtract,
            left: mul_y,
            right: one_i32,
        },
        span,
    );

    let y_expr = expressions.append(
        naga::Expression::As {
            expr: sub_y,
            kind: naga::ScalarKind::Float,
            convert: Some(4),
        },
        span,
    );

    let result_expr = expressions.append(
        naga::Expression::Compose {
            ty: vec4f,
            components: vec![x_expr, y_expr, zero_f32, one_f32],
        },
        span,
    );

    let emit_result = emitter.finish(&expressions);

    let mut body = naga::Block::new();
    if let Some((stmt, span)) = emit_result {
        body.push(stmt, span);
    }
    body.push(
        naga::Statement::Return {
            value: Some(result_expr),
        },
        span,
    );

    let function = naga::Function {
        name: Some("vs_postprocess".to_string()),
        arguments: vec![naga::FunctionArgument {
            name: Some("vertex_index".to_string()),
            ty: u32_ty,
            binding: Some(naga::Binding::BuiltIn(naga::BuiltIn::VertexIndex)),
        }],
        result: Some(naga::FunctionResult {
            ty: vec4f,
            binding: Some(naga::Binding::BuiltIn(naga::BuiltIn::Position {
                invariant: false,
            })),
        }),
        local_variables: naga::Arena::new(),
        expressions,
        named_expressions: naga::FastIndexMap::default(),
        body,
        diagnostic_filter_leaf: None,
    };

    module.entry_points.push(naga::EntryPoint {
        name: "vs_postprocess".to_string(),
        stage: naga::ShaderStage::Vertex,
        early_depth_test: None,
        workgroup_size: [0; 3],
        workgroup_size_overrides: None,
        function,
    });
}

fn rename_entry_point(module: &mut naga::Module) {
    for ep in module.entry_points.iter_mut() {
        if ep.name == "main" {
            ep.name = "fs_postprocess".to_string();
            if let Some(ref mut name) = ep.function.name {
                if name == "main" {
                    *name = "fs_postprocess".to_string();
                }
            }
        }
    }
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
    }
}