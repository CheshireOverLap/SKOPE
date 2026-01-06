//! SKOPE Build Script
//!
//! 셰이더 전처리: #include 지시문 처리

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    // 셰이더 디렉토리 감시
    println!("cargo:rerun-if-changed=src/shaders/");

    // 출력 디렉토리
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let shader_out = PathBuf::from(&out_dir).join("shaders");
    fs::create_dir_all(&shader_out).unwrap();

    // 전처리할 셰이더 목록
    let shaders_to_process = [
        "material_eval.wgsl",
        "visibility.wgsl",
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
                let full_path = if include_path.starts_with('/') {
                    // 절대 경로 (base_dir 기준)
                    base_dir.join(&include_path[1..])
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
    } else if let Some(path) = rest.strip_prefix('<').and_then(|s| s.strip_suffix('>')) {
        Some(format!("/{}", path))
    } else {
        None
    }
}
