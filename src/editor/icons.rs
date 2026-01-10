// SKOPE Editor Icons
// SVG 아이콘을 런타임에 렌더링하여 egui 텍스처로 로드

use std::collections::HashMap;
use std::path::Path;
use crate::paths;

/// 에디터 아이콘 매니저
/// SVG 파일을 로드하여 egui 텍스처로 변환
pub struct IconManager {
    textures: HashMap<String, egui::TextureHandle>,
    loaded: bool,
}

impl Default for IconManager {
    fn default() -> Self {
        Self {
            textures: HashMap::new(),
            loaded: false,
        }
    }
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
            ("tab_scene", "symbol_scene.svg", 20),
            ("tab_game", "symbol_game.svg", 20),
            ("tab_hierarchy", "symbol_hierachy.svg", 20),
            ("tab_inspector", "symbol_Inspector.svg", 20),
            ("tab_assets", "symbol_Folder.svg", 20),
            ("tab_console", "symbol_Console.svg", 20),
            ("tab_ai", "symbol_AI.svg", 20),
            ("tab_ai_chat", "symbol_AIChat.svg", 20),
            ("tab_ai_memory", "symbol_AIMemory.svg", 20),

            // 기즈모 도구 아이콘 (18px)
            ("tool_select", "symbol_hold.svg", 18),
            ("tool_move", "symbol_mov3.svg", 18),
            ("tool_rotate", "symbol_turn.svg", 18),
            ("tool_scale", "symbol_scale.svg", 18),

            // 가시성 아이콘 (16px)
            ("visibility_on", "symbol_see.svg", 16),
            ("visibility_off", "symbol_hide.svg", 16),

            // 폴더/에셋 아이콘 (16px)
            ("folder", "symbol_Folder.svg", 16),
            ("folder_open", "symbol_Open Folder.svg", 16),
            ("asset_3d", "symbol_3D Asset.svg", 16),

            // AI 도구 아이콘 (16px)
            ("ai_tools", "symbol_AITools.svg", 16),
        ];

        for (name, filename, size) in all_icons {
            let svg_path = icons_dir.join(filename);
            if let Some(texture) = self.load_svg(ctx, &svg_path, name, *size) {
                self.textures.insert(name.to_string(), texture);
                log::debug!("[Icons] Loaded: {}", name);
            } else {
                log::warn!("[Icons] Failed to load: {} ({})", name, svg_path.display());
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

    /// SVG 파일을 egui 텍스처로 변환
    fn load_svg(
        &self,
        ctx: &egui::Context,
        path: &Path,
        name: &str,
        size: u32,
    ) -> Option<egui::TextureHandle> {
        // SVG 파일 읽기
        let svg_data = std::fs::read(path).ok()?;

        // usvg로 파싱
        let options = resvg::usvg::Options::default();
        let tree = resvg::usvg::Tree::from_data(&svg_data, &options).ok()?;

        // 렌더링 크기 계산
        let svg_size = tree.size();
        let scale = size as f32 / svg_size.width().max(svg_size.height());
        let width = (svg_size.width() * scale).ceil() as u32;
        let height = (svg_size.height() * scale).ceil() as u32;

        // Pixmap 생성 및 렌더링
        let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;

        // 배경 투명
        pixmap.fill(resvg::tiny_skia::Color::TRANSPARENT);

        // 변환 행렬 (스케일링)
        let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);

        // SVG 렌더링
        resvg::render(&tree, transform, &mut pixmap.as_mut());

        // egui ColorImage로 변환
        let pixels: Vec<u8> = pixmap.data().to_vec();
        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            [width as usize, height as usize],
            &pixels,
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
            Tab::UiEditor => "tab_inspector",  // 임시로 Inspector 아이콘 사용
            Tab::Animation => "tab_scene",     // 임시로 Scene 아이콘 사용
            Tab::MagicSystem => "tab_inspector", // 임시로 Inspector 아이콘 사용
        };

        self.get(name)
    }

    /// 로드 완료 여부
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }
}
