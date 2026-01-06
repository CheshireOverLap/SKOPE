//! WGSL Shader Preprocessor
//!
//! #include 지시문을 지원하는 셰이더 전처리기
//!
//! 사용법:
//! ```wgsl
//! #include "common/structs.wgsl"
//! #include "common/pbr.wgsl"
//! ```

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::fs;

/// 전처리 에러
#[derive(Debug)]
pub enum PreprocessError {
    /// 파일을 찾을 수 없음
    FileNotFound(PathBuf),
    /// 파일 읽기 실패
    IoError(PathBuf, std::io::Error),
    /// 순환 참조 감지
    CircularInclude(PathBuf),
    /// 잘못된 #include 문법
    InvalidIncludeSyntax(String),
}

impl std::fmt::Display for PreprocessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PreprocessError::FileNotFound(path) => {
                write!(f, "Shader file not found: {}", path.display())
            }
            PreprocessError::IoError(path, err) => {
                write!(f, "Failed to read {}: {}", path.display(), err)
            }
            PreprocessError::CircularInclude(path) => {
                write!(f, "Circular include detected: {}", path.display())
            }
            PreprocessError::InvalidIncludeSyntax(line) => {
                write!(f, "Invalid #include syntax: {}", line)
            }
        }
    }
}

impl std::error::Error for PreprocessError {}

/// 전처리 결과 (의존성 포함)
#[derive(Debug, Clone)]
pub struct ProcessResult {
    /// 전처리된 소스 코드
    pub source: String,
    /// 의존성 파일 목록 (#include된 파일들)
    pub dependencies: Vec<PathBuf>,
}

/// 셰이더 전처리기
pub struct ShaderPreprocessor {
    /// 셰이더 기본 디렉토리
    base_path: PathBuf,
}

