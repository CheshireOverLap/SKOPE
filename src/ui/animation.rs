// SKOPE UI - Animation System
#![allow(dead_code)]

use super::types::*;
use super::style::Style;

/// 활성 애니메이션
#[derive(Debug, Clone)]
pub struct ActiveAnimation {
    /// 대상 위젯 ID
    pub widget_id: String,
    /// 애니메이션 이름
    pub name: String,
    /// 경과 시간
    pub elapsed: f32,
    /// 총 지연 시간
    pub delay: f32,
    /// 총 지속 시간
    pub duration: f32,
    /// 이징 함수
    pub easing: Easing,
    /// 애니메이션 트랙들
    pub tracks: Vec<AnimationTrack>,
    /// 반복 설정
    pub repeat: AnimationRepeat,
    /// 완료 콜백 이벤트
    pub on_complete: Option<String>,
}

impl ActiveAnimation {
    /// 현재 진행률 (0.0 - 1.0)
    pub fn progress(&self) -> f32 {
        if self.elapsed < self.delay {
            return 0.0;
        }

        let active_time = self.elapsed - self.delay;
        let raw_progress = (active_time / self.duration).clamp(0.0, 1.0);

        self.easing.apply(raw_progress)
    }

    /// 완료 여부
    pub fn is_complete(&self) -> bool {
        match self.repeat {
            AnimationRepeat::Once => self.elapsed >= self.delay + self.duration,
            AnimationRepeat::Loop => false,
            AnimationRepeat::Count(n) => {
                let cycles = ((self.elapsed - self.delay) / self.duration) as u32;
                cycles >= n
            }
            AnimationRepeat::PingPong => false,
        }
    }
}

/// 애니메이션 반복 설정
#[derive(Debug, Clone, Copy, Default)]
pub enum AnimationRepeat {
    #[default]
    Once,
    Loop,
    Count(u32),
    PingPong,
}

/// 애니메이션 트랙 (특정 속성의 애니메이션)
#[derive(Debug, Clone)]
pub struct AnimationTrack {
    /// 애니메이션할 속성
    pub property: AnimatedProperty,
    /// 시작 값
    pub from: AnimatedValue,
    /// 끝 값
    pub to: AnimatedValue,
    /// 키프레임들 (선택적)
    pub keyframes: Option<Vec<Keyframe>>,
}

impl AnimationTrack {
    /// 현재 진행률에서의 값 계산
    pub fn value_at(&self, progress: f32) -> AnimatedValue {
        if let Some(ref keyframes) = self.keyframes {
            // 키프레임 보간
            self.interpolate_keyframes(keyframes, progress)
        } else {
            // 단순 선형 보간
            self.from.lerp(&self.to, progress)
        }
    }

    fn interpolate_keyframes(&self, keyframes: &[Keyframe], progress: f32) -> AnimatedValue {
        if keyframes.is_empty() {
            return self.from.lerp(&self.to, progress);
        }

        // 현재 진행률에 해당하는 키프레임 구간 찾기
        let mut prev_kf = &Keyframe {
            time: 0.0,
            value: self.from.clone(),
            easing: None,
        };

        for kf in keyframes {
            if progress <= kf.time {
                // prev_kf와 kf 사이 보간
                let segment_progress = if kf.time == prev_kf.time {
                    1.0
                } else {
                    (progress - prev_kf.time) / (kf.time - prev_kf.time)
                };

                let eased_progress = kf.easing
                    .unwrap_or(Easing::Linear)
                    .apply(segment_progress);

                return prev_kf.value.lerp(&kf.value, eased_progress);
            }
            prev_kf = kf;
        }

        // 마지막 키프레임 이후
        self.to.clone()
    }
}

/// 키프레임
#[derive(Debug, Clone)]
pub struct Keyframe {
    /// 시간 (0.0 - 1.0 정규화)
    pub time: f32,
    /// 값
    pub value: AnimatedValue,
    /// 이 키프레임으로 전환할 때 사용할 이징
    pub easing: Option<Easing>,
}

/// 애니메이션 가능한 속성
#[derive(Debug, Clone)]
pub enum AnimatedProperty {
    Opacity,
    OffsetX,
    OffsetY,
    ScaleX,
    ScaleY,
    Rotation,
    Width,
    Height,
    BackgroundColor,
    TextColor,
    BorderColor,
    BorderRadius,
    BorderWidth,
}

/// 애니메이션 값
#[derive(Debug, Clone)]
pub enum AnimatedValue {
    Float(f32),
    Vec2(f32, f32),
    Color(f32, f32, f32, f32),
}

