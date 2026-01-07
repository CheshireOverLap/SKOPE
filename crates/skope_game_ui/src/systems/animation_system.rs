//! SKOPE UI - Animation System
//!
//! 키프레임 기반 애니메이션 관리

use crate::types::*;
use crate::animation::{ActiveAnimation, AnimationRepeat, AnimatedProperty, AnimatedValue};

/// 애니메이션 시스템
pub struct AnimationSystem {
    /// 활성 애니메이션들
    active_animations: Vec<ActiveAnimation>,
}

impl AnimationSystem {
    pub fn new() -> Self {
        Self {
            active_animations: Vec::new(),
        }
    }

    /// 애니메이션 업데이트 (완료된 이벤트 이름 반환)
    pub fn update(&mut self, root: &mut Widget, delta_time: f32) -> Vec<String> {
        let mut completed_events: Vec<String> = Vec::new();

        let animations: Vec<ActiveAnimation> = self.active_animations.drain(..).collect();

        for mut anim in animations {
            anim.elapsed += delta_time;
            let progress = anim.progress();

            // 위젯에 애니메이션 속성 적용
            if let Some(widget) = find_widget_by_id_mut(root, &anim.widget_id) {
                for track in &anim.tracks {
                    let value = track.value_at(progress);
                    apply_animated_value(widget, &track.property, &value);
                }
            }

            // 완료 확인
            if anim.is_complete() {
                if let Some(ref event) = anim.on_complete {
                    completed_events.push(event.clone());
                }

                match anim.repeat {
                    AnimationRepeat::Loop => {
                        anim.elapsed = anim.elapsed % anim.duration;
                        self.active_animations.push(anim);
                    }
                    AnimationRepeat::PingPong => {
                        anim.elapsed = anim.elapsed % anim.duration;
                        for track in &mut anim.tracks {
                            std::mem::swap(&mut track.from, &mut track.to);
                        }
                        self.active_animations.push(anim);
                    }
                    AnimationRepeat::Count(n) => {
                        let cycles = ((anim.elapsed - anim.delay) / anim.duration) as u32;
                        if cycles < n {
                            self.active_animations.push(anim);
                        }
                    }
                    AnimationRepeat::Once => {
                        // 완료됨
                    }
                }
            } else {
                self.active_animations.push(anim);
            }
        }

        completed_events
    }

    /// 애니메이션 재생
    pub fn play(&mut self, animation: ActiveAnimation) {
        // 같은 위젯의 같은 이름 애니메이션 제거
        self.active_animations.retain(|a| {
            !(a.widget_id == animation.widget_id && a.name == animation.name)
        });
        self.active_animations.push(animation);
    }

    /// 애니메이션 중지
    pub fn stop(&mut self, widget_id: &str, name: Option<&str>) {
        self.active_animations.retain(|a| {
            if a.widget_id != widget_id {
                return true;
            }
            if let Some(n) = name {
                a.name != n
            } else {
                false
            }
        });
    }

    /// 모든 애니메이션 중지
    pub fn stop_all(&mut self) {
        self.active_animations.clear();
    }

    /// 활성 애니메이션 개수
    pub fn active_count(&self) -> usize {
        self.active_animations.len()
    }
}

impl Default for AnimationSystem {
    fn default() -> Self {
        Self::new()
    }
}

// Helper functions
fn find_widget_by_id_mut<'a>(widget: &'a mut Widget, id: &str) -> Option<&'a mut Widget> {
    if widget.id.as_ref().map(|s| s.as_str()) == Some(id) {
        return Some(widget);
    }
    for child in &mut widget.children {
        if let Some(found) = find_widget_by_id_mut(child, id) {
            return Some(found);
        }
    }
    None
}

fn apply_animated_value(widget: &mut Widget, property: &AnimatedProperty, value: &AnimatedValue) {
    match property {
        AnimatedProperty::Opacity => {
            widget.style.opacity = value.as_float();
        }
        AnimatedProperty::OffsetX => {
            widget.layout.offset.0 = value.as_float();
        }
        AnimatedProperty::OffsetY => {
            widget.layout.offset.1 = value.as_float();
        }
        AnimatedProperty::ScaleX => {
            widget.style.scale.0 = value.as_float();
        }
        AnimatedProperty::ScaleY => {
            widget.style.scale.1 = value.as_float();
        }
        AnimatedProperty::Rotation => {
            widget.style.rotation = value.as_float();
        }
        AnimatedProperty::Width => {
            if let Size::Fixed(_, h) = widget.layout.size {
                widget.layout.size = Size::Fixed(value.as_float(), h);
            }
        }
        AnimatedProperty::Height => {
            if let Size::Fixed(w, _) = widget.layout.size {
                widget.layout.size = Size::Fixed(w, value.as_float());
            }
        }
        AnimatedProperty::BackgroundColor => {
            let [r, g, b, a] = value.as_color();
            widget.style.background_color = Some(Color::Rgba(r, g, b, a));
        }
        AnimatedProperty::TextColor => {
            let [r, g, b, a] = value.as_color();
            widget.style.text_color = Some(Color::Rgba(r, g, b, a));
        }
        AnimatedProperty::BorderColor => {
            let [r, g, b, a] = value.as_color();
            widget.style.border_color = Some(Color::Rgba(r, g, b, a));
        }
        AnimatedProperty::BorderRadius => {
            widget.style.border_radius = value.as_float();
        }
        AnimatedProperty::BorderWidth => {
            widget.style.border_width = value.as_float();
        }
    }
}
