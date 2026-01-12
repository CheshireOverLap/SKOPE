//! Game UI Command Processing
//!
//! Lua 스크립팅에서 전달받은 UI 명령어 처리

use crate::ui;
use crate::scripting;

/// UI 명령 처리
pub fn process_ui_command(
    game_ui: &mut ui::UiSystem,
    cmd: &scripting::UiCommand,
) {
    match cmd {
        scripting::UiCommand::SetVisible { widget_id, visible } => {
            if let Some(widget) = find_widget_mut(&mut game_ui.root, widget_id) {
                widget.visible = *visible;
            }
        }
        scripting::UiCommand::SetText { widget_id, text } => {
            if let Some(widget) = find_widget_mut(&mut game_ui.root, widget_id) {
                if let ui::WidgetType::Text { content, .. } = &mut widget.widget_type {
                    *content = text.clone();
                }
            }
        }
        scripting::UiCommand::SetProgress { widget_id, value, max_value } => {
            if let Some(widget) = find_widget_mut(&mut game_ui.root, widget_id) {
                if let ui::WidgetType::ProgressBar { value: v, max_value: m, .. } = &mut widget.widget_type {
                    *v = *value;
                    *m = *max_value;
                }
            }
        }
        scripting::UiCommand::SetInputValue { widget_id, value } => {
            if let Some(widget) = find_widget_mut(&mut game_ui.root, widget_id) {
                if let ui::WidgetType::InputField { value: v, .. } = &mut widget.widget_type {
                    *v = value.clone();
                }
            }
        }
        scripting::UiCommand::SetOpacity { widget_id, opacity } => {
            if let Some(widget) = find_widget_mut(&mut game_ui.root, widget_id) {
                widget.style.opacity = *opacity;
            }
        }
        scripting::UiCommand::SetTooltip { widget_id, text } => {
            if let Some(widget) = find_widget_mut(&mut game_ui.root, widget_id) {
                widget.tooltip = text.clone();
            }
        }
        scripting::UiCommand::SetInteractive { widget_id, interactive } => {
            if let Some(widget) = find_widget_mut(&mut game_ui.root, widget_id) {
                widget.interactive = *interactive;
            }
        }
        scripting::UiCommand::SetState { widget_id, state } => {
            if let Some(widget) = find_widget_mut(&mut game_ui.root, widget_id) {
                widget.current_state = state.clone();
            }
        }
        scripting::UiCommand::SetBinding { key, value } => {
            game_ui.binding_context_mut().set(key, value.clone().into());
        }
        scripting::UiCommand::ClearBinding { key } => {
            game_ui.binding_context_mut().set(key, ui::BindingValue::Null);
        }
        // 애니메이션
        scripting::UiCommand::PlayAnimation { widget_id, animation_name, duration } => {
            let dur = duration.unwrap_or(0.3);
            let anim = match animation_name.as_str() {
                "fade_in" => ui::animation_presets::fade_in(widget_id, dur),
                "fade_out" => ui::animation_presets::fade_out(widget_id, dur),
                "slide_in_left" => ui::animation_presets::slide_in_left(widget_id, 100.0, dur),
                "slide_in_right" => ui::animation_presets::slide_in_right(widget_id, 100.0, dur),
                "slide_in_top" => ui::animation_presets::slide_in_top(widget_id, 100.0, dur),
                "slide_in_bottom" => ui::animation_presets::slide_in_bottom(widget_id, 100.0, dur),
                "pop_in" => ui::animation_presets::pop_in(widget_id, dur),
                "pop_out" => ui::animation_presets::pop_out(widget_id, dur),
                "shake" => ui::animation_presets::shake(widget_id),
                "pulse" => ui::animation_presets::pulse(widget_id),
                _ => {
                    log::warn!("[UI] Unknown animation: {}", animation_name);
                    return;
                }
            };
            game_ui.play_animation(anim);
        }
        scripting::UiCommand::StopAnimation { widget_id } => {
            game_ui.stop_animation(widget_id, None);
        }
        // 추가 속성 Setter
        scripting::UiCommand::SetBackgroundColor { widget_id, r, g, b, a } => {
            if let Some(widget) = find_widget_mut(&mut game_ui.root, widget_id) {
                widget.style.background_color = Some(ui::Color::Rgba(*r, *g, *b, *a));
            }
        }
        scripting::UiCommand::SetTextColor { widget_id, r, g, b, a } => {
            if let Some(widget) = find_widget_mut(&mut game_ui.root, widget_id) {
                widget.style.text_color = Some(ui::Color::Rgba(*r, *g, *b, *a));
            }
        }
        scripting::UiCommand::SetDraggable { widget_id, draggable } => {
            if let Some(widget) = find_widget_mut(&mut game_ui.root, widget_id) {
                widget.draggable = *draggable;
            }
        }
        scripting::UiCommand::SetDropTarget { widget_id, drop_target } => {
            if let Some(widget) = find_widget_mut(&mut game_ui.root, widget_id) {
                widget.drop_target = *drop_target;
            }
        }
        scripting::UiCommand::SetOffset { widget_id, x, y } => {
            if let Some(widget) = find_widget_mut(&mut game_ui.root, widget_id) {
                widget.layout.offset = (*x, *y);
            }
        }
        scripting::UiCommand::SetSize { widget_id, width, height } => {
            if let Some(widget) = find_widget_mut(&mut game_ui.root, widget_id) {
                widget.layout.size = ui::Size::Fixed(*width, *height);
            }
        }
        scripting::UiCommand::SetScroll { widget_id, x, y } => {
            if let Some(widget) = find_widget_mut(&mut game_ui.root, widget_id) {
                widget.scroll_offset = (*x, *y);
            }
        }
        // 위젯 생명주기
        scripting::UiCommand::Create { definition, parent_id } => {
            let widget = create_widget_from_definition(definition);
            if let Some(ref mut root) = game_ui.root {
                if let Some(ref pid) = parent_id {
                    // 부모에 추가
                    if let Some(parent) = find_widget_recursive_mut(root, pid) {
                        parent.children.push(widget);
                    }
                } else {
                    // 루트에 추가
                    root.children.push(widget);
                }
            }
        }
        scripting::UiCommand::Destroy { widget_id } => {
            if let Some(ref mut root) = game_ui.root {
                remove_widget_by_id(root, widget_id);
            }
        }
        scripting::UiCommand::SetParent { widget_id, new_parent_id } => {
            if let Some(ref mut root) = game_ui.root {
                // 1. 위젯을 현재 부모에서 제거하고 반환
                if let Some(widget) = remove_and_return_widget(root, widget_id) {
                    // 2. 새 부모에 추가
                    if let Some(new_parent) = find_widget_recursive_mut(root, new_parent_id) {
                        new_parent.children.push(widget);
                    }
                }
            }
        }
        // 기타 명령어는 무시
        _ => {}
    }
}

