// SKOPE UI - RON Parser
#![allow(dead_code)]

use super::types::*;
use super::UiError;

/// RON 문자열에서 위젯 파싱
pub fn parse_widget(ron_str: &str) -> Result<Widget, UiError> {
    ron::from_str(ron_str).map_err(|e| UiError::ParseError(e.to_string()))
}

/// RON 문자열에서 UI 설정 파싱
pub fn parse_ui_config(ron_str: &str) -> Result<UiConfig, UiError> {
    ron::from_str(ron_str).map_err(|e| UiError::ParseError(e.to_string()))
}

/// 위젯을 RON 문자열로 직렬화
pub fn serialize_widget(widget: &Widget) -> Result<String, UiError> {
    ron::ser::to_string_pretty(widget, ron::ser::PrettyConfig::default())
        .map_err(|e| UiError::ParseError(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_widget() {
        let ron = r#"
            Widget(
                id: Some("test"),
                layout: Layout(
                    anchor: TopLeft,
                    offset: (10.0, 20.0),
                    size: Fixed(100.0, 50.0),
                ),
            )
        "#;

        let widget = parse_widget(ron).unwrap();
        assert_eq!(widget.id, Some("test".to_string()));
    }

    #[test]
    fn test_parse_text_widget() {
        let ron = r#"
            Widget(
                id: Some("title"),
                widget_type: Text(
                    content: "Hello SKOPE",
                    font_size: Some(24.0),
                ),
            )
        "#;

        let widget = parse_widget(ron).unwrap();
        assert!(matches!(widget.widget_type, WidgetType::Text { .. }));
    }

    #[test]
    fn test_parse_nested_widgets() {
        let ron = r#"
            Widget(
                id: Some("container"),
                children: [
                    Widget(
                        id: Some("child1"),
                    ),
                    Widget(
                        id: Some("child2"),
                    ),
                ],
            )
        "#;

        let widget = parse_widget(ron).unwrap();
        assert_eq!(widget.children.len(), 2);
    }

    #[test]
    fn test_parse_button() {
        let ron = r#"
            Widget(
                id: Some("play_btn"),
                widget_type: Button(
                    text: Some("Play"),
                    states: ButtonStates(
                        normal: Some("btn_normal.png"),
                        hover: Some("btn_hover.png"),
                    ),
                ),
                events: {
                    "click": "start_game",
                },
            )
        "#;

        let widget = parse_widget(ron).unwrap();
        assert!(matches!(widget.widget_type, WidgetType::Button { .. }));
        assert!(widget.events.contains_key("click"));
    }
}
