//! Spawn Menu (Shift+A)
//!
//! Blender 스타일 프리미티브 생성 메뉴

use fyrox_core::pool::Handle;
use fyrox_ui::{
    button::ButtonBuilder,
    message::UiMessage,
    popup::{Placement, PopupBuilder, PopupMessage},
    stack_panel::StackPanelBuilder,
    widget::WidgetBuilder,
    Orientation, Thickness, UiNode, UserInterface,
};

/// 생성 가능한 항목
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnItem {
    /// 큐브 (1x1x1)
    Cube,
    /// 구 (UV Sphere)
    Sphere,
    /// 실린더
    Cylinder,
    /// 평면 (XZ)
    Plane,
    /// 빈 엔티티 (Transform만)
    Empty,
    /// 포인트 라이트
    PointLight,
    /// 스팟 라이트
    SpotLight,
    /// 태양 라이트 (Directional)
    SunLight,
}

impl SpawnItem {
    /// 모든 항목 목록
    pub fn all() -> &'static [(SpawnItem, &'static str)] {
        &[
            (SpawnItem::Cube, "Cube"),
            (SpawnItem::Sphere, "Sphere"),
            (SpawnItem::Cylinder, "Cylinder"),
            (SpawnItem::Plane, "Plane"),
            (SpawnItem::Empty, "Empty"),
            (SpawnItem::PointLight, "Point Light"),
            (SpawnItem::SpotLight, "Spot Light"),
            (SpawnItem::SunLight, "Sun Light"),
        ]
    }

    /// 메시 이름 반환 (메시가 있는 경우)
    pub fn mesh_name(&self) -> Option<&'static str> {
        match self {
            SpawnItem::Cube => Some("Cube"),
            SpawnItem::Sphere => Some("Sphere"),
            SpawnItem::Cylinder => Some("Cylinder"),
            SpawnItem::Plane => Some("Plane"),
            _ => None,
        }
    }

    /// 엔티티 이름 반환
    pub fn entity_name(&self) -> &'static str {
        match self {
            SpawnItem::Cube => "Cube",
            SpawnItem::Sphere => "Sphere",
            SpawnItem::Cylinder => "Cylinder",
            SpawnItem::Plane => "Plane",
            SpawnItem::Empty => "Empty",
            SpawnItem::PointLight => "Point Light",
            SpawnItem::SpotLight => "Spot Light",
            SpawnItem::SunLight => "Sun Light",
        }
    }
}

/// 생성 메뉴 팝업
pub struct SpawnMenu {
    /// 팝업 핸들
    popup: Handle<UiNode>,
    /// 버튼 목록 (핸들, 항목)
    buttons: Vec<(Handle<UiNode>, SpawnItem)>,
}

impl SpawnMenu {
    /// 새 SpawnMenu 생성
    pub fn new(ui: &mut UserInterface) -> Self {
        let ctx = &mut ui.build_ctx();

        // 버튼들 생성
        let mut buttons = Vec::new();
        let mut button_handles = Vec::new();

        for (item, label) in SpawnItem::all() {
            let btn = ButtonBuilder::new(
                WidgetBuilder::new()
                    .with_width(150.0)
                    .with_height(26.0)
                    .with_margin(Thickness::uniform(2.0)),
            )
            .with_text(*label)
            .build(ctx);

            buttons.push((btn, *item));
            button_handles.push(btn);
        }

        // 수직 스택 패널
        let content = StackPanelBuilder::new(
            WidgetBuilder::new().with_children(button_handles),
        )
        .with_orientation(Orientation::Vertical)
        .build(ctx);

        // 팝업
        let popup = PopupBuilder::new(
            WidgetBuilder::new()
                .with_width(160.0),
        )
        .stays_open(false) // 외부 클릭 시 자동 닫힘
        .with_content(content)
        .with_placement(Placement::Cursor(Handle::NONE))
        .build(ctx);

        Self { popup, buttons }
    }

    /// 커서 위치에 메뉴 열기
    pub fn open_at_cursor(&self, ui: &UserInterface) {
        // 커서 위치로 placement 업데이트 후 열기
        ui.send(self.popup, PopupMessage::Placement(Placement::Cursor(Handle::NONE)));
        ui.send(self.popup, PopupMessage::Open);
    }

    /// 메뉴 닫기
    #[allow(dead_code)]
    pub fn close(&self, ui: &UserInterface) {
        ui.send(self.popup, PopupMessage::Close);
    }

    /// 메시지 처리 - 클릭된 항목 반환
    pub fn handle_message(&self, message: &UiMessage) -> Option<SpawnItem> {
        // 버튼 클릭 메시지 확인
        if let Some(fyrox_ui::button::ButtonMessage::Click) = message.data() {
            for (btn, item) in &self.buttons {
                if message.destination() == *btn {
                    return Some(*item);
                }
            }
        }
        None
    }

    /// 팝업 핸들 반환
    #[allow(dead_code)]
    pub fn popup_handle(&self) -> Handle<UiNode> {
        self.popup
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spawn_item_mesh_name() {
        assert_eq!(SpawnItem::Cube.mesh_name(), Some("Cube"));
        assert_eq!(SpawnItem::Empty.mesh_name(), None);
        assert_eq!(SpawnItem::PointLight.mesh_name(), None);
    }

    #[test]
    fn test_spawn_item_entity_name() {
        assert_eq!(SpawnItem::Cube.entity_name(), "Cube");
        assert_eq!(SpawnItem::PointLight.entity_name(), "Point Light");
    }
}
