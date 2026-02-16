//! SKOPE Build Script
//!
//! 셰이더 전처리 및 임베딩:
//! - #include 지시문 처리
//! - Release 빌드용 셰이더 임베딩 생성

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    // 셰이더 디렉토리 감시
    println!("cargo:rerun-if-changed=src/shaders/");
    println!("cargo:rerun-if-changed=engine_assets/shaders/");

    // 출력 디렉토리
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let shader_out = PathBuf::from(&out_dir).join("shaders");
    fs::create_dir_all(&shader_out).unwrap();

    // 전처리할 셰이더 목록 (레거시)
    let shaders_to_process = [
        "material_eval.wgsl",
        "visibility.wgsl",
        "zprepass.wgsl",
        "taa.wgsl",
        "motion_vectors.wgsl",
        "hzb.wgsl",
        "debug_draw.wgsl",
    ];

    let shader_dir = PathBuf::from("src/shaders");

    for shader in &shaders_to_process {
        let input_path = shader_dir.join(shader);
        let output_path = shader_out.join(shader);

        if input_path.exists() {
            match preprocess_shader(&input_path, &shader_dir) {
                Ok(processed) => {
                    fs::write(&output_path, &processed).unwrap();
                    println!("cargo:warning=Preprocessed shader: {}", shader);
                }
                Err(e) => {
                    println!("cargo:warning=Failed to preprocess {}: {}", shader, e);
                    // 실패 시 원본 복사
                    if let Ok(original) = fs::read_to_string(&input_path) {
                        fs::write(&output_path, &original).unwrap();
                    }
                }
            }
        }
    }

    // 셰이더 임베딩 생성 (Release 빌드용)
    generate_shader_embeddings(&out_dir);
}

/// 셰이더 전처리 (#include 처리)
fn preprocess_shader(path: &Path, base_dir: &Path) -> Result<String, String> {
    let mut included = HashSet::new();
    process_file(path, base_dir, &mut included)
}

fn process_file(path: &Path, base_dir: &Path, included: &mut HashSet<PathBuf>) -> Result<String, String> {
    let canonical = path.canonicalize()
        .map_err(|e| format!("Cannot find file {}: {}", path.display(), e))?;

    // 순환 참조 체크
    if included.contains(&canonical) {
        return Ok(String::new()); // 이미 포함됨, 스킵
    }
    included.insert(canonical);

    let source = fs::read_to_string(path)
        .map_err(|e| format!("Cannot read {}: {}", path.display(), e))?;

    let current_dir = path.parent().unwrap_or(Path::new("."));
    let mut output = String::with_capacity(source.len() * 2);

    for line in source.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("#include") {
            // #include 파싱
            if let Some(include_path) = parse_include(trimmed) {
                let full_path = if let Some(stripped) = include_path.strip_prefix('/') {
                    // 절대 경로 (base_dir 기준)
                    base_dir.join(stripped)
                } else {
                    // 상대 경로 (현재 파일 기준)
                    current_dir.join(&include_path)
                };

                // 재귀적으로 처리
                let included_source = process_file(&full_path, base_dir, included)?;

                output.push_str(&format!("// === BEGIN: {} ===\n", include_path));
                output.push_str(&included_source);
                output.push_str(&format!("// === END: {} ===\n", include_path));
            } else {
                // 파싱 실패 시 원본 유지
                output.push_str(line);
                output.push('\n');
            }
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }

    Ok(output)
}

fn parse_include(line: &str) -> Option<String> {
    let rest = line.strip_prefix("#include")?.trim();

    if let Some(path) = rest.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        Some(path.to_string())
    } else { rest.strip_prefix('<').and_then(|s| s.strip_suffix('>')).map(|path| format!("/{}", path)) }
}

