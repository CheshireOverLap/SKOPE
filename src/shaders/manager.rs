//! Shader Manager
//!
//! 셰이더 로딩, 캐싱, 핫 리로드 관리
//!
//! # ShaderId 기반 사용법
//!
//! ```rust,ignore
//! let mut manager = ShaderManager::new(device.clone(), "engine_assets/shaders");
//!
//! // ShaderId로 로드
//! let shader = manager.get(ShaderId::BloomThreshold);
//!
//! // 핫리로드 (매 프레임)
//! let reloaded = manager.auto_reload();
//! for id in reloaded {
//!     // 파이프라인 리빌드
//! }
//! ```

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use super::preprocessor::{ShaderPreprocessor, PreprocessError};
use super::shader_id::ShaderId;
use super::watcher::ShaderWatcher;

/// 셰이더 관리자
pub struct ShaderManager {
    /// 기본 셰이더 디렉토리
    base_path: PathBuf,
    /// 전처리기
    preprocessor: ShaderPreprocessor,
    /// 캐시된 셰이더 모듈 (문자열 키)
    cache: HashMap<String, CachedShader>,
    /// ShaderId 기반 캐시
    id_cache: HashMap<ShaderId, wgpu::ShaderModule>,
    /// wgpu 디바이스 (셰이더 컴파일용)
    device: Arc<wgpu::Device>,
    /// Global shader defines (applied to all preprocessed shaders)
    global_defines: HashMap<String, Option<String>>,

    // === 핫리로드 (Debug 전용) ===
    #[cfg(debug_assertions)]
    watcher: Option<ShaderWatcher>,
    #[cfg(debug_assertions)]
    pending_reloads: HashSet<ShaderId>,
    #[cfg(debug_assertions)]
    last_reload: Instant,
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

        #[cfg(debug_assertions)]
        let watcher = Self::setup_watcher(&base_path);