impl AnimatedValue {
    /// 두 값 사이 선형 보간
    pub fn lerp(&self, other: &AnimatedValue, t: f32) -> AnimatedValue {
        match (self, other) {
            (AnimatedValue::Float(a), AnimatedValue::Float(b)) => {
                AnimatedValue::Float(a + (b - a) * t)
            }
            (AnimatedValue::Vec2(ax, ay), AnimatedValue::Vec2(bx, by)) => {
                AnimatedValue::Vec2(
                    ax + (bx - ax) * t,
                    ay + (by - ay) * t,
                )
            }
            (AnimatedValue::Color(ar, ag, ab, aa), AnimatedValue::Color(br, bg, bb, ba)) => {
                AnimatedValue::Color(
                    ar + (br - ar) * t,
                    ag + (bg - ag) * t,
                    ab + (bb - ab) * t,
                    aa + (ba - aa) * t,
                )
            }
            _ => self.clone(),
        }
    }

    pub fn as_float(&self) -> f32 {
        match self {
            AnimatedValue::Float(f) => *f,
            _ => 0.0,
        }
    }

    pub fn as_vec2(&self) -> (f32, f32) {
        match self {
            AnimatedValue::Vec2(x, y) => (*x, *y),
            _ => (0.0, 0.0),
        }
    }

    pub fn as_color(&self) -> [f32; 4] {
        match self {
            AnimatedValue::Color(r, g, b, a) => [*r, *g, *b, *a],
            _ => [1.0, 1.0, 1.0, 1.0],
        }
    }
}

/// 애니메이션 빌더
pub struct AnimationBuilder {
    widget_id: String,
    name: String,
    duration: f32,
    delay: f32,
    easing: Easing,
    tracks: Vec<AnimationTrack>,
    repeat: AnimationRepeat,
    on_complete: Option<String>,
}