/// 셰이더 임베딩 코드 생성
///
/// engine_assets/shaders/ 디렉토리의 모든 .wgsl 파일을 읽어
/// ShaderId → 소스 매핑 함수를 생성합니다.
fn generate_shader_embeddings(out_dir: &str) {
    let embed_path = PathBuf::from(out_dir).join("shaders_embedded.rs");
    let engine_shaders = PathBuf::from("engine_assets/shaders");

    // engine_assets/shaders 디렉토리가 없으면 스킵
    if !engine_shaders.exists() {
        let fallback = r#"// 자동 생성된 셰이더 임베딩 (engine_assets/shaders 없음)

/// 임베딩된 셰이더 (engine_assets/shaders 없음)
pub fn get_embedded_shader(_id: ShaderId) -> &'static str {
    ""
}
"#;
        fs::write(&embed_path, fallback).unwrap();
        return;
    }

    // ShaderId → 파일 경로 매핑
    let shader_mappings = [
        // GBuffer
        ("ShaderId::Visibility", "gbuffer/visibility.wgsl"),
        ("ShaderId::MaterialEval", "gbuffer/material_eval.wgsl"),
        ("ShaderId::SkinnedMesh", "gbuffer/skinned_mesh.wgsl"),
        ("ShaderId::Shader", "gbuffer/shader.wgsl"),
        ("ShaderId::DebugDraw", "gbuffer/debug_draw.wgsl"),
        // Lighting
        ("ShaderId::ShadowDepth", "lighting/shadow_depth.wgsl"),
        ("ShaderId::ShadowSampling", "lighting/shadow_sampling.wgsl"),
        ("ShaderId::IblPrefilter", "lighting/ibl_prefilter.wgsl"),
        ("ShaderId::ClusterCull", "lighting/cluster_cull.wgsl"),
        ("ShaderId::CharacterLighting", "lighting/character_lighting.wgsl"),
        // Post
        ("ShaderId::BloomThreshold", "post/bloom_threshold.wgsl"),
        ("ShaderId::BloomDownsample", "post/bloom_downsample.wgsl"),
        ("ShaderId::BloomUpsample", "post/bloom_upsample.wgsl"),
        ("ShaderId::Tonemapping", "post/tonemapping.wgsl"),
        ("ShaderId::ColorGrading", "post/color_grading.wgsl"),
        ("ShaderId::Taa", "post/taa.wgsl"),
        ("ShaderId::Dof", "post/dof.wgsl"),
        ("ShaderId::MotionBlur", "post/motion_blur.wgsl"),
        ("ShaderId::Ssao", "post/ssao.wgsl"),
        ("ShaderId::FilmEffects", "post/film_effects.wgsl"),
        // Compute
        ("ShaderId::HistogramCompute", "compute/histogram_compute.wgsl"),
        ("ShaderId::HistogramAverage", "compute/histogram_average.wgsl"),
        ("ShaderId::SssBlur", "compute/sss_blur.wgsl"),
        // Effects
        ("ShaderId::Particle", "effects/particle.wgsl"),
        ("ShaderId::ParticleUpdate", "effects/particle_update.wgsl"),
        ("ShaderId::ParticleSpawn", "effects/particle_spawn.wgsl"),
        ("ShaderId::ParticleRender", "effects/gpu_particle_render.wgsl"),
        ("ShaderId::Flipbook", "effects/flipbook.wgsl"),
        ("ShaderId::Vat", "effects/vat.wgsl"),
        // Editor
        ("ShaderId::Gizmo", "editor/gizmo.wgsl"),
        ("ShaderId::Grid", "editor/grid.wgsl"),
        ("ShaderId::EditorUi", "editor/ui.wgsl"),
        ("ShaderId::EditorUiFont", "editor/ui_font.wgsl"),
        ("ShaderId::OutlineHull", "editor/outline_hull.wgsl"),
        ("ShaderId::OutlineEdgeDetect", "editor/outline_edge_detect.wgsl"),
        ("ShaderId::OutlineComposite", "editor/outline_composite.wgsl"),
        // Hair
        ("ShaderId::HairCard", "hair/hair_card.wgsl"),
        ("ShaderId::HairComposite", "hair/hair_composite.wgsl"),
        ("ShaderId::HairFlyaway", "hair/hair_flyaway_generate.wgsl"),
        ("ShaderId::HairStrandRasterize", "hair/hair_strand_rasterize.wgsl"),
        ("ShaderId::HairStrandSpawn", "hair/hair_strand_spawn.wgsl"),
        // Magic
        ("ShaderId::MagicCircle", "magic/magic_circle.wgsl"),
        ("ShaderId::SdfPrimitives", "magic/sdf_primitives.wgsl"),
        // UI
        ("ShaderId::GameUi", "ui/ui_shader.wgsl"),
        ("ShaderId::GameText", "ui/text_shader.wgsl"),
        // Common
        ("ShaderId::CommonConstants", "common/constants.wgsl"),
        ("ShaderId::CommonMath", "common/math.wgsl"),
        ("ShaderId::CommonPbr", "common/pbr.wgsl"),
        ("ShaderId::CommonShadow", "common/shadow.wgsl"),
        ("ShaderId::CommonStructs", "common/structs.wgsl"),
    ];

    let mut output = String::from(
        "// 자동 생성된 셰이더 임베딩 (build.rs)\n\n\
         /// 임베딩된 셰이더 소스 가져오기\n\
         #[allow(unreachable_patterns)]\n\
         pub fn get_embedded_shader(id: ShaderId) -> &'static str {\n    \
             match id {\n"
    );

    let mut found_count = 0;

    for (id, path) in &shader_mappings {
        let full_path = engine_shaders.join(path);
        if full_path.exists() {
            // include_str! 사용을 위해 CARGO_MANIFEST_DIR 기준 경로
            output.push_str(&format!(
                "        {} => include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/engine_assets/shaders/{}\")),\n",
                id, path
            ));
            found_count += 1;
        }
    }

    // 기본 케이스
    output.push_str("        _ => \"\",\n");
    output.push_str("    }\n}\n");

    fs::write(&embed_path, output).unwrap();
    println!("cargo:warning=Generated shader embeddings: {} shaders", found_count);
}
