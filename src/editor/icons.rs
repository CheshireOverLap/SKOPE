//! Editor Icon Manager
//!
//! engine/icons/ 디렉토리의 PNG 아이콘을 관리하고
//! SlateApp에서 사용할 수 있도록 프리로드 목록을 생성합니다.

use std::collections::HashMap;
use std::path::Path;

/// 아이콘 이름 상수 — 위젯에서 `icon_filename(names::PLAY)` 등으로 참조
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

    // 이펙트 토글
    pub const EFFECT_ON: &str = "Toggle_effect_on";
    pub const EFFECT_OFF: &str = "Toggle_effect_off";
    pub const FOG_ON: &str = "Toggle_fog_on";
    pub const FOG_OFF: &str = "Toggle_fog_off";
    pub const LIGHT_ON: &str = "Toggle_light_on";
    pub const LIGHT_OFF: &str = "Toggle_light_off";
    pub const SKYBOX_ON: &str = "Toggle_skybox_on";
    pub const SKYBOX_OFF: &str = "Toggle_skybox_off";
    pub const SOUND_ON: &str = "Toggle_Sound_on";
    pub const SOUND_OFF: &str = "Toggle_Sound_off";

    // AI
    pub const AI: &str = "AI";
    pub const AI_CHAT: &str = "AIChat";
    pub const AI_MEMORY: &str = "AIMemory";
    pub const AI_TOOLS: &str = "AITools";

    // 기타
    pub const ASSET_3D: &str = "3D Asset";
    pub const TIMELINE: &str = "Timeline";
    pub const UI_EDITOR: &str = "UIEditor";
    pub const OUT: &str = "out";
    pub const LOGO: &str = "logo";

    // 계층 구조
    pub const HIERARCHY_CAMERA: &str = "Hierachy_camera";
    pub const HIERARCHY_EMPTY: &str = "Hierachy_emptyObject";
    pub const HIERARCHY_LIGHT: &str = "Hierachy_light";
    pub const HIERARCHY_OBJECT: &str = "Hierachy_object";

    // 타이틀바
    pub const TITLEBAR_CLOSE: &str = "titlebar_x";
    pub const TITLEBAR_MAXIMIZE: &str = "titlebar_sizeup";
    pub const TITLEBAR_RESTORE: &str = "titlebar_sizedown";
    pub const TITLEBAR_MINIMIZE: &str = "titlebar_under";
}

/// 로드된 아이콘 정보
pub struct IconInfo {
    /// 텍스처 키 (렌더러에서 사용하는 파일명)
    pub texture_key: String,
    pub width: u32,
    pub height: u32,
}

/// 아이콘 매니저
///
/// 이름 상수 → 텍스처 파일명 매핑 및 프리로드 목록 관리.
///
/// ## 사용법
/// ```ignore
/// let icons = IconManager::new("engine/icons");
///
/// // SlateAppConfig에 프리로드 목록 전달
/// let config = SlateAppConfig::new("SKOPE")
///     .with_icon_base_path("engine/icons")
///     .with_preload_icons(icons.build_preload_list());
///
/// // 위젯에서 아이콘 참조
/// let play_icon = icons.texture_key(names::PLAY);
/// SImage::new().image(play_icon)
/// ```
pub struct IconManager {
    /// 이름 → 텍스처 키 (렌더러 lookup에 사용되는 상대 경로)
    name_to_key: HashMap<String, String>,
    /// 아이콘 디렉토리 경로
    icons_dir: String,
    /// 로드된 아이콘 정보
    loaded: HashMap<String, IconInfo>,
}

impl IconManager {
    pub fn new(icons_dir: &str) -> Self {
        let mut mgr = Self {
            name_to_key: HashMap::new(),
            icons_dir: icons_dir.to_string(),
            loaded: HashMap::new(),
        };
        mgr.register_default_icons();
        mgr
    }