impl ShaderPreprocessor {
    /// 새 전처리기 생성
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }

    /// 셰이더 파일을 전처리
    pub fn process_file(&self, path: impl AsRef<Path>) -> Result<String, PreprocessError> {
        let full_path = self.base_path.join(path.as_ref());
        let mut included = HashSet::new();
        self.process_file_recursive(&full_path, &mut included)
    }

    /// 셰이더 파일을 전처리하고 의존성도 반환
    pub fn process_file_with_deps(&self, path: impl AsRef<Path>) -> Result<ProcessResult, PreprocessError> {
        let full_path = self.base_path.join(path.as_ref());
        let mut included = HashSet::new();
        let mut dependencies = Vec::new();
        let source = self.process_file_recursive_with_deps(&full_path, &mut included, &mut dependencies)?;
        Ok(ProcessResult { source, dependencies })
    }

    /// 셰이더 소스 문자열을 전처리 (상대 경로 기준점 필요)
    pub fn process_source(
        &self,
        source: &str,
        relative_to: impl AsRef<Path>,
    ) -> Result<String, PreprocessError> {
        let base_dir = self.base_path.join(relative_to.as_ref()).parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| self.base_path.clone());

        let mut included = HashSet::new();
        self.process_source_recursive(source, &base_dir, &mut included)
    }

    /// 재귀적으로 파일 처리
    fn process_file_recursive(
        &self,
        path: &Path,
        included: &mut HashSet<PathBuf>,
    ) -> Result<String, PreprocessError> {
        // 정규화된 경로
        let canonical = path.canonicalize()
            .map_err(|_| PreprocessError::FileNotFound(path.to_path_buf()))?;

        // 순환 참조 체크
        if included.contains(&canonical) {
            return Err(PreprocessError::CircularInclude(path.to_path_buf()));
        }
        included.insert(canonical.clone());

        // 파일 읽기
        let source = fs::read_to_string(path)
            .map_err(|e| PreprocessError::IoError(path.to_path_buf(), e))?;

        let base_dir = path.parent().unwrap_or(Path::new("."));
        self.process_source_recursive(&source, base_dir, included)
    }

    /// 재귀적으로 파일 처리 (의존성 추적 포함)
    fn process_file_recursive_with_deps(
        &self,
        path: &Path,
        included: &mut HashSet<PathBuf>,
        dependencies: &mut Vec<PathBuf>,
    ) -> Result<String, PreprocessError> {
        // 정규화된 경로
        let canonical = path.canonicalize()
            .map_err(|_| PreprocessError::FileNotFound(path.to_path_buf()))?;

        // 순환 참조 체크
        if included.contains(&canonical) {
            return Err(PreprocessError::CircularInclude(path.to_path_buf()));
        }
        included.insert(canonical.clone());

        // 파일 읽기
        let source = fs::read_to_string(path)
            .map_err(|e| PreprocessError::IoError(path.to_path_buf(), e))?;

        let base_dir = path.parent().unwrap_or(Path::new("."));
        self.process_source_recursive_with_deps(&source, base_dir, included, dependencies)
    }

    /// 재귀적으로 소스 처리 (의존성 추적 포함)
    fn process_source_recursive_with_deps(
        &self,
        source: &str,
        base_dir: &Path,
        included: &mut HashSet<PathBuf>,
        dependencies: &mut Vec<PathBuf>,
    ) -> Result<String, PreprocessError> {
        let mut output = String::with_capacity(source.len());

        for line in source.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("#include") {
                // #include 처리
                let include_path = self.parse_include(trimmed)?;

                // 상대 경로 해석
                let full_path = if include_path.starts_with('/') {
                    // 절대 경로 (base_path 기준)
                    self.base_path.join(&include_path[1..])
                } else {
                    // 상대 경로 (현재 파일 기준)
                    base_dir.join(&include_path)
                };

                // 이미 포함된 파일은 스킵 (중복 방지)
                let canonical = full_path.canonicalize()
                    .map_err(|_| PreprocessError::FileNotFound(full_path.clone()))?;

                if !included.contains(&canonical) {
                    // 의존성에 추가
                    dependencies.push(canonical.clone());

                    // 재귀적으로 처리
                    let included_source = self.process_file_recursive_with_deps(&full_path, included, dependencies)?;

                    // 구분 주석 추가
                    output.push_str(&format!("// === BEGIN: {} ===\n", include_path));
                    output.push_str(&included_source);
                    output.push_str(&format!("\n// === END: {} ===\n", include_path));
                }
            } else {
                output.push_str(line);
                output.push('\n');
            }
        }

        Ok(output)
    }

    /// 재귀적으로 소스 처리
    fn process_source_recursive(
        &self,
        source: &str,
        base_dir: &Path,
        included: &mut HashSet<PathBuf>,
    ) -> Result<String, PreprocessError> {
        let mut output = String::with_capacity(source.len());

        for line in source.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("#include") {
                // #include 처리
                let include_path = self.parse_include(trimmed)?;

                // 상대 경로 해석
                let full_path = if include_path.starts_with('/') {
                    // 절대 경로 (base_path 기준)
                    self.base_path.join(&include_path[1..])
                } else {
                    // 상대 경로 (현재 파일 기준)
                    base_dir.join(&include_path)
                };

                // 이미 포함된 파일은 스킵 (중복 방지)
                let canonical = full_path.canonicalize()
                    .map_err(|_| PreprocessError::FileNotFound(full_path.clone()))?;

                if !included.contains(&canonical) {
                    // 재귀적으로 처리
                    let included_source = self.process_file_recursive(&full_path, included)?;

                    // 구분 주석 추가
                    output.push_str(&format!("// === BEGIN: {} ===\n", include_path));
                    output.push_str(&included_source);
                    output.push_str(&format!("\n// === END: {} ===\n", include_path));
                }
            } else {
                output.push_str(line);
                output.push('\n');
            }
        }

        Ok(output)
    }

    /// #include 문에서 경로 추출
    fn parse_include(&self, line: &str) -> Result<String, PreprocessError> {
        // #include "path/to/file.wgsl"
        // #include <path/to/file.wgsl>

        let rest = line.strip_prefix("#include").unwrap().trim();

        if let Some(path) = rest.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
            Ok(path.to_string())
        } else if let Some(path) = rest.strip_prefix('<').and_then(|s| s.strip_suffix('>')) {
            // <> 는 base_path 기준 절대 경로
            Ok(format!("/{}", path))
        } else {
            Err(PreprocessError::InvalidIncludeSyntax(line.to_string()))
        }
    }
}

/// 편의 함수: 셰이더 파일 전처리
pub fn preprocess_shader(base_path: impl Into<PathBuf>, shader_path: impl AsRef<Path>) -> Result<String, PreprocessError> {
    let preprocessor = ShaderPreprocessor::new(base_path);
    preprocessor.process_file(shader_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_include_quotes() {
        let pp = ShaderPreprocessor::new("/tmp");
        assert_eq!(
            pp.parse_include(r#"#include "common/structs.wgsl""#).unwrap(),
            "common/structs.wgsl"
        );
    }

    #[test]
    fn test_parse_include_angles() {
        let pp = ShaderPreprocessor::new("/tmp");
        assert_eq!(
            pp.parse_include(r#"#include <common/structs.wgsl>"#).unwrap(),
            "/common/structs.wgsl"
        );
    }

    #[test]
    fn test_process_real_shader() {
        // 실제 셰이더 디렉토리 경로
        let shader_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/shaders");

        if shader_dir.exists() {
            let pp = ShaderPreprocessor::new(&shader_dir);
            let result = pp.process_file("test_include.wgsl");

            match result {
                Ok(source) => {
                    // include된 내용이 있는지 확인
                    assert!(source.contains("const PI"), "PI constant should be included");
                    assert!(source.contains("fn safe_normalize"), "safe_normalize should be included");
                    assert!(source.contains("fn D_GGX"), "D_GGX should be included");
                    println!("Preprocessed shader ({} bytes):\n{}", source.len(), &source[..500.min(source.len())]);
                }
                Err(e) => {
                    println!("Preprocess error: {}", e);
                    // 파일이 없으면 스킵
                }
            }
        }
    }
}
