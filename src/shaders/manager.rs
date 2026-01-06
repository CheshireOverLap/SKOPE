//! Shader Manager
//!
//! 셰이더 로딩, 캐싱, 핫 리로드 관리

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use super::preprocessor::{ShaderPreprocessor, PreprocessError};

/// 셰이더 관리자
pub struct ShaderManager {
    /// 기본 셰이더 디렉토리
    base_path: PathBuf,
    /// 전처리기
    preprocessor: ShaderPreprocessor,
    /// 캐시된 셰이더 모듈
    cache: HashMap<String, CachedShader>,
    /// wgpu 디바이스 (셰이더 컴파일용)
    device: Arc<wgpu::Device>,
}

/// 캐시된 셰이더 정보
struct CachedShader {
    /// 컴파일된 셰이더 모듈
    module: wgpu::ShaderModule,
    /// 전처리된 소스 (디버깅용)
    source: String,
    /// 마지막 수정 시간
    modified: SystemTime,
    /// 의존 파일들 (include된 파일)
    dependencies: Vec<PathBuf>,
}

/// 셰이더 로드 결과
pub struct LoadedShader {
    pub module: wgpu::ShaderModule,
    pub source: String,
}

impl ShaderManager {
    /// 새 셰이더 관리자 생성
    pub fn new(device: Arc<wgpu::Device>, base_path: impl Into<PathBuf>) -> Self {
        let base_path = base_path.into();
        Self {
            preprocessor: ShaderPreprocessor::new(&base_path),
            base_path,
            cache: HashMap::new(),
            device,
        }
    }

    /// 셰이더 로드 (캐시 사용)
    pub fn load(&mut self, name: &str, path: impl AsRef<Path>) -> Result<&wgpu::ShaderModule, ShaderError> {
        let path = path.as_ref();

        // 캐시 체크
        if let Some(cached) = self.cache.get(name) {
            // 파일 변경 체크 (핫 리로드)
            if !self.is_modified(path, &cached.dependencies, cached.modified) {
                return Ok(&self.cache.get(name).unwrap().module);
            }
            log::info!("[ShaderManager] Reloading modified shader: {}", name);
        }

        // 전처리
        let source = self.preprocessor.process_file(path)
            .map_err(|e| ShaderError::Preprocess(e))?;

        // 의존성 추출 (나중에 핫 리로드용)
        let dependencies = self.extract_dependencies(&source, path);

        // 컴파일
        let module = self.compile(name, &source)?;

        // 캐시 저장
        let modified = std::fs::metadata(self.base_path.join(path))
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);

        self.cache.insert(name.to_string(), CachedShader {
            module,
            source,
            modified,
            dependencies,
        });

        Ok(&self.cache.get(name).unwrap().module)
    }

    /// 셰이더 직접 로드 (캐시 없이, 소스도 반환)
    pub fn load_direct(&self, path: impl AsRef<Path>) -> Result<LoadedShader, ShaderError> {
        let path = path.as_ref();

        // 전처리
        let source = self.preprocessor.process_file(path)
            .map_err(|e| ShaderError::Preprocess(e))?;

        // 컴파일
        let label = path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("shader");

        let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(label),
            source: wgpu::ShaderSource::Wgsl(source.clone().into()),
        });

        Ok(LoadedShader { module, source })
    }

    /// 인라인 소스로 셰이더 로드 (내장 셰이더용)
    pub fn load_inline(&mut self, name: &str, source: &str) -> Result<&wgpu::ShaderModule, ShaderError> {
        if self.cache.contains_key(name) {
            return Ok(&self.cache.get(name).unwrap().module);
        }

        let module = self.compile(name, source)?;

        self.cache.insert(name.to_string(), CachedShader {
            module,
            source: source.to_string(),
            modified: SystemTime::now(),
            dependencies: vec![],
        });

        Ok(&self.cache.get(name).unwrap().module)
    }

    /// 모든 변경된 셰이더 리로드 (핫 리로드)
    pub fn reload_modified(&mut self) -> Vec<String> {
        let mut reloaded = vec![];

        // 변경된 셰이더 찾기
        let to_reload: Vec<(String, PathBuf)> = self.cache.iter()
            .filter_map(|(name, cached)| {
                // 의존성에서 경로 추출
                cached.dependencies.first().map(|p| {
                    if self.is_any_modified(&cached.dependencies, cached.modified) {
                        Some((name.clone(), p.clone()))
                    } else {
                        None
                    }
                }).flatten()
            })
            .collect();

        // 리로드
        for (name, path) in to_reload {
            if let Ok(_) = self.load(&name, &path) {
                reloaded.push(name);
            }
        }

        reloaded
    }

    /// 캐시된 셰이더 소스 가져오기 (디버깅용)
    pub fn get_source(&self, name: &str) -> Option<&str> {
        self.cache.get(name).map(|c| c.source.as_str())
    }

    /// 셰이더 컴파일
    fn compile(&self, name: &str, source: &str) -> Result<wgpu::ShaderModule, ShaderError> {
        // naga로 먼저 검증 (더 좋은 에러 메시지)
        #[cfg(feature = "shader_validation")]
        {
            use naga::front::wgsl;
            if let Err(e) = wgsl::parse_str(source) {
                return Err(ShaderError::Parse(format!("{}: {:?}", name, e)));
            }
        }

        // wgpu 모듈 생성
        let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(name),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });

        log::debug!("[ShaderManager] Compiled shader: {}", name);
        Ok(module)
    }

    /// 파일 수정 여부 확인
    fn is_modified(&self, main_path: &Path, dependencies: &[PathBuf], cached_time: SystemTime) -> bool {
        // 메인 파일 체크
        let main_full = self.base_path.join(main_path);
        if let Ok(meta) = std::fs::metadata(&main_full) {
            if let Ok(modified) = meta.modified() {
                if modified > cached_time {
                    return true;
                }
            }
        }

        // 의존 파일들 체크
        self.is_any_modified(dependencies, cached_time)
    }

    fn is_any_modified(&self, paths: &[PathBuf], cached_time: SystemTime) -> bool {
        for path in paths {
            if let Ok(meta) = std::fs::metadata(path) {
                if let Ok(modified) = meta.modified() {
                    if modified > cached_time {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// 소스에서 의존성 추출
    fn extract_dependencies(&self, _source: &str, main_path: &Path) -> Vec<PathBuf> {
        // TODO: 전처리 과정에서 실제 include된 파일 목록 수집
        // 지금은 메인 파일만
        vec![self.base_path.join(main_path)]
    }
}

/// 셰이더 에러
#[derive(Debug)]
pub enum ShaderError {
    Preprocess(PreprocessError),
    Parse(String),
    Compile(String),
}

impl std::fmt::Display for ShaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShaderError::Preprocess(e) => write!(f, "Preprocess error: {}", e),
            ShaderError::Parse(e) => write!(f, "Parse error: {}", e),
            ShaderError::Compile(e) => write!(f, "Compile error: {}", e),
        }
    }
}

impl std::error::Error for ShaderError {}
