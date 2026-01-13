//! SKOPE Material Hot Reload
//!
//! 파일 변경 감지 및 자동 리로드 (디버그 빌드 전용)

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use super::loader::MaterialLoader;
use super::registry::MaterialRegistry;

/// 머티리얼 핫 리로드 시스템 (디버그 빌드 전용)
pub struct MaterialHotReload {
    /// 파일 감시기
    watcher: Option<RecommendedWatcher>,
    /// 이벤트 수신기
    rx: Option<Receiver<notify::Result<Event>>>,
    /// 감시 디렉토리
    watch_path: PathBuf,
    /// 로더
    loader: MaterialLoader,
    /// 활성화 여부
    enabled: bool,
}

impl MaterialHotReload {
    /// 새 핫 리로드 시스템 생성
    pub fn new(material_path: impl Into<PathBuf>) -> Self {
        let watch_path = material_path.into();
        let loader = MaterialLoader::new(&watch_path);

        Self {
            watcher: None,
            rx: None,
            watch_path,
            loader,
            enabled: false,
        }
    }

    /// 파일 감시 시작
    pub fn start_watching(&mut self) -> Result<(), notify::Error> {
        if self.enabled {
            return Ok(());
        }

        if !self.watch_path.exists() {
            log::warn!(
                "[MaterialHotReload] Watch path does not exist: {}",
                self.watch_path.display()
            );
            return Ok(());
        }

        let (tx, rx) = mpsc::channel();

        let mut watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default().with_poll_interval(Duration::from_millis(500)),
        )?;

        watcher.watch(&self.watch_path, RecursiveMode::Recursive)?;

        log::info!(
            "[MaterialHotReload] Started watching: {}",
            self.watch_path.display()
        );

        self.watcher = Some(watcher);
        self.rx = Some(rx);
        self.enabled = true;

        Ok(())
    }

    /// 파일 감시 중지
    pub fn stop_watching(&mut self) {
        self.watcher = None;
        self.rx = None;
        self.enabled = false;
        log::info!("[MaterialHotReload] Stopped watching");
    }

    /// 활성화 여부
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// 변경 확인 및 리로드 (매 프레임 호출)
    pub fn check_and_reload(&mut self, registry: &mut MaterialRegistry) -> Vec<String> {
        let Some(ref rx) = self.rx else {
            return Vec::new();
        };

        let mut changed_materials = Vec::new();

        // Non-blocking 이벤트 수집
        while let Ok(Ok(event)) = rx.try_recv() {
            match event.kind {
                EventKind::Modify(_) | EventKind::Create(_) => {
                    for path in event.paths {
                        if Self::is_material_file(&path) {
                            match self.reload_material(&path, registry) {
                                Ok(name) => {
                                    log::info!("[MaterialHotReload] Reloaded: {}", name);
                                    changed_materials.push(name);
                                }
                                Err(e) => {
                                    log::warn!(
                                        "[MaterialHotReload] Failed to reload {:?}: {}",
                                        path,
                                        e
                                    );
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        changed_materials
    }

    /// .mat.ron 파일인지 확인
    fn is_material_file(path: &Path) -> bool {
        path.to_string_lossy().ends_with(".mat.ron")
    }

    /// 개별 머티리얼 리로드
    fn reload_material(
        &self,
        path: &Path,
        registry: &mut MaterialRegistry,
    ) -> Result<String, super::loader::MaterialLoadError> {
        let new_def = self.loader.load_file(path)?;
        let name = new_def.name.clone();

        // 기존 엔트리 업데이트 (GPU 인덱스 유지)
        if let Some(entry) = registry.get_mut(&name) {
            entry.def = new_def;
            entry.dirty = true;
        } else {
            // 새 머티리얼
            registry.register(new_def, Some(path.to_path_buf()));
        }

        Ok(name)
    }
}

impl Drop for MaterialHotReload {
    fn drop(&mut self) {
        self.stop_watching();
    }
}
