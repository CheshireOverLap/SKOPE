//! Editor Panel Widgets
//!
//! 에디터 패널용 위젯 헬퍼

use fyrox_core::algebra::Vector3;
use fyrox_core::pool::Handle;
use fyrox_ui::vec::Vec3EditorBuilder;
use fyrox_ui::widget::WidgetBuilder;
use fyrox_ui::{BuildContext, Thickness, UiNode};

/// Vec3 에디터 생성 헬퍼
pub fn build_vec3_editor(ctx: &mut BuildContext) -> Handle<UiNode> {
    Vec3EditorBuilder::<f32>::new(
        WidgetBuilder::new()
            .with_height(22.0)
            .with_margin(Thickness::uniform(2.0)),
    )
    .with_value(Vector3::new(0.0, 0.0, 0.0))
    .with_step(Vector3::new(0.1, 0.1, 0.1))
    .with_precision(3)
    .build(ctx)
}
