//! Asset Browser 패널
//!
//! 모든 에셋을 카테고리별로 표시하고 씬에 스폰

use bevy_ecs::prelude::*;
use fyrox_core::pool::Handle;
use fyrox_ui::{
    button::{ButtonBuilder, ButtonMessage},
    message::UiMessage,
    scroll_viewer::ScrollViewerBuilder,
    stack_panel::StackPanelBuilder,
    text::TextBuilder,
    widget::WidgetBuilder,
    window::{WindowBuilder, WindowTitle},
    Orientation, Thickness, UiNode, UserInterface,
};
use std::collections::HashMap;

use crate::ecs_resources::MeshAssets;
use crate::prefab::PrefabRegistry;

/// 에셋 카테고리
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AssetCategory {
    #[default]
    All,
    Meshes,
    Prefabs,
    Scripts,
}

/// 에셋 참조
#[derive(Debug, Clone)]
pub struct AssetRef {
    pub name: String,
    pub category: AssetCategory,
    pub index: Option<usize>,
}

/// Asset Browser 패널
pub struct AssetBrowserPanel {
    /// 윈도우 핸들
    pub window: Handle<UiNode>,

    /// 카테고리 버튼들
    tab_all: Handle<UiNode>,
    tab_meshes: Handle<UiNode>,
    tab_prefabs: Handle<UiNode>,
    tab_scripts: Handle<UiNode>,

    /// 에셋 목록 컨테이너
    asset_list: Handle<UiNode>,

    /// 정보 표시
    info_text: Handle<UiNode>,

    /// 버튼 → 에셋 매핑
    button_to_asset: HashMap<Handle<UiNode>, AssetRef>,

    /// 현재 카테고리
    current_category: AssetCategory,

    /// 선택된 에셋
    selected_asset: Option<AssetRef>,

    /// 탭 변경 플래그 (refresh 필요)
    needs_refresh: bool,
}

impl AssetBrowserPanel {
    /// 새 Asset Browser 패널 생성
    pub fn new(ui: &mut UserInterface) -> Self {
        let ctx = &mut ui.build_ctx();

        // 탭 버튼들
        let tab_all = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_width(45.0)
                .with_height(22.0)
                .with_margin(Thickness::uniform(1.0)),
        )
        .with_text("All")
        .build(ctx);

