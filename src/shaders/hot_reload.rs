//! Shader Hot Reload System
//!
//! 런타임에 셰이더 파일 변경 감지 및 재컴파일

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::preprocessor::{PreprocessError, ProcessResult, ShaderPreprocessor};
use super::watcher::{ShaderWatcher, WatcherError};

/// 셰이더 핫 리로드 에러
#[derive(Debug)]
pub enum HotReloadError {
    /// 감시기 에러
    Watcher(WatcherError),
    /// 전처리 에러
    Preprocess(PreprocessError),
    /// 셰이더 컴파일 에러
    Compile(String),
    /// 파일 읽기 에러
    Io(std::io::Error),
}

impl std::fmt::Display for HotReloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HotReloadError::Watcher(e) => write!(f, "Watcher error: {}", e),
            HotReloadError::Preprocess(e) => write!(f, "Preprocess error: {}", e),
            HotReloadError::Compile(msg) => write!(f, "Shader compile error: {}", msg),
            HotReloadError::Io(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for HotReloadError {}

impl From<WatcherError> for HotReloadError {
    fn from(e: WatcherError) -> Self {
        HotReloadError::Watcher(e)
    }
}

impl From<PreprocessError> for HotReloadError {
    fn from(e: PreprocessError) -> Self {
        HotReloadError::Preprocess(e)
    }
}

/// 추적 중인 셰이더 정보
#[derive(Debug, Clone)]
struct TrackedShader {
    /// 메인 셰이더 파일 경로 (base_path 기준 상대 경로)
    relative_path: PathBuf,
    /// 마지막 컴파일 성공한 소스 (에러 복구용)
    last_valid_source: Option<String>,
    /// 의존성 파일들 (절대 경로)
    dependencies: Vec<PathBuf>,
    /// 마지막 수정 시간
    last_modified: Option<SystemTime>,
}

/// 셰이더 핫 리로드 관리자
pub struct ShaderHotReload {
    /// 파일 감시기
    watcher: Option<ShaderWatcher>,
    /// 셰이더 기본 디렉토리
    base_path: PathBuf,
    /// 전처리기
    preprocessor: ShaderPreprocessor,
    /// 추적 중인 셰이더들 (이름 → 정보)
    tracked_shaders: HashMap<String, TrackedShader>,
    /// 감시 시작 여부
    watching: bool,
}

impl ShaderHotReload {
    /// 새 핫 리로드 관리자 생성
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        let base_path = base_path.into();
        Self {
            watcher: None,
            preprocessor: ShaderPreprocessor::new(&base_path),
            base_path,
            tracked_shaders: HashMap::new(),
            watching: false,
        }
    }

    /// 파일 감시 시작
    pub fn start_watching(&mut self) -> Result<(), HotReloadError> {
        if self.watching {
            return Ok(());
        }

        let mut watcher = ShaderWatcher::new()?;
        watcher.watch(&self.base_path)?;

        log::info!(
            "[ShaderHotReload] Started watching: {}",
            self.base_path.display()
        );

        self.watcher = Some(watcher);
        self.watching = true;

        Ok(())
    }

    /// 셰이더 추적 등록
    ///
    /// # Arguments
    /// * `name` - 셰이더 식별자 (예: "material_eval", "vbuffer")
    /// * `relative_path` - base_path 기준 상대 경로
    pub fn track(&mut self, name: &str, relative_path: impl AsRef<Path>) {
        let relative_path = relative_path.as_ref().to_path_buf();

        // 의존성 수집 시도
        let (dependencies, last_modified) = match self.collect_shader_info(&relative_path) {
            Ok((deps, time)) => (deps, Some(time)),
            Err(e) => {
                log::warn!(
                    "[ShaderHotReload] Failed to collect info for {}: {}",
                    name,
                    e
                );
                (Vec::new(), None)
            }
        };

        log::debug!(
            "[ShaderHotReload] Tracking '{}': {} ({} deps)",
            name,
            relative_path.display(),
            dependencies.len()
        );

        self.tracked_shaders.insert(
            name.to_string(),
            TrackedShader {
                relative_path,
                last_valid_source: None,
                dependencies,
                last_modified,
            },
        );
    }

    /// 셰이더 정보 수집 (의존성 + 수정 시간)
    fn collect_shader_info(
        &self,
        relative_path: &Path,
    ) -> Result<(Vec<PathBuf>, SystemTime), HotReloadError> {
        let full_path = self.base_path.join(relative_path);

        // 수정 시간
        let metadata = std::fs::metadata(&full_path).map_err(HotReloadError::Io)?;
        let modified = metadata.modified().map_err(HotReloadError::Io)?;

        // 의존성 수집 (전처리 수행)
        let result = self.preprocessor.process_file_with_deps(relative_path)?;

        Ok((result.dependencies, modified))
    }

    /// 변경된 셰이더 확인 및 목록 반환
    ///
    /// non-blocking, 매 프레임 호출 가능
    pub fn check_changes(&mut self) -> Vec<String> {
        let changed_files = match &mut self.watcher {
            Some(w) => w.poll_changes(),
            None => return Vec::new(),
        };

        if changed_files.is_empty() {
            return Vec::new();
        }

        log::debug!(
            "[ShaderHotReload] Detected {} file change(s)",
            changed_files.len()
        );

        // 변경된 파일과 관련된 셰이더 찾기
        let mut changed_shaders = Vec::new();

        for (name, shader) in &self.tracked_shaders {
            let main_path = self.base_path.join(&shader.relative_path);

            // 메인 파일 변경됨?
            let main_canonical = main_path.canonicalize().ok();
            let main_changed = main_canonical
                .as_ref()
                .map(|c| changed_files.iter().any(|f| f == c))
                .unwrap_or(false);

            // 의존성 파일 변경됨?
            let dep_changed = shader
                .dependencies
                .iter()
                .any(|dep| changed_files.contains(dep));

            if main_changed || dep_changed {
                log::info!("[ShaderHotReload] Shader '{}' needs reload", name);
                changed_shaders.push(name.clone());
            }
        }

        changed_shaders
    }

    /// 셰이더 재로드 (전처리 + 컴파일)
    ///
    /// 성공 시 새 ShaderModule 반환, 실패 시 이전 버전 유지
    pub fn reload(
        &mut self,
        name: &str,
        device: &wgpu::Device,
    ) -> Result<wgpu::ShaderModule, HotReloadError> {
        let shader = self
            .tracked_shaders
            .get(name)
            .ok_or_else(|| HotReloadError::Compile(format!("Shader '{}' not tracked", name)))?;

        let relative_path = shader.relative_path.clone();

        // 전처리
        log::debug!("[ShaderHotReload] Preprocessing '{}'...", name);
        let result = self.preprocessor.process_file_with_deps(&relative_path)?;

        // 컴파일
        log::debug!("[ShaderHotReload] Compiling '{}'...", name);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(name),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(&result.source)),
        });

        // 성공 시 정보 업데이트
        if let Some(shader) = self.tracked_shaders.get_mut(name) {
            shader.last_valid_source = Some(result.source);
            shader.dependencies = result.dependencies;
            shader.last_modified = Some(SystemTime::now());
        }

        log::info!("[ShaderHotReload] Successfully reloaded '{}'", name);

        Ok(module)
    }

    /// 셰이더 소스 가져오기 (전처리 포함)
    ///
    /// 컴파일 없이 소스만 필요할 때 사용
    pub fn get_source(&self, name: &str) -> Result<String, HotReloadError> {
        let shader = self
            .tracked_shaders
            .get(name)
            .ok_or_else(|| HotReloadError::Compile(format!("Shader '{}' not tracked", name)))?;

        let result = self.preprocessor.process_file(&shader.relative_path)?;
        Ok(result)
    }

    /// 마지막으로 성공한 소스 가져오기
    #[allow(dead_code)]
    pub fn get_last_valid_source(&self, name: &str) -> Option<&str> {
        self.tracked_shaders
            .get(name)
            .and_then(|s| s.last_valid_source.as_deref())
    }

    /// 추적 중인 셰이더 목록
    #[allow(dead_code)]
    pub fn tracked_names(&self) -> Vec<&str> {
        self.tracked_shaders.keys().map(|s| s.as_str()).collect()
    }

    /// 핫 리로드 활성화 여부
    pub fn is_watching(&self) -> bool {
        self.watching
    }

    /// 베이스 경로 반환
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }
}