impl AnimationBuilder {
    pub fn new(widget_id: impl Into<String>) -> Self {
        Self {
            widget_id: widget_id.into(),
            name: String::new(),
            duration: 0.3,
            delay: 0.0,
            easing: Easing::EaseOut,
            tracks: Vec::new(),
            repeat: AnimationRepeat::Once,
            on_complete: None,
        }
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn duration(mut self, seconds: f32) -> Self {
        self.duration = seconds;
        self
    }

    pub fn delay(mut self, seconds: f32) -> Self {
        self.delay = seconds;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn repeat(mut self, repeat: AnimationRepeat) -> Self {
        self.repeat = repeat;
        self
    }

    pub fn on_complete(mut self, event: impl Into<String>) -> Self {
        self.on_complete = Some(event.into());
        self
    }

    /// Fade 애니메이션 추가
    pub fn fade(mut self, from: f32, to: f32) -> Self {
        self.tracks.push(AnimationTrack {
            property: AnimatedProperty::Opacity,
            from: AnimatedValue::Float(from),
            to: AnimatedValue::Float(to),
            keyframes: None,
        });
        self
    }

    /// 위치 이동 애니메이션 추가
    pub fn move_to(mut self, from: (f32, f32), to: (f32, f32)) -> Self {
        self.tracks.push(AnimationTrack {
            property: AnimatedProperty::OffsetX,
            from: AnimatedValue::Float(from.0),
            to: AnimatedValue::Float(to.0),
            keyframes: None,
        });
        self.tracks.push(AnimationTrack {
            property: AnimatedProperty::OffsetY,
            from: AnimatedValue::Float(from.1),
            to: AnimatedValue::Float(to.1),
            keyframes: None,
        });
        self
    }

    /// 스케일 애니메이션 추가
    pub fn scale(mut self, from: (f32, f32), to: (f32, f32)) -> Self {
        self.tracks.push(AnimationTrack {
            property: AnimatedProperty::ScaleX,
            from: AnimatedValue::Float(from.0),
            to: AnimatedValue::Float(to.0),
            keyframes: None,
        });
        self.tracks.push(AnimationTrack {
            property: AnimatedProperty::ScaleY,
            from: AnimatedValue::Float(from.1),
            to: AnimatedValue::Float(to.1),
            keyframes: None,
        });
        self
    }

    /// 회전 애니메이션 추가
    pub fn rotate(mut self, from: f32, to: f32) -> Self {
        self.tracks.push(AnimationTrack {
            property: AnimatedProperty::Rotation,
            from: AnimatedValue::Float(from),
            to: AnimatedValue::Float(to),
            keyframes: None,
        });
        self
    }

    /// 색상 애니메이션 추가
    pub fn color(mut self, property: AnimatedProperty, from: [f32; 4], to: [f32; 4]) -> Self {
        self.tracks.push(AnimationTrack {
            property,
            from: AnimatedValue::Color(from[0], from[1], from[2], from[3]),
            to: AnimatedValue::Color(to[0], to[1], to[2], to[3]),
            keyframes: None,
        });
        self
    }

    /// 커스텀 트랙 추가
    pub fn track(mut self, track: AnimationTrack) -> Self {
        self.tracks.push(track);
        self
    }

    /// 빌드
    pub fn build(self) -> ActiveAnimation {
        ActiveAnimation {
            widget_id: self.widget_id,
            name: self.name,
            elapsed: 0.0,
            delay: self.delay,
            duration: self.duration,
            easing: self.easing,
            tracks: self.tracks,
            repeat: self.repeat,
            on_complete: self.on_complete,
        }
    }
}

/// 상태 전환 애니메이션 생성
pub fn create_state_transition(
    widget_id: &str,
    from_style: &Style,
    to_style: &Style,
    transition: &Transition,
) -> ActiveAnimation {
    let mut tracks = Vec::new();

    // 불투명도
    if from_style.opacity != to_style.opacity {
        tracks.push(AnimationTrack {
            property: AnimatedProperty::Opacity,
            from: AnimatedValue::Float(from_style.opacity),
            to: AnimatedValue::Float(to_style.opacity),
            keyframes: None,
        });
    }

    // 스케일
    if from_style.scale != to_style.scale {
        tracks.push(AnimationTrack {
            property: AnimatedProperty::ScaleX,
            from: AnimatedValue::Float(from_style.scale.0),
            to: AnimatedValue::Float(to_style.scale.0),
            keyframes: None,
        });
        tracks.push(AnimationTrack {
            property: AnimatedProperty::ScaleY,
            from: AnimatedValue::Float(from_style.scale.1),
            to: AnimatedValue::Float(to_style.scale.1),
            keyframes: None,
        });
    }

    // 회전
    if from_style.rotation != to_style.rotation {
        tracks.push(AnimationTrack {
            property: AnimatedProperty::Rotation,
            from: AnimatedValue::Float(from_style.rotation),
            to: AnimatedValue::Float(to_style.rotation),
            keyframes: None,
        });
    }

    // 배경색
    if from_style.background_color != to_style.background_color {
        if let (Some(from_color), Some(to_color)) = (from_style.background_color, to_style.background_color) {
            let from_rgba = from_color.to_rgba();
            let to_rgba = to_color.to_rgba();
            tracks.push(AnimationTrack {
                property: AnimatedProperty::BackgroundColor,
                from: AnimatedValue::Color(from_rgba[0], from_rgba[1], from_rgba[2], from_rgba[3]),
                to: AnimatedValue::Color(to_rgba[0], to_rgba[1], to_rgba[2], to_rgba[3]),
                keyframes: None,
            });
        }
    }

    ActiveAnimation {
        widget_id: widget_id.to_string(),
        name: "state_transition".to_string(),
        elapsed: 0.0,
        delay: transition.delay,
        duration: transition.duration,
        easing: transition.easing,
        tracks,
        repeat: AnimationRepeat::Once,
        on_complete: None,
    }
}

/// 미리 정의된 애니메이션 프리셋
pub mod presets {
    use super::*;

    /// 페이드 인
    pub fn fade_in(widget_id: &str, duration: f32) -> ActiveAnimation {
        AnimationBuilder::new(widget_id)
            .name("fade_in")
            .duration(duration)
            .fade(0.0, 1.0)
            .build()
    }

    /// 페이드 아웃
    pub fn fade_out(widget_id: &str, duration: f32) -> ActiveAnimation {
        AnimationBuilder::new(widget_id)
            .name("fade_out")
            .duration(duration)
            .fade(1.0, 0.0)
            .build()
    }

    /// 왼쪽에서 슬라이드 인
    pub fn slide_in_left(widget_id: &str, distance: f32, duration: f32) -> ActiveAnimation {
        AnimationBuilder::new(widget_id)
            .name("slide_in_left")
            .duration(duration)
            .easing(Easing::EaseOutCubic)
            .move_to((-distance, 0.0), (0.0, 0.0))
            .fade(0.0, 1.0)
            .build()
    }

    /// 오른쪽에서 슬라이드 인
    pub fn slide_in_right(widget_id: &str, distance: f32, duration: f32) -> ActiveAnimation {
        AnimationBuilder::new(widget_id)
            .name("slide_in_right")
            .duration(duration)
            .easing(Easing::EaseOutCubic)
            .move_to((distance, 0.0), (0.0, 0.0))
            .fade(0.0, 1.0)
            .build()
    }

    /// 위에서 슬라이드 인
    pub fn slide_in_top(widget_id: &str, distance: f32, duration: f32) -> ActiveAnimation {
        AnimationBuilder::new(widget_id)
            .name("slide_in_top")
            .duration(duration)
            .easing(Easing::EaseOutCubic)
            .move_to((0.0, -distance), (0.0, 0.0))
            .fade(0.0, 1.0)
            .build()
    }

    /// 아래에서 슬라이드 인
    pub fn slide_in_bottom(widget_id: &str, distance: f32, duration: f32) -> ActiveAnimation {
        AnimationBuilder::new(widget_id)
            .name("slide_in_bottom")
            .duration(duration)
            .easing(Easing::EaseOutCubic)
            .move_to((0.0, distance), (0.0, 0.0))
            .fade(0.0, 1.0)
            .build()
    }

    /// 팝 인 (스케일)
    pub fn pop_in(widget_id: &str, duration: f32) -> ActiveAnimation {
        AnimationBuilder::new(widget_id)
            .name("pop_in")
            .duration(duration)
            .easing(Easing::EaseOutBack)
            .scale((0.0, 0.0), (1.0, 1.0))
            .fade(0.0, 1.0)
            .build()
    }

    /// 팝 아웃 (스케일)
    pub fn pop_out(widget_id: &str, duration: f32) -> ActiveAnimation {
        AnimationBuilder::new(widget_id)
            .name("pop_out")
            .duration(duration)
            .easing(Easing::EaseIn)
            .scale((1.0, 1.0), (0.0, 0.0))
            .fade(1.0, 0.0)
            .build()
    }

    /// 흔들기 (에러 표시용)
    pub fn shake(widget_id: &str) -> ActiveAnimation {
        let track = AnimationTrack {
            property: AnimatedProperty::OffsetX,
            from: AnimatedValue::Float(0.0),
            to: AnimatedValue::Float(0.0),
            keyframes: Some(vec![
                Keyframe { time: 0.0, value: AnimatedValue::Float(0.0), easing: None },
                Keyframe { time: 0.1, value: AnimatedValue::Float(-10.0), easing: None },
                Keyframe { time: 0.2, value: AnimatedValue::Float(10.0), easing: None },
                Keyframe { time: 0.3, value: AnimatedValue::Float(-8.0), easing: None },
                Keyframe { time: 0.4, value: AnimatedValue::Float(8.0), easing: None },
                Keyframe { time: 0.5, value: AnimatedValue::Float(-4.0), easing: None },
                Keyframe { time: 0.6, value: AnimatedValue::Float(4.0), easing: None },
                Keyframe { time: 0.7, value: AnimatedValue::Float(0.0), easing: None },
            ]),
        };

        AnimationBuilder::new(widget_id)
            .name("shake")
            .duration(0.5)
            .track(track)
            .build()
    }

    /// 펄스 (주목 끌기)
    pub fn pulse(widget_id: &str) -> ActiveAnimation {
        let track = AnimationTrack {
            property: AnimatedProperty::ScaleX,
            from: AnimatedValue::Float(1.0),
            to: AnimatedValue::Float(1.0),
            keyframes: Some(vec![
                Keyframe { time: 0.0, value: AnimatedValue::Float(1.0), easing: Some(Easing::EaseInOut) },
                Keyframe { time: 0.5, value: AnimatedValue::Float(1.1), easing: Some(Easing::EaseInOut) },
                Keyframe { time: 1.0, value: AnimatedValue::Float(1.0), easing: None },
            ]),
        };

        AnimationBuilder::new(widget_id)
            .name("pulse")
            .duration(0.6)
            .repeat(AnimationRepeat::Loop)
            .track(track.clone())
            .track(AnimationTrack {
                property: AnimatedProperty::ScaleY,
                ..track
            })
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animation_progress() {
        let anim = AnimationBuilder::new("test")
            .duration(1.0)
            .fade(0.0, 1.0)
            .build();

        assert_eq!(anim.progress(), 0.0);
    }

    #[test]
    fn test_easing_linear() {
        assert_eq!(Easing::Linear.apply(0.0), 0.0);
        assert_eq!(Easing::Linear.apply(0.5), 0.5);
        assert_eq!(Easing::Linear.apply(1.0), 1.0);
    }

    #[test]
    fn test_animated_value_lerp() {
        let from = AnimatedValue::Float(0.0);
        let to = AnimatedValue::Float(100.0);

        assert_eq!(from.lerp(&to, 0.5).as_float(), 50.0);
    }
}