        Self {
            preprocessor: ShaderPreprocessor::new(&base_path),
            base_path,
            cache: HashMap::new(),
            id_cache: HashMap::new(),
            device,
            global_defines: HashMap::new(),
            #[cfg(debug_assertions)]
            watcher,
            #[cfg(debug_assertions)]
            pending_reloads: HashSet::new(),
            #[cfg(debug_assertions)]
            last_reload: Instant::now(),
        }
    }

    /// Set a global shader define (applied to all preprocessed shaders).
    pub fn set_define(&mut self, name: &str, value: Option<&str>) {
        self.global_defines.insert(name.to_string(), value.map(|s| s.to_string()));
        self.preprocessor.define(name, value);
    }

    /// Remove a global shader define.
    pub fn remove_define(&mut self, name: &str) {
        self.global_defines.remove(name);
        self.preprocessor.undefine(name);
    }

    /// 파일 와처 설정 (Debug 빌드)
    #[cfg(debug_assertions)]
    fn setup_watcher(base_path: &Path) -> Option<ShaderWatcher> {
        match ShaderWatcher::new() {
            Ok(mut watcher) => {
                if let Err(e) = watcher.watch(base_path) {
                    log::warn!("[ShaderManager] Failed to watch shaders: {}", e);
                    return None;
                }
                log::info!("[ShaderManager] Hot-reload watcher started: {}", base_path.display());
                Some(watcher)
            }
            Err(e) => {
                log::warn!("[ShaderManager] Failed to create watcher: {}", e);
                None
            }
        }
    }

    // =========================================================================
    // ShaderId 기반 API
    // =========================================================================

    /// ShaderId로 셰이더 가져오기 (캐시 또는 로드)
    pub fn get(&mut self, id: ShaderId) -> &wgpu::ShaderModule {
        if !self.id_cache.contains_key(&id) {
            self.load_by_id(id);
        }
        self.id_cache.get(&id).unwrap()
    }

    /// ShaderId로 셰이더 로드
    fn load_by_id(&mut self, id: ShaderId) {
        let source = self.load_source_by_id(id);

        let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(id.name()),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });

        self.id_cache.insert(id, module);
        log::debug!("[ShaderManager] Loaded shader: {:?}", id);
    }

    /// 셰이더 소스 로드 (Debug: 파일, Release: 임베딩 폴백)
    fn load_source_by_id(&self, id: ShaderId) -> String {
        // Debug: 파일에서 로드 시도
        #[cfg(debug_assertions)]
        {
            let path = id.to_path();
            match std::fs::read_to_string(&path) {
                Ok(source) => {
                    log::trace!("[ShaderManager] Loaded from file: {}", path.display());
                    return source;
                }
                Err(e) => {
                    log::warn!("[ShaderManager] File load failed {:?}: {}, using fallback", path, e);
                }
            }
        }

        // Fallback: 기본 셰이더 또는 에러
        Self::fallback_source(id)
    }

    /// 폴백 셰이더 소스 (파일 로드 실패 시)
    ///
    /// Release 빌드: build.rs에서 생성된 임베딩 사용
    /// Debug 빌드: 마젠타 에러 셰이더 반환
    fn fallback_source(id: ShaderId) -> String {
        // 임베딩된 셰이더 시도
        let embedded = super::embedded::get_embedded_shader(id);
        if !embedded.is_empty() {
            return embedded.to_string();
        }

        // 임베딩도 없으면 최소한의 유효한 WGSL (마젠타 에러)
        format!(
            "// Fallback shader for {:?}\n// File not found: {}\n\n@vertex\nfn vs_main() -> @builtin(position) vec4<f32> {{\n    return vec4<f32>(0.0);\n}}\n\n@fragment\nfn fs_main() -> @location(0) vec4<f32> {{\n    return vec4<f32>(1.0, 0.0, 1.0, 1.0); // Magenta = error\n}}\n",
            id, id.to_path().display()
        )
    }

    // =========================================================================
    // 핫리로드 API
    // =========================================================================

    /// 매 프레임 호출 - 자동 리로드 (100ms debounce)
    #[cfg(debug_assertions)]
    pub fn auto_reload(&mut self) -> Vec<ShaderId> {
        // 파일 변경 감지
        if let Some(ref mut watcher) = self.watcher {
            let changed_paths = watcher.poll_changes();
            for path in changed_paths {
                if let Some(id) = ShaderId::from_path(&path) {
                    self.pending_reloads.insert(id);
                }
            }
        }

        if self.pending_reloads.is_empty() {
            return vec![];
        }

        // Debounce: 100ms
        if self.last_reload.elapsed() < Duration::from_millis(100) {
            return vec![];
        }

        self.flush_pending_reloads()
    }

    #[cfg(debug_assertions)]
    fn flush_pending_reloads(&mut self) -> Vec<ShaderId> {
        let mut reloaded = Vec::new();

        let pending: Vec<ShaderId> = self.pending_reloads.drain().collect();
        for id in pending {
            let path = id.to_path();
            match std::fs::read_to_string(&path) {
                Ok(source) => {
                    let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some(id.name()),
                        source: wgpu::ShaderSource::Wgsl(source.into()),
                    });
                    self.id_cache.insert(id, module);
                    reloaded.push(id);
                    log::info!("[ShaderManager] Reloaded: {:?}", id);
                }
                Err(e) => {
                    log::error!("[ShaderManager] Reload failed {:?}: {}", id, e);
                    // 기존 모듈 유지
                }
            }
        }

        self.last_reload = Instant::now();
        reloaded
    }

    /// 모든 셰이더 강제 리로드 (Shift+F5)
    #[cfg(debug_assertions)]
    pub fn force_reload_all(&mut self) -> Vec<ShaderId> {
        log::info!("[ShaderManager] Force reloading all shaders...");

        // 현재 캐시된 셰이더만 리로드
        for id in self.id_cache.keys().cloned().collect::<Vec<_>>() {
            self.pending_reloads.insert(id);
        }

        // 즉시 플러시
        self.last_reload = Instant::now() - Duration::from_secs(1);
        self.flush_pending_reloads()
    }

    /// 핫리로드 활성화 여부
    #[cfg(debug_assertions)]
    pub fn is_hot_reload_enabled(&self) -> bool {
        self.watcher.is_some()
    }

    // Release 빌드용 스텁
    #[cfg(not(debug_assertions))]
    pub fn auto_reload(&mut self) -> Vec<ShaderId> { vec![] }

    #[cfg(not(debug_assertions))]
    pub fn force_reload_all(&mut self) -> Vec<ShaderId> { vec![] }

    #[cfg(not(debug_assertions))]
    pub fn is_hot_reload_enabled(&self) -> bool { false }

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
            .map_err(ShaderError::Preprocess)?;

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
            .map_err(ShaderError::Preprocess)?;

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
                cached.dependencies.first().and_then(|p| {
                    if self.is_any_modified(&cached.dependencies, cached.modified) {
                        Some((name.clone(), p.clone()))
                    } else {
                        None
                    }
                })
            })
            .collect();

        // 리로드
        for (name, path) in to_reload {
            if self.load(&name, &path).is_ok() {
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
        // NOTE: shader_validation feature는 현재 미구현
        #[allow(unexpected_cfgs)]
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