        let tab_meshes = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_width(55.0)
                .with_height(22.0)
                .with_margin(Thickness::uniform(1.0)),
        )
        .with_text("Meshes")
        .build(ctx);

        let tab_prefabs = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_width(55.0)
                .with_height(22.0)
                .with_margin(Thickness::uniform(1.0)),
        )
        .with_text("Prefabs")
        .build(ctx);

        let tab_scripts = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_width(55.0)
                .with_height(22.0)
                .with_margin(Thickness::uniform(1.0)),
        )
        .with_text("Scripts")
        .build(ctx);

        // 탭 바 (수평 레이아웃)
        let tab_bar = StackPanelBuilder::new(
            WidgetBuilder::new()
                .with_height(26.0)
                .with_margin(Thickness::uniform(2.0))
                .with_child(tab_all)
                .with_child(tab_meshes)
                .with_child(tab_prefabs)
                .with_child(tab_scripts),
        )
        .with_orientation(Orientation::Horizontal)
        .build(ctx);

        // 에셋 목록 (빈 상태로 시작)
        let asset_list = StackPanelBuilder::new(WidgetBuilder::new())
            .with_orientation(Orientation::Vertical)
            .build(ctx);

        // 스크롤 뷰어
        let scroll_viewer = ScrollViewerBuilder::new(WidgetBuilder::new())
            .with_content(asset_list)
            .build(ctx);

        // 정보 표시
        let info_text = TextBuilder::new(
            WidgetBuilder::new()
                .with_height(36.0)
                .with_margin(Thickness::uniform(4.0)),
        )
        .with_text("Click asset to spawn")
        .build(ctx);

        // 전체 컨텐츠
        let content = StackPanelBuilder::new(
            WidgetBuilder::new()
                .with_child(tab_bar)
                .with_child(scroll_viewer)
                .with_child(info_text),
        )
        .with_orientation(Orientation::Vertical)
        .build(ctx);

        // 윈도우
        let window = WindowBuilder::new(
            WidgetBuilder::new()
                .with_width(230.0)
                .with_height(350.0)
                .with_desired_position(fyrox_core::algebra::Vector2::new(310.0, 50.0)),
        )
        .with_title(WindowTitle::text("Asset Browser"))
        .with_content(content)
        .build(ctx);

        Self {
            window,
            tab_all,
            tab_meshes,
            tab_prefabs,
            tab_scripts,
            asset_list,
            info_text,
            button_to_asset: HashMap::new(),
            current_category: AssetCategory::All,
            selected_asset: None,
            needs_refresh: false,
        }
    }

    /// World 리소스에서 에셋 목록 갱신
    pub fn refresh(&mut self, world: &World, ui: &mut UserInterface) {
        self.needs_refresh = false;

        // 기존 버튼들 제거는 UI 메시지로 처리하면 복잡하므로
        // 간단히 매핑만 클리어 (위젯은 rebuild시 덮어씌워짐)
        self.button_to_asset.clear();

        let mut assets = Vec::new();

        // Meshes 수집
        if let Some(mesh_assets) = world.get_resource::<MeshAssets>() {
            for (name, &index) in &mesh_assets.name_to_index {
                assets.push(AssetRef {
                    name: name.clone(),
                    category: AssetCategory::Meshes,
                    index: Some(index),
                });
            }
        }

        // Prefabs 수집
        if let Some(prefab_registry) = world.get_resource::<PrefabRegistry>() {
            for name in prefab_registry.list() {
                assets.push(AssetRef {
                    name: name.to_string(),
                    category: AssetCategory::Prefabs,
                    index: None,
                });
            }
        }

        // Scripts 수집 (파일 시스템)
        if let Ok(entries) = std::fs::read_dir("assets/scripts") {
            for entry in entries.flatten() {
                if let Some(name) = entry.path().file_name() {
                    if let Some(name_str) = name.to_str() {
                        if name_str.ends_with(".lua") {
                            assets.push(AssetRef {
                                name: name_str.to_string(),
                                category: AssetCategory::Scripts,
                                index: None,
                            });
                        }
                    }
                }
            }
        }

        // 이름순 정렬
        assets.sort_by(|a, b| a.name.cmp(&b.name));

        // 카테고리 필터 적용
        let filtered: Vec<_> = assets
            .into_iter()
            .filter(|a| {
                self.current_category == AssetCategory::All || a.category == self.current_category
            })
            .collect();

        // 버튼 생성
        let ctx = &mut ui.build_ctx();
        let mut button_handles = Vec::new();

        for asset in filtered {
            let label = match asset.category {
                AssetCategory::Meshes => format!("[M] {}", asset.name),
                AssetCategory::Prefabs => format!("[P] {}", asset.name),
                AssetCategory::Scripts => format!("[S] {}", asset.name),
                AssetCategory::All => asset.name.clone(),
            };

            let btn = ButtonBuilder::new(
                WidgetBuilder::new()
                    .with_height(22.0)
                    .with_margin(Thickness::uniform(1.0)),
            )
            .with_text(&label)
            .build(ctx);

            self.button_to_asset.insert(btn, asset);
            button_handles.push(btn);
        }

        // 새 에셋 목록 생성
        let new_asset_list = StackPanelBuilder::new(
            WidgetBuilder::new().with_children(button_handles),
        )
        .with_orientation(Orientation::Vertical)
        .build(ctx);

        // 스크롤 뷰어 내용 교체
        let scroll_viewer = ScrollViewerBuilder::new(WidgetBuilder::new())
            .with_content(new_asset_list)
            .build(ctx);

        // 정보 텍스트
        let new_info_text = TextBuilder::new(
            WidgetBuilder::new()
                .with_height(36.0)
                .with_margin(Thickness::uniform(4.0)),
        )
        .with_text("Click asset to spawn")
        .build(ctx);

        // 탭 버튼들 재생성
        let tab_all = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_width(45.0)
                .with_height(22.0)
                .with_margin(Thickness::uniform(1.0)),
        )
        .with_text("All")
        .build(ctx);

        let tab_meshes = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_width(55.0)
                .with_height(22.0)
                .with_margin(Thickness::uniform(1.0)),
        )
        .with_text("Meshes")
        .build(ctx);

        let tab_prefabs = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_width(55.0)
                .with_height(22.0)
                .with_margin(Thickness::uniform(1.0)),
        )
        .with_text("Prefabs")
        .build(ctx);

        let tab_scripts = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_width(55.0)
                .with_height(22.0)
                .with_margin(Thickness::uniform(1.0)),
        )
        .with_text("Scripts")
        .build(ctx);

        let tab_bar = StackPanelBuilder::new(
            WidgetBuilder::new()
                .with_height(26.0)
                .with_margin(Thickness::uniform(2.0))
                .with_child(tab_all)
                .with_child(tab_meshes)
                .with_child(tab_prefabs)
                .with_child(tab_scripts),
        )
        .with_orientation(Orientation::Horizontal)
        .build(ctx);

        // 전체 컨텐츠 재구성
        let content = StackPanelBuilder::new(
            WidgetBuilder::new()
                .with_child(tab_bar)
                .with_child(scroll_viewer)
                .with_child(new_info_text),
        )
        .with_orientation(Orientation::Vertical)
        .build(ctx);

        // 윈도우 재생성
        let window = WindowBuilder::new(
            WidgetBuilder::new()
                .with_width(230.0)
                .with_height(350.0)
                .with_desired_position(fyrox_core::algebra::Vector2::new(310.0, 50.0)),
        )
        .with_title(WindowTitle::text("Asset Browser"))
        .with_content(content)
        .build(ctx);

        // 핸들 업데이트
        self.window = window;
        self.tab_all = tab_all;
        self.tab_meshes = tab_meshes;
        self.tab_prefabs = tab_prefabs;
        self.tab_scripts = tab_scripts;
        self.asset_list = new_asset_list;
        self.info_text = new_info_text;

        log::info!(
            "[AssetBrowser] Refreshed with {} assets (category: {:?})",
            self.button_to_asset.len(),
            self.current_category
        );
    }

    /// 탭 변경 후 refresh가 필요한지 확인
    pub fn needs_refresh(&self) -> bool {
        self.needs_refresh
    }

    /// UI 메시지 처리
    /// 반환: 스폰할 에셋 (클릭 시)
    pub fn handle_message(&mut self, message: &UiMessage, ui: &UserInterface) -> Option<AssetRef> {
        // 버튼 클릭 처리
        if let Some(ButtonMessage::Click) = message.data() {
            let dest = message.destination();

            // 탭 클릭
            if dest == self.tab_all {
                self.current_category = AssetCategory::All;
                self.needs_refresh = true;
                log::info!("[AssetBrowser] Tab: All");
                return None;
            } else if dest == self.tab_meshes {
                self.current_category = AssetCategory::Meshes;
                self.needs_refresh = true;
                log::info!("[AssetBrowser] Tab: Meshes");
                return None;
            } else if dest == self.tab_prefabs {
                self.current_category = AssetCategory::Prefabs;
                self.needs_refresh = true;
                log::info!("[AssetBrowser] Tab: Prefabs");
                return None;
            } else if dest == self.tab_scripts {
                self.current_category = AssetCategory::Scripts;
                self.needs_refresh = true;
                log::info!("[AssetBrowser] Tab: Scripts");
                return None;
            }

            // 에셋 버튼 클릭
            if let Some(asset) = self.button_to_asset.get(&dest) {
                self.selected_asset = Some(asset.clone());

                // 정보 업데이트
                let info = format!(
                    "{}\nType: {:?}{}",
                    asset.name,
                    asset.category,
                    asset
                        .index
                        .map(|i| format!(" | Index: {}", i))
                        .unwrap_or_default()
                );
                ui.send(self.info_text, fyrox_ui::text::TextMessage::Text(info));

                return Some(asset.clone());
            }
        }

        None
    }

    /// 선택된 에셋 반환
    #[allow(dead_code)]
    pub fn selected_asset(&self) -> Option<&AssetRef> {
        self.selected_asset.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_category_default() {
        assert_eq!(AssetCategory::default(), AssetCategory::All);
    }

    #[test]
    fn test_asset_ref() {
        let asset = AssetRef {
            name: "Cube".to_string(),
            category: AssetCategory::Meshes,
            index: Some(0),
        };
        assert_eq!(asset.name, "Cube");
        assert_eq!(asset.category, AssetCategory::Meshes);
        assert_eq!(asset.index, Some(0));
    }
}
