// SKOPE Editor Icons
// PNG 아이콘을 런타임에 로드하여 egui 텍스처로 변환

use std::collections::HashMap;
use std::path::Path;
use crate::paths;

/// 에디터 아이콘 매니저
/// PNG 파일을 로드하여 egui 텍스처로 변환
#[derive(Default)]
pub struct IconManager {
    textures: HashMap<String, egui::TextureHandle>,
    loaded: bool,
}


impl IconManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// 아이콘 로드 (처음 한 번만 호출)
    pub fn load(&mut self, ctx: &egui::Context) {
        if self.loaded {
            return;
        }

        let icons_dir = Path::new(paths::engine::ICONS);

        // 모든 아이콘 매핑 (이름, 파일명, 크기)
        let all_icons: &[(&str, &str, u32)] = &[
            // 탭 아이콘 (20px)
            ("tab_scene", "symbol_scene.png", 20),
            ("tab_game", "symbol_game.png", 20),
            ("tab_hierarchy", "symbol_hierachy.png", 20),
            ("tab_inspector", "symbol_Inspector.png", 20),
            ("tab_assets", "symbol_Folder.png", 20),
            ("tab_console", "symbol_Console.png", 20),
            ("tab_ai", "symbol_AI.png", 20),
            ("tab_ai_chat", "symbol_AIChat.png", 20),
            ("tab_ai_memory", "symbol_AIMemory.png", 20),
            ("tab_timeline", "symbol_Timeline.png", 20),
            ("tab_ui_editor", "symbol_UIEditor.png", 20),

            // 기즈모 도구 아이콘 (18px)
            ("tool_select", "symbol_hold.png", 18),
            ("tool_move", "symbol_mov3.png", 18),
            ("tool_rotate", "symbol_turn.png", 18),
            ("tool_scale", "symbol_scale.png", 18),

            // 가시성 아이콘 (16px)
            ("visibility_on", "symbol_see.png", 16),
            ("visibility_off", "symbol_hide.png", 16),

            // 폴더/에셋 아이콘 (16px)
            ("folder", "symbol_Folder.png", 16),
            ("folder_open", "symbol_Open Folder.png", 16),
            ("asset_3d", "symbol_3D Asset.png", 16),

            // AI 도구 아이콘 (16px)
            ("ai_tools", "symbol_AITools.png", 16),

            // 플레이백 컨트롤 (18px)
            ("play", "symbol_play.png", 18),
            ("stop", "symbol_stop.png", 18),

            // 하이어라키 아이템 타입 아이콘 (16px)
            ("hierarchy_object", "symbol_Hierachy_object.png", 16),
            ("hierarchy_camera", "symbol_Hierachy_camera.png", 16),
            ("hierarchy_light", "symbol_Hierachy_light.png", 16),
            ("hierarchy_empty", "symbol_Hierachy_emptyObject.png", 16),

            // 토글 아이콘 (16px)
            ("toggle_sound_on", "symbol_Toggle_Sound_on.png", 16),
            ("toggle_sound_off", "symbol_Toggle_Sound_off.png", 16),
            ("toggle_effect_on", "symbol_Toggle_effect_on.png", 16),
            ("toggle_effect_off", "symbol_Toggle_effect_off.png", 16),
            ("toggle_fog_on", "symbol_Toggle_fog_on.png", 16),
            ("toggle_fog_off", "symbol_Toggle_fog_off.png", 16),
            ("toggle_light_on", "symbol_Toggle_light_on.png", 16),
            ("toggle_light_off", "symbol_Toggle_light_off.png", 16),
            ("toggle_skybox_on", "symbol_Toggle_skybox_on.png", 16),
            ("toggle_skybox_off", "symbol_Toggle_skybox_off.png", 16),
            ("toggle_grid_on", "symbol_grid_toggle_on.png", 16),
            ("toggle_grid_off", "symbol_grid_toggle_off.png", 16),
            ("toggle_check_on", "symbol_check_toggle_on.png", 16),
            ("toggle_check_off", "symbol_check_toggle_off.png", 16),

            // 타이틀바 아이콘 (16px)
            ("titlebar_close", "titlebar/_Titlebar_x.png", 16),
            ("titlebar_maximize", "titlebar/_titlebar_sizeup.png", 16),
            ("titlebar_restore", "titlebar/_titlebar_sizedown.png", 16),
            ("titlebar_minimize", "titlebar/_titlebar_under.png", 16),
        ];

        for (name, filename, size) in all_icons {
            let png_path = icons_dir.join(filename);
            if let Some(texture) = self.load_png(ctx, &png_path, name, *size) {
                self.textures.insert(name.to_string(), texture);
                log::debug!("[Icons] Loaded: {}", name);
            } else {
                log::warn!("[Icons] Failed to load: {} ({})", name, png_path.display());
            }
        }

        log::info!("[Icons] Loaded {} icons", self.textures.len());
        self.loaded = true;
    }

    /// 기즈모 모드에 해당하는 아이콘 가져오기
    pub fn get_for_gizmo(&self, mode: &super::docking::GizmoMode) -> Option<&egui::TextureHandle> {
        use super::docking::GizmoMode;

        let name = match mode {
            GizmoMode::Select => "tool_select",
            GizmoMode::Move => "tool_move",
            GizmoMode::Rotate => "tool_rotate",
            GizmoMode::Scale => "tool_scale",
        };

        self.get(name)
    }

    /// PNG 파일을 egui 텍스처로 변환
    fn load_png(
        &self,
        ctx: &egui::Context,
        path: &Path,
        name: &str,
        size: u32,
    ) -> Option<egui::TextureHandle> {
        // PNG 파일 로드
        let img = image::open(path).ok()?;

        // 지정된 크기로 리사이즈
        let resized = img.resize(size, size, image::imageops::FilterType::Lanczos3);
        let rgba = resized.to_rgba8();
        let (width, height) = rgba.dimensions();

        // egui ColorImage로 변환
        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            [width as usize, height as usize],
            rgba.as_raw(),
        );

        // 텍스처 등록
        let texture = ctx.load_texture(
            name,
            color_image,
            egui::TextureOptions::LINEAR,
        );

        Some(texture)
    }

    /// 아이콘 텍스처 가져오기
    pub fn get(&self, name: &str) -> Option<&egui::TextureHandle> {
        self.textures.get(name)
    }

    /// 탭 타입에 해당하는 아이콘 가져오기
    pub fn get_for_tab(&self, tab: &super::docking::Tab) -> Option<&egui::TextureHandle> {
        use super::docking::Tab;

        let name = match tab {
            Tab::Scene => "tab_scene",
            Tab::Game => "tab_game",
            Tab::Hierarchy => "tab_hierarchy",
            Tab::Inspector => "tab_inspector",
            Tab::Assets => "tab_assets",
            Tab::Console => "tab_console",
            Tab::AiChat => "tab_ai_chat",
            Tab::AiMemory => "tab_ai_memory",
            Tab::AiTodos => "tab_ai",
            Tab::UiEditor => "tab_ui_editor",
            Tab::Animation => "tab_timeline",
            Tab::MagicSystem => "tab_inspector",
        };

        self.get(name)
    }

    /// 로드 완료 여부
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }
}
