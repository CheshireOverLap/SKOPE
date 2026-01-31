// SKOPE UI - Hot Reload System
// RON 파일 변경 시 실시간으로 UI 업데이트
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};
use std::collections::HashMap;
use std::fs;

use crate::{UiSystem, UiError};

/// 핫 리로드 감시자
pub struct HotReloader {
    /// 감시 중인 파일들
    watched_files: HashMap<PathBuf, FileState>,
    /// 감시 간격
    poll_interval: Duration,
    /// 마지막 체크 시간
    last_check: Instant,
    /// 활성화 여부
    enabled: bool,
}

#[derive(Debug)]
struct FileState {
    last_modified: SystemTime,
    load_error: Option<String>,
}

impl HotReloader {
    pub fn new() -> Self {
        Self {
            watched_files: HashMap::new(),
            poll_interval: Duration::from_millis(500), // 0.5초마다 체크
            last_check: Instant::now(),
            enabled: true,
        }
    }

    /// 감시 간격 설정
    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// 핫 리로드 활성화/비활성화
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// 파일 감시 시작
    pub fn watch<P: AsRef<Path>>(&mut self, path: P) -> Result<(), UiError> {
        let path = path.as_ref().to_path_buf();

        let metadata = fs::metadata(&path)
            .map_err(|e| UiError::IoError(format!("파일을 찾을 수 없음: {}", e)))?;

        let modified = metadata.modified()
            .map_err(|e| UiError::IoError(format!("수정 시간 확인 실패: {}", e)))?;

        self.watched_files.insert(path, FileState {
            last_modified: modified,
            load_error: None,
        });

        Ok(())
    }

    /// 파일 감시 중단
    pub fn unwatch<P: AsRef<Path>>(&mut self, path: P) {
        self.watched_files.remove(path.as_ref());
    }

    /// 모든 감시 중단
    pub fn unwatch_all(&mut self) {
        self.watched_files.clear();
    }

    /// 변경 확인 및 리로드 (매 프레임 호출)
    pub fn check_and_reload(&mut self, ui_system: &mut UiSystem) -> Vec<ReloadEvent> {
        if !self.enabled {
            return Vec::new();
        }

        // 폴링 간격 확인
        if self.last_check.elapsed() < self.poll_interval {
            return Vec::new();
        }
        self.last_check = Instant::now();

        let mut events = Vec::new();

        // 각 파일 확인
        let paths: Vec<_> = self.watched_files.keys().cloned().collect();

        for path in paths {
            if let Some(event) = self.check_file(&path, ui_system) {
                events.push(event);
            }
        }

        events
    }

    fn check_file(&mut self, path: &PathBuf, ui_system: &mut UiSystem) -> Option<ReloadEvent> {
        let metadata = match fs::metadata(path) {
            Ok(m) => m,
            Err(e) => {
                return Some(ReloadEvent::Error {
                    path: path.clone(),
                    error: format!("파일 접근 실패: {}", e),
                });
            }
        };

        let modified = match metadata.modified() {
            Ok(m) => m,
            Err(_) => return None,
        };

        let state = self.watched_files.get_mut(path)?;

        // 변경 감지
        if modified > state.last_modified {
            state.last_modified = modified;

            // 리로드 시도
            match ui_system.load_from_file(path) {
                Ok(()) => {
                    state.load_error = None;
                    log::info!("UI 리로드 성공: {:?}", path);
                    Some(ReloadEvent::Reloaded {
                        path: path.clone(),
                    })
                }
                Err(e) => {
                    let error = e.to_string();
                    state.load_error = Some(error.clone());
                    log::error!("UI 리로드 실패: {:?} - {}", path, error);
                    Some(ReloadEvent::Error {
                        path: path.clone(),
                        error,
                    })
                }
            }
        } else {
            None
        }
    }

    /// 현재 감시 중인 파일 목록
    pub fn watched_files(&self) -> Vec<&PathBuf> {
        self.watched_files.keys().collect()
    }

    /// 특정 파일의 에러 상태
    pub fn get_error(&self, path: &Path) -> Option<&str> {
        self.watched_files
            .get(path)
            .and_then(|s| s.load_error.as_deref())
    }
}

impl Default for HotReloader {
    fn default() -> Self {
        Self::new()
    }
}

/// 리로드 이벤트
#[derive(Debug, Clone)]
pub enum ReloadEvent {
    /// 성공적으로 리로드됨
    Reloaded { path: PathBuf },
    /// 리로드 실패
    Error { path: PathBuf, error: String },
}

/// 디렉토리 전체 감시 (재귀)
pub fn watch_directory<P: AsRef<Path>>(
    reloader: &mut HotReloader,
    dir: P,
    extension: &str,
) -> Result<usize, UiError> {
    let dir = dir.as_ref();
    let mut count = 0;

    if !dir.is_dir() {
        return Err(UiError::IoError("디렉토리가 아님".to_string()));
    }

    for entry in fs::read_dir(dir).map_err(|e| UiError::IoError(e.to_string()))? {
        let entry = entry.map_err(|e| UiError::IoError(e.to_string()))?;
        let path = entry.path();

        if path.is_dir() {
            // 재귀적으로 하위 디렉토리 감시
            count += watch_directory(reloader, &path, extension)?;
        } else if path.extension().map(|e| e == extension).unwrap_or(false) {
            reloader.watch(&path)?;
            count += 1;
        }
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn test_hot_reloader_creation() {
        let reloader = HotReloader::new();
        assert!(reloader.enabled);
        assert!(reloader.watched_files.is_empty());
    }
}