    /// 기본 아이콘 이름 → 파일명 매핑 등록
    fn register_default_icons(&mut self) {
        // symbol_ 접두사 아이콘
        let symbol_icons = [
            (names::PLAY, "symbol_play.png"),
            (names::STOP, "symbol_stop.png"),
            (names::HOLD, "symbol_hold.png"),
            (names::MOVE, "symbol_mov3.png"),
            (names::ROTATE, "symbol_turn.png"),
            (names::SCALE, "symbol_scale.png"),
            (names::GRID_ON, "symbol_grid_toggle_on.png"),
            (names::GRID_OFF, "symbol_grid_toggle_off.png"),
            (names::SCENE, "symbol_scene.png"),
            (names::GAME, "symbol_game.png"),
            (names::HIERARCHY, "symbol_hierachy.png"),
            (names::INSPECTOR, "symbol_Inspector.png"),
            (names::CONSOLE, "symbol_Console.png"),
            (names::FOLDER, "symbol_Folder.png"),
            (names::OPEN_FOLDER, "symbol_Open Folder.png"),
            (names::SEE, "symbol_see.png"),
            (names::HIDE, "symbol_hide.png"),
            (names::CHECK_ON, "symbol_check_toggle_on.png"),
            (names::CHECK_OFF, "symbol_check_toggle_off.png"),
            (names::EFFECT_ON, "symbol_Toggle_effect_on.png"),
            (names::EFFECT_OFF, "symbol_Toggle_effect_off.png"),
            (names::FOG_ON, "symbol_Toggle_fog_on.png"),
            (names::FOG_OFF, "symbol_Toggle_fog_off.png"),
            (names::LIGHT_ON, "symbol_Toggle_light_on.png"),
            (names::LIGHT_OFF, "symbol_Toggle_light_off.png"),
            (names::SKYBOX_ON, "symbol_Toggle_skybox_on.png"),
            (names::SKYBOX_OFF, "symbol_Toggle_skybox_off.png"),
            (names::SOUND_ON, "symbol_Toggle_Sound_on.png"),
            (names::SOUND_OFF, "symbol_Toggle_Sound_off.png"),
            (names::AI, "symbol_AI.png"),
            (names::AI_CHAT, "symbol_AIChat.png"),
            (names::AI_MEMORY, "symbol_AIMemory.png"),
            (names::AI_TOOLS, "symbol_AITools.png"),
            (names::ASSET_3D, "symbol_3D Asset.png"),
            (names::TIMELINE, "symbol_Timeline.png"),
            (names::UI_EDITOR, "symbol_UIEditor.png"),
            (names::OUT, "symbol_out.png"),
            (names::LOGO, "skope_logo.png"),
            (names::HIERARCHY_CAMERA, "symbol_Hierachy_camera.png"),
            (names::HIERARCHY_EMPTY, "symbol_Hierachy_emptyObject.png"),
            (names::HIERARCHY_LIGHT, "symbol_Hierachy_light.png"),
            (names::HIERARCHY_OBJECT, "symbol_Hierachy_object.png"),
        ];

        for (name, filename) in symbol_icons {
            self.name_to_key.insert(name.to_string(), filename.to_string());
        }

        // 타이틀바 아이콘 (서브디렉토리)
        let titlebar_icons = [
            (names::TITLEBAR_CLOSE, "titlebar/_Titlebar_x.png"),
            (names::TITLEBAR_MAXIMIZE, "titlebar/_titlebar_sizeup.png"),
            (names::TITLEBAR_RESTORE, "titlebar/_titlebar_sizedown.png"),
            (names::TITLEBAR_MINIMIZE, "titlebar/_titlebar_under.png"),
        ];

        for (name, filename) in titlebar_icons {
            self.name_to_key.insert(name.to_string(), filename.to_string());
        }
    }

    /// 이름 상수로 텍스처 키(렌더러 lookup용) 조회
    ///
    /// 반환값은 `DrawElement::Image { path }` 또는 `SImage::new().image()` 에 전달할 문자열.
    pub fn texture_key(&self, name: &str) -> Option<&str> {
        self.name_to_key.get(name).map(|s| s.as_str())
    }

    /// SlateAppConfig::with_preload_icons()에 전달할 전체 프리로드 목록 생성
    ///
    /// 디렉토리를 스캔하여 실제 존재하는 PNG 파일만 반환합니다.
    pub fn build_preload_list(&self) -> Vec<String> {
        let dir = Path::new(&self.icons_dir);
        if !dir.exists() {
            log::warn!("[IconManager] Icons directory not found: {}", self.icons_dir);
            return self.name_to_key.values().cloned().collect();
        }

        let mut icons = Vec::new();

        // 루트 디렉토리의 PNG 파일
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext.eq_ignore_ascii_case("png") {
                            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                                icons.push(name.to_string());
                            }
                        }
                    }
                }
            }
        }

        // titlebar 서브디렉토리
        let titlebar_dir = dir.join("titlebar");
        if titlebar_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&titlebar_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        if let Some(ext) = path.extension() {
                            if ext.eq_ignore_ascii_case("png") {
                                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                                    icons.push(format!("titlebar/{}", name));
                                }
                            }
                        }
                    }
                }
            }
        }

        log::info!("[IconManager] Found {} icons to preload", icons.len());
        icons
    }

    /// 커스텀 이름 → 파일명 매핑 등록
    pub fn register(&mut self, name: impl Into<String>, texture_key: impl Into<String>) {
        self.name_to_key.insert(name.into(), texture_key.into());
    }

    /// 이름 상수 → 파일명 매핑 목록
    pub fn all_mappings(&self) -> &HashMap<String, String> {
        &self.name_to_key
    }

    /// 아이콘 디렉토리 경로
    pub fn icons_dir(&self) -> &str {
        &self.icons_dir
    }

    /// 등록된 아이콘 수
    pub fn count(&self) -> usize {
        self.name_to_key.len()
    }

    /// 로드 완료 정보 기록 (SlateApp에서 프리로드 후 호출)
    pub fn mark_loaded(&mut self, texture_key: &str, width: u32, height: u32) {
        if let Some(name) = self.name_to_key.iter()
            .find(|(_, v)| v.as_str() == texture_key)
            .map(|(k, _)| k.clone())
        {
            self.loaded.insert(name, IconInfo {
                texture_key: texture_key.to_string(),
                width,
                height,
            });
        }
    }

    /// 로드된 아이콘 정보 조회
    pub fn get(&self, name: &str) -> Option<&IconInfo> {
        self.loaded.get(name)
    }

    /// 아이콘이 로드되었는지 확인
    pub fn is_loaded(&self, name: &str) -> bool {
        self.loaded.contains_key(name)
    }

    /// 로드된 아이콘 수
    pub fn loaded_count(&self) -> usize {
        self.loaded.len()
    }
}

impl Default for IconManager {
    fn default() -> Self {
        Self::new("engine/icons")
    }
}
