//! Shader Hot Reload - File System Watcher
//!
//! 파일 시스템 이벤트 감시를 통한 WGSL 셰이더 핫 리로드

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

/// 셰이더 파일 감시기
pub struct ShaderWatcher {
    /// 내부 watcher (Drop 시 자동 해제)
    #[allow(dead_code)]
    watcher: RecommendedWatcher,
    /// 이벤트 수신기
    rx: Receiver<Result<Event, notify::Error>>,
    /// 감시 중인 경로
    watch_paths: Vec<PathBuf>,
    /// 변경된 파일들 (중복 제거)
    pending_changes: HashSet<PathBuf>,
}

/// 감시기 에러
#[derive(Debug)]
pub enum WatcherError {
    /// 초기화 실패
    InitFailed(String),
    /// 경로 추가 실패
    WatchFailed(String),
}

impl std::fmt::Display for WatcherError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WatcherError::InitFailed(msg) => write!(f, "Shader watcher init failed: {}", msg),
            WatcherError::WatchFailed(msg) => write!(f, "Watch path failed: {}", msg),
        }
    }
}

impl std::error::Error for WatcherError {}

impl ShaderWatcher {
    /// 새 셰이더 감시기 생성
    pub fn new() -> Result<Self, WatcherError> {
        let (tx, rx) = channel();

        // 커스텀 설정: 폴링 간격 500ms
        let config = Config::default()
            .with_poll_interval(Duration::from_millis(500));

        let watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            config,
        )
        .map_err(|e| WatcherError::InitFailed(e.to_string()))?;

        Ok(Self {
            watcher,
            rx,
            watch_paths: Vec::new(),
            pending_changes: HashSet::new(),
        })
    }

    /// 경로 감시 추가
    pub fn watch(&mut self, path: impl AsRef<Path>) -> Result<(), WatcherError> {
        let path = path.as_ref().to_path_buf();

        // 이미 감시 중인지 확인
        if self.watch_paths.contains(&path) {
            return Ok(());
        }

        self.watcher
            .watch(&path, RecursiveMode::Recursive)
            .map_err(|e| WatcherError::WatchFailed(e.to_string()))?;

        self.watch_paths.push(path);
        log::info!("[ShaderWatcher] Watching: {:?}", self.watch_paths.last());

        Ok(())
    }

    /// 경로 감시 해제
    #[allow(dead_code)]
    pub fn unwatch(&mut self, path: impl AsRef<Path>) -> Result<(), WatcherError> {
        let path = path.as_ref();

        self.watcher
            .unwatch(path)
            .map_err(|e| WatcherError::WatchFailed(e.to_string()))?;

        self.watch_paths.retain(|p| p != path);
        log::info!("[ShaderWatcher] Unwatched: {:?}", path);

        Ok(())
    }

    /// 변경된 파일들 폴링 (non-blocking)
    ///
    /// 변경된 .wgsl 파일들의 경로를 반환
    pub fn poll_changes(&mut self) -> Vec<PathBuf> {
        // 모든 pending 이벤트 수집
        while let Ok(result) = self.rx.try_recv() {
            if let Ok(event) = result {
                // 수정/생성 이벤트만 처리
                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) => {
                        for path in event.paths {
                            // .wgsl 파일만 필터링
                            if Self::is_wgsl_file(&path) {
                                self.pending_changes.insert(path);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // 결과 반환 및 버퍼 클리어
        let changes: Vec<PathBuf> = self.pending_changes.drain().collect();

        if !changes.is_empty() {
            log::info!("[ShaderWatcher] Detected {} changed shader(s)", changes.len());
        }

        changes
    }

    /// WGSL 파일인지 확인
    fn is_wgsl_file(path: &Path) -> bool {
        path.extension()
            .map(|ext| ext == "wgsl")
            .unwrap_or(false)
    }

    /// 감시 중인 경로 목록
    #[allow(dead_code)]
    pub fn watched_paths(&self) -> &[PathBuf] {
        &self.watch_paths
    }
}

impl Default for ShaderWatcher {
    fn default() -> Self {
        Self::new().expect("Failed to create ShaderWatcher")
    }
}