/// 앵커 문자열을 Anchor enum으로 변환
fn anchor_from_string(s: &str) -> ui::Anchor {
    match s {
        "TopLeft" => ui::Anchor::TopLeft,
        "TopCenter" => ui::Anchor::TopCenter,
        "TopRight" => ui::Anchor::TopRight,
        "MiddleLeft" => ui::Anchor::MiddleLeft,
        "Center" => ui::Anchor::Center,
        "MiddleRight" => ui::Anchor::MiddleRight,
        "BottomLeft" => ui::Anchor::BottomLeft,
        "BottomCenter" => ui::Anchor::BottomCenter,
        "BottomRight" => ui::Anchor::BottomRight,
        "Stretch" => ui::Anchor::Stretch,
        _ => ui::Anchor::TopLeft,
    }
}

/// 위젯 ID로 위젯 찾기 (mutable)
fn find_widget_mut<'a>(root: &'a mut Option<ui::Widget>, widget_id: &str) -> Option<&'a mut ui::Widget> {
    if let Some(ref mut widget) = root {
        find_widget_recursive_mut(widget, widget_id)
    } else {
        None
    }
}

fn find_widget_recursive_mut<'a>(widget: &'a mut ui::Widget, widget_id: &str) -> Option<&'a mut ui::Widget> {
    if widget.id.as_deref() == Some(widget_id) {
        return Some(widget);
    }

    for child in &mut widget.children {
        if let Some(found) = find_widget_recursive_mut(child, widget_id) {
            return Some(found);
        }
    }

    None
}

/// WidgetDefinition에서 Widget 생성
fn create_widget_from_definition(def: &scripting::WidgetDefinition) -> ui::Widget {
    let widget_type = match def.widget_type.as_str() {
        "Container" => ui::WidgetType::Container,
        "Text" => ui::WidgetType::Text {
            content: def.text.clone().unwrap_or_default(),
            font: None,
            font_size: None,
        },
        "Button" => ui::WidgetType::Button {
            text: def.text.clone(),
            states: ui::ButtonStates::default(),
        },
        "Image" => ui::WidgetType::Image {
            src: def.src.clone().unwrap_or_default(),
            color: None,
            preserve_aspect: true,
        },
        _ => ui::WidgetType::Container,
    };

    let anchor = anchor_from_string(def.anchor.as_deref().unwrap_or("TopLeft"));

    ui::Widget {
        id: def.id.clone(),
        widget_type,
        layout: ui::Layout {
            anchor,
            offset: def.offset.unwrap_or((0.0, 0.0)),
            size: def.size.map(|(w, h)| ui::Size::Fixed(w, h)).unwrap_or(ui::Size::FitContent),
            ..Default::default()
        },
        style: ui::Style {
            background_color: def.background_color.map(|(r, g, b, a)| ui::Color::Rgba(r, g, b, a)),
            text_color: def.text_color.map(|(r, g, b, a)| ui::Color::Rgba(r, g, b, a)),
            ..Default::default()
        },
        visible: def.visible,
        interactive: def.interactive,
        ..Default::default()
    }
}

/// 위젯 트리에서 특정 ID의 위젯 제거 (재귀)
fn remove_widget_by_id(widget: &mut ui::Widget, target_id: &str) -> bool {
    // 자식들 중에서 찾아서 제거
    if let Some(idx) = widget.children.iter().position(|c| c.id.as_deref() == Some(target_id)) {
        widget.children.remove(idx);
        return true;
    }

    // 재귀적으로 자식들의 자식에서 검색
    for child in &mut widget.children {
        if remove_widget_by_id(child, target_id) {
            return true;
        }
    }

    false
}

/// 위젯을 제거하고 반환 (재부모화용)
fn remove_and_return_widget(widget: &mut ui::Widget, target_id: &str) -> Option<ui::Widget> {
    // 자식들 중에서 찾아서 제거하고 반환
    if let Some(idx) = widget.children.iter().position(|c| c.id.as_deref() == Some(target_id)) {
        return Some(widget.children.remove(idx));
    }

    // 재귀적으로 자식들의 자식에서 검색
    for child in &mut widget.children {
        if let Some(found) = remove_and_return_widget(child, target_id) {
            return Some(found);
        }
    }

    None
}
