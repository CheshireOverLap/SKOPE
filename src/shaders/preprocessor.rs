//! WGSL Shader Preprocessor
//!
//! #include, #define, #ifdef/#ifndef/#else/#endif 지시문을 지원하는 셰이더 전처리기
//!
//! 사용법:
//! ```wgsl
//! #define ENABLE_BLOOM
//! #define MAX_LIGHTS 128
//! #include "common/structs.wgsl"
//! #ifdef ENABLE_BLOOM
//!   // bloom code
//! #endif
//! ```

use std::collections::{HashMap, HashSet};
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
    /// #endif without matching #ifdef/#ifndef
    UnmatchedEndif,
    /// #else without matching #ifdef/#ifndef
    UnmatchedElse,
    /// #ifdef/#ifndef without closing #endif
    UnclosedIfdef(String),
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
            PreprocessError::UnmatchedEndif => {
                write!(f, "#endif without matching #ifdef/#ifndef")
            }
            PreprocessError::UnmatchedElse => {
                write!(f, "#else without matching #ifdef/#ifndef")
            }
            PreprocessError::UnclosedIfdef(name) => {
                write!(f, "Unclosed #ifdef/#ifndef for '{}'", name)
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
    /// 매크로 정의 (#define NAME 또는 #define NAME VALUE)
    defines: HashMap<String, Option<String>>,
}

