//! Editor Icon Manager
//!
//! PNG 아이콘을 로드하여 UI에서 사용 (skope_ui로 재구현 예정)

use std::collections::HashMap;

/// 아이콘 이름 상수
pub mod names {
    // 플레이 컨트롤
    pub const PLAY: &str = "play";
    pub const STOP: &str = "stop";
    pub const HOLD: &str = "hold";

    // 기즈모
    pub const MOVE: &str = "mov3";
    pub const ROTATE: &str = "turn";
    pub const SCALE: &str = "scale";

    // 그리드
    pub const GRID_ON: &str = "grid_toggle_on";
    pub const GRID_OFF: &str = "grid_toggle_off";

    // 씬/게임
    pub const SCENE: &str = "scene";
    pub const GAME: &str = "game";

    // 패널
    pub const HIERARCHY: &str = "hierachy";
    pub const INSPECTOR: &str = "Inspector";
    pub const CONSOLE: &str = "Console";
    pub const FOLDER: &str = "Folder";
    pub const OPEN_FOLDER: &str = "Open Folder";

    // 가시성
    pub const SEE: &str = "see";
    pub const HIDE: &str = "hide";

    // 토글
    pub const CHECK_ON: &str = "check_toggle_on";
    pub const CHECK_OFF: &str = "check_toggle_off";

    // 타이틀바
    pub const TITLEBAR_CLOSE: &str = "titlebar_x";
    pub const TITLEBAR_MAXIMIZE: &str = "titlebar_sizeup";
    pub const TITLEBAR_RESTORE: &str = "titlebar_sizedown";
    pub const TITLEBAR_MINIMIZE: &str = "titlebar_under";
}

/// 로드된 아이콘 정보 (Stub)
pub struct IconInfo {
    pub width: u32,
    pub height: u32,
}

/// 아이콘 매니저 (Stub - skope_ui로 재구현 예정)
pub struct IconManager {
    icons: HashMap<String, IconInfo>,
    icons_dir: String,
}

impl IconManager {
    pub fn new(icons_dir: &str) -> Self {
        Self {
            icons: HashMap::new(),
            icons_dir: icons_dir.to_string(),
        }
    }

    /// Stub: 아이콘 로드
    pub fn load_all<T>(
        &mut self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _renderer: &mut T,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("[IconManager] Stub - skope_ui로 재구현 예정");
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&IconInfo> {
        self.icons.get(name)
    }

    pub fn has(&self, name: &str) -> bool {
        self.icons.contains_key(name)
    }

    pub fn count(&self) -> usize {
        self.icons.len()
    }

    pub fn list(&self) -> Vec<&String> {
        self.icons.keys().collect()
    }
}

impl Default for IconManager {
    fn default() -> Self {
        Self::new("engine/icons")
    }
}