impl ShaderPreprocessor {
    /// 새 전처리기 생성
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
            defines: HashMap::new(),
        }
    }

    /// 초기 defines를 포함한 전처리기 생성
    pub fn with_defines(base_path: impl Into<PathBuf>, defines: HashMap<String, Option<String>>) -> Self {
        Self {
            base_path: base_path.into(),
            defines,
        }
    }

    /// 매크로 정의 추가
    pub fn define(&mut self, name: &str, value: Option<&str>) {
        self.defines.insert(name.to_string(), value.map(|s| s.to_string()));
    }

    /// 매크로 정의 제거
    pub fn undefine(&mut self, name: &str) {
        self.defines.remove(name);
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
        let mut local_defines = self.defines.clone();
        let mut condition_stack: Vec<bool> = vec![];
        let mut active = true;

        for line in source.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("#define ") {
                if active {
                    let rest = trimmed.strip_prefix("#define ").unwrap().trim();
                    let (name, value) = if let Some(pos) = rest.find(' ') {
                        (&rest[..pos], Some(rest[pos + 1..].trim().to_string()))
                    } else {
                        (rest, None)
                    };
                    local_defines.insert(name.to_string(), value);
                }
                continue;
            }

            if trimmed.starts_with("#ifdef ") {
                let name = trimmed.strip_prefix("#ifdef ").unwrap().trim();
                condition_stack.push(active);
                active = active && local_defines.contains_key(name);
                continue;
            }

            if trimmed.starts_with("#ifndef ") {
                let name = trimmed.strip_prefix("#ifndef ").unwrap().trim();
                condition_stack.push(active);
                active = active && !local_defines.contains_key(name);
                continue;
            }

            if trimmed == "#else" {
                if condition_stack.is_empty() {
                    return Err(PreprocessError::UnmatchedElse);
                }
                let parent_active = *condition_stack.last().unwrap();
                active = parent_active && !active;
                continue;
            }

            if trimmed == "#endif" {
                if condition_stack.is_empty() {
                    return Err(PreprocessError::UnmatchedEndif);
                }
                active = condition_stack.pop().unwrap();
                continue;
            }

            if trimmed.starts_with("#include") && active {
                let include_path = self.parse_include(trimmed)?;

                let full_path = if let Some(stripped) = include_path.strip_prefix('/') {
                    self.base_path.join(stripped)
                } else {
                    base_dir.join(&include_path)
                };

                let canonical = full_path.canonicalize()
                    .map_err(|_| PreprocessError::FileNotFound(full_path.clone()))?;

                if !included.contains(&canonical) {
                    dependencies.push(canonical.clone());

                    let included_source = self.process_file_recursive_with_deps(&full_path, included, dependencies)?;

                    output.push_str(&format!("// === BEGIN: {} ===\n", include_path));
                    output.push_str(&included_source);
                    output.push_str(&format!("\n// === END: {} ===\n", include_path));
                }
            } else if active {
                let mut out_line = line.to_string();
                for (name, value) in &local_defines {
                    if let Some(val) = value {
                        out_line = replace_word_boundary(&out_line, name, val);
                    }
                }
                output.push_str(&out_line);
                output.push('\n');
            }
        }

        if !condition_stack.is_empty() {
            return Err(PreprocessError::UnclosedIfdef(format!("{} unclosed block(s)", condition_stack.len())));
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
        // Local copy of defines so #define in source is scoped per-file
        // but inherits from preprocessor-level defines
        let mut local_defines = self.defines.clone();
        // Stack for nested #ifdef/#ifndef: stores parent 'active' state
        let mut condition_stack: Vec<bool> = vec![];
        let mut active = true;

        for line in source.lines() {
            let trimmed = line.trim();

            // --- #define ---
            if trimmed.starts_with("#define ") {
                if active {
                    let rest = trimmed.strip_prefix("#define ").unwrap().trim();
                    let (name, value) = if let Some(pos) = rest.find(' ') {
                        (&rest[..pos], Some(rest[pos + 1..].trim().to_string()))
                    } else {
                        (rest, None)
                    };
                    local_defines.insert(name.to_string(), value);
                }
                continue;
            }

            // --- #ifdef ---
            if trimmed.starts_with("#ifdef ") {
                let name = trimmed.strip_prefix("#ifdef ").unwrap().trim();
                condition_stack.push(active);
                active = active && local_defines.contains_key(name);
                continue;
            }

            // --- #ifndef ---
            if trimmed.starts_with("#ifndef ") {
                let name = trimmed.strip_prefix("#ifndef ").unwrap().trim();
                condition_stack.push(active);
                active = active && !local_defines.contains_key(name);
                continue;
            }

            // --- #else ---
            if trimmed == "#else" {
                if condition_stack.is_empty() {
                    return Err(PreprocessError::UnmatchedElse);
                }
                let parent_active = *condition_stack.last().unwrap();
                // Only flip if parent is active; if parent is inactive, stay inactive
                active = parent_active && !active;
                continue;
            }

            // --- #endif ---
            if trimmed == "#endif" {
                if condition_stack.is_empty() {
                    return Err(PreprocessError::UnmatchedEndif);
                }
                active = condition_stack.pop().unwrap();
                continue;
            }

            // --- #include (only when active) ---
            if trimmed.starts_with("#include") && active {
                let include_path = self.parse_include(trimmed)?;

                let full_path = if let Some(stripped) = include_path.strip_prefix('/') {
                    self.base_path.join(stripped)
                } else {
                    base_dir.join(&include_path)
                };

                let canonical = full_path.canonicalize()
                    .map_err(|_| PreprocessError::FileNotFound(full_path.clone()))?;

                if !included.contains(&canonical) {
                    let included_source = self.process_file_recursive(&full_path, included)?;

                    output.push_str(&format!("// === BEGIN: {} ===\n", include_path));
                    output.push_str(&included_source);
                    output.push_str(&format!("\n// === END: {} ===\n", include_path));
                }
            } else if active {
                // Macro value substitution with word-boundary check
                let mut out_line = line.to_string();
                for (name, value) in &local_defines {
                    if let Some(val) = value {
                        out_line = replace_word_boundary(&out_line, name, val);
                    }
                }
                output.push_str(&out_line);
                output.push('\n');
            }
        }

        if !condition_stack.is_empty() {
            return Err(PreprocessError::UnclosedIfdef(format!("{} unclosed block(s)", condition_stack.len())));
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

/// Replace occurrences of `word` in `text` only at word boundaries.
/// A word boundary means the character before/after is not alphanumeric or underscore.
fn replace_word_boundary(text: &str, word: &str, replacement: &str) -> String {
    if word.is_empty() {
        return text.to_string();
    }
    let mut result = String::with_capacity(text.len());
    let text_bytes = text.as_bytes();
    let word_len = word.len();
    let mut i = 0;

    while i < text.len() {
        if i + word_len <= text.len() && &text[i..i + word_len] == word {
            // Check left boundary
            let left_ok = if i == 0 {
                true
            } else {
                let c = text_bytes[i - 1] as char;
                !c.is_alphanumeric() && c != '_'
            };
            // Check right boundary
            let right_ok = if i + word_len >= text.len() {
                true
            } else {
                let c = text_bytes[i + word_len] as char;
                !c.is_alphanumeric() && c != '_'
            };

            if left_ok && right_ok {
                result.push_str(replacement);
                i += word_len;
                continue;
            }
        }
        result.push(text_bytes[i] as char);
        i += 1;
    }
    result
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

    // ================================================================
    // #define / #ifdef / #ifndef / #else / #endif tests
    // ================================================================

    #[test]
    fn test_ifdef_defined() {
        let pp = ShaderPreprocessor::new("/tmp");
        let source = "#define ENABLE_BLOOM\n#ifdef ENABLE_BLOOM\nbloom_code();\n#endif\ncommon_code();\n";
        let result = pp.process_source(source, "test.wgsl").unwrap();
        assert!(result.contains("bloom_code();"));
        assert!(result.contains("common_code();"));
    }

    #[test]
    fn test_ifdef_undefined() {
        let pp = ShaderPreprocessor::new("/tmp");
        let source = "#ifdef ENABLE_BLOOM\nbloom_code();\n#endif\ncommon_code();\n";
        let result = pp.process_source(source, "test.wgsl").unwrap();
        assert!(!result.contains("bloom_code();"));
        assert!(result.contains("common_code();"));
    }

    #[test]
    fn test_ifndef() {
        let pp = ShaderPreprocessor::new("/tmp");
        let source = "#ifndef ENABLE_BLOOM\nno_bloom();\n#endif\n";
        let result = pp.process_source(source, "test.wgsl").unwrap();
        assert!(result.contains("no_bloom();"));

        // Now define it
        let source2 = "#define ENABLE_BLOOM\n#ifndef ENABLE_BLOOM\nno_bloom();\n#endif\nafter();\n";
        let result2 = pp.process_source(source2, "test.wgsl").unwrap();
        assert!(!result2.contains("no_bloom();"));
        assert!(result2.contains("after();"));
    }

    #[test]
    fn test_nested_ifdef() {
        let pp = ShaderPreprocessor::new("/tmp");
        let source = "\
#define A
#define B
#ifdef A
  outer();
  #ifdef B
    inner();
  #endif
#endif
";
        let result = pp.process_source(source, "test.wgsl").unwrap();
        assert!(result.contains("outer();"));
        assert!(result.contains("inner();"));

        // Inner false
        let source2 = "\
#define A
#ifdef A
  outer();
  #ifdef B
    inner();
  #endif
#endif
";
        let result2 = pp.process_source(source2, "test.wgsl").unwrap();
        assert!(result2.contains("outer();"));
        assert!(!result2.contains("inner();"));

        // Outer false => inner should also be excluded
        let source3 = "\
#define B
#ifdef A
  outer();
  #ifdef B
    inner();
  #endif
#endif
";
        let result3 = pp.process_source(source3, "test.wgsl").unwrap();
        assert!(!result3.contains("outer();"));
        assert!(!result3.contains("inner();"));
    }

    #[test]
    fn test_else() {
        let pp = ShaderPreprocessor::new("/tmp");
        let source = "\
#define FEATURE
#ifdef FEATURE
  with_feature();
#else
  without_feature();
#endif
";
        let result = pp.process_source(source, "test.wgsl").unwrap();
        assert!(result.contains("with_feature();"));
        assert!(!result.contains("without_feature();"));

        let source2 = "\
#ifdef FEATURE
  with_feature();
#else
  without_feature();
#endif
";
        let result2 = pp.process_source(source2, "test.wgsl").unwrap();
        assert!(!result2.contains("with_feature();"));
        assert!(result2.contains("without_feature();"));
    }

    #[test]
    fn test_else_nested_parent_false() {
        // When parent is inactive, #else should NOT flip to active
        let pp = ShaderPreprocessor::new("/tmp");
        let source = "\
#ifdef OUTER
  #ifdef INNER
    a();
  #else
    b();
  #endif
#endif
";
        let result = pp.process_source(source, "test.wgsl").unwrap();
        // OUTER is not defined, so nothing should be output
        assert!(!result.contains("a();"));
        assert!(!result.contains("b();"));
    }

    #[test]
    fn test_define_value_substitution() {
        let pp = ShaderPreprocessor::new("/tmp");
        let source = "#define MAX_LIGHTS 128\nlet count = MAX_LIGHTS;\n";
        let result = pp.process_source(source, "test.wgsl").unwrap();
        assert!(result.contains("let count = 128;"));
        assert!(!result.contains("MAX_LIGHTS"));
    }

    #[test]
    fn test_define_word_boundary() {
        let pp = ShaderPreprocessor::new("/tmp");
        // BLOOM should not replace BLOOM_THRESHOLD
        let source = "#define BLOOM 1\nlet a = BLOOM;\nlet b = BLOOM_THRESHOLD;\n";
        let result = pp.process_source(source, "test.wgsl").unwrap();
        assert!(result.contains("let a = 1;"));
        assert!(result.contains("let b = BLOOM_THRESHOLD;"));
    }

    #[test]
    fn test_external_defines() {
        let mut defines = HashMap::new();
        defines.insert("USE_NANITE".to_string(), None);
        defines.insert("MAX_CASCADES".to_string(), Some("4".to_string()));

        let pp = ShaderPreprocessor::with_defines("/tmp", defines);
        let source = "\
#ifdef USE_NANITE
  nanite_path();
#endif
let cascades = MAX_CASCADES;
";
        let result = pp.process_source(source, "test.wgsl").unwrap();
        assert!(result.contains("nanite_path();"));
        assert!(result.contains("let cascades = 4;"));
    }

    #[test]
    fn test_unmatched_endif() {
        let pp = ShaderPreprocessor::new("/tmp");
        let source = "#endif\n";
        let result = pp.process_source(source, "test.wgsl");
        assert!(matches!(result, Err(PreprocessError::UnmatchedEndif)));
    }

    #[test]
    fn test_unmatched_else() {
        let pp = ShaderPreprocessor::new("/tmp");
        let source = "#else\n";
        let result = pp.process_source(source, "test.wgsl");
        assert!(matches!(result, Err(PreprocessError::UnmatchedElse)));
    }

    #[test]
    fn test_unclosed_ifdef() {
        let pp = ShaderPreprocessor::new("/tmp");
        let source = "#ifdef FOO\ncode();\n";
        let result = pp.process_source(source, "test.wgsl");
        assert!(matches!(result, Err(PreprocessError::UnclosedIfdef(_))));
    }

    #[test]
    fn test_word_boundary_helper() {
        assert_eq!(replace_word_boundary("FOO + BAR", "FOO", "1"), "1 + BAR");
        assert_eq!(replace_word_boundary("FOOBAR", "FOO", "1"), "FOOBAR");
        assert_eq!(replace_word_boundary("_FOO", "FOO", "1"), "_FOO");
        assert_eq!(replace_word_boundary("FOO_BAR", "FOO", "1"), "FOO_BAR");
        assert_eq!(replace_word_boundary("(FOO)", "FOO", "1"), "(1)");
        assert_eq!(replace_word_boundary("FOO", "FOO", "1"), "1");
    }
}
