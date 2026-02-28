//! Animation System - 애니메이션 (언리얼 Slate의 FCurveSequence, FSlateAnimation)
//!
//! UI 애니메이션을 위한 커브, 시퀀스, 보간 시스템입니다.

use std::f32::consts::PI;

// ============================================================================
// EasingFunction
// ============================================================================

/// 이징 함수 종류
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EasingFunction {
    /// 선형
    #[default]
    Linear,
    /// 부드러운 시작
    EaseIn,
    /// 부드러운 끝
    EaseOut,
    /// 부드러운 시작과 끝
    EaseInOut,
    /// 빠른 시작
    QuadIn,
    /// 빠른 끝
    QuadOut,
    /// 빠른 시작과 끝
    QuadInOut,
    /// 큐빅 시작
    CubicIn,
    /// 큐빅 끝
    CubicOut,
    /// 큐빅 시작과 끝
    CubicInOut,
    /// 탄성 (튕김)
    ElasticOut,
    /// 바운스
    BounceOut,
    /// 뒤로 갔다 오기
    BackOut,
}

impl EasingFunction {
    /// 이징 적용 (0.0 ~ 1.0 입력, 0.0 ~ 1.0 출력)
    pub fn ease(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);

        match self {
            Self::Linear => t,

            Self::EaseIn => t * t,
            Self::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Self::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }

            Self::QuadIn => t * t,
            Self::QuadOut => 1.0 - (1.0 - t).powi(2),
            Self::QuadInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }

            Self::CubicIn => t * t * t,
            Self::CubicOut => 1.0 - (1.0 - t).powi(3),
            Self::CubicInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }

            Self::ElasticOut => {
                if t == 0.0 {
                    0.0
                } else if t == 1.0 {
                    1.0
                } else {
                    let c4 = (2.0 * PI) / 3.0;
                    2.0_f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0
                }
            }

            Self::BounceOut => {
                let n1 = 7.5625;
                let d1 = 2.75;

                if t < 1.0 / d1 {
                    n1 * t * t
                } else if t < 2.0 / d1 {
                    let t = t - 1.5 / d1;
                    n1 * t * t + 0.75
                } else if t < 2.5 / d1 {
                    let t = t - 2.25 / d1;
                    n1 * t * t + 0.9375
                } else {
                    let t = t - 2.625 / d1;
                    n1 * t * t + 0.984375
                }
            }

            Self::BackOut => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
            }
        }
    }
}

// ============================================================================
// Interpolator trait (UE5 TAttributeInterpolator 대응)
// ============================================================================

/// 모든 delta-time 기반 애니메이션의 공통 인터페이스
///
/// UE5의 TAttributeInterpolator에 해당합니다.
/// `tick_interpolator()` 메서드명으로 기존 `tick()` 메서드와 충돌을 방지합니다.
pub trait Interpolator: Send + Sync {
    /// 프레임 업데이트. true 반환 = 애니메이션 완료 (매니저가 제거)
    fn tick_interpolator(&mut self, delta_time: f32) -> bool;
    /// 현재 재생 중인지
    fn is_interpolating(&self) -> bool;
}

// ============================================================================
// AnimationCurve
// ============================================================================

/// 애니메이션 커브
#[derive(Debug, Clone)]
pub struct AnimationCurve {
    /// 시작 시간 (전체 시퀀스 내)
    pub start_time: f32,
    /// 지속 시간
    pub duration: f32,
    /// 이징 함수
    pub easing: EasingFunction,
    /// 시작 값
    pub start_value: f32,
    /// 끝 값
    pub end_value: f32,
}

impl AnimationCurve {
    /// 새 커브 생성
    pub fn new(duration: f32) -> Self {
        Self {
            start_time: 0.0,
            duration,
            easing: EasingFunction::Linear,
            start_value: 0.0,
            end_value: 1.0,
        }
    }

    /// 시작 시간 설정
    pub fn at(mut self, start_time: f32) -> Self {
        self.start_time = start_time;
        self
    }

    /// 이징 설정
    pub fn with_easing(mut self, easing: EasingFunction) -> Self {
        self.easing = easing;
        self
    }

    /// 값 범위 설정
    pub fn from_to(mut self, start: f32, end: f32) -> Self {
        self.start_value = start;
        self.end_value = end;
        self
    }

    /// 현재 시간에서 값 계산
    pub fn evaluate(&self, time: f32) -> f32 {
        if time < self.start_time {
            return self.start_value;
        }

        let local_time = time - self.start_time;
        if local_time >= self.duration {
            return self.end_value;
        }

        let t = local_time / self.duration;
        let eased_t = self.easing.ease(t);

        self.start_value + (self.end_value - self.start_value) * eased_t
    }

    /// 끝 시간
    pub fn end_time(&self) -> f32 {
        self.start_time + self.duration
    }

    /// 완료 여부
    pub fn is_complete(&self, time: f32) -> bool {
        time >= self.end_time()
    }
}

// ============================================================================
// CurveSequence
// ============================================================================

/// 커브 시퀀스 (여러 커브의 조합)
///
/// 언리얼의 FCurveSequence에 해당합니다.
#[derive(Debug, Clone)]
pub struct CurveSequence {
    /// 커브들
    curves: Vec<AnimationCurve>,
    /// 시작 시간
    start_time: f64,
    /// 재생 중인지
    is_playing: bool,
    /// 역재생 중인지
    is_reversed: bool,
    /// 루프 여부
    is_looping: bool,
    /// 전체 지속 시간 (자동 계산)
    total_duration: f32,
}

impl Default for CurveSequence {
    fn default() -> Self {
        Self::new()
    }
}

impl CurveSequence {
    /// 새 시퀀스 생성
    pub fn new() -> Self {
        Self {
            curves: Vec::new(),
            start_time: 0.0,
            is_playing: false,
            is_reversed: false,
            is_looping: false,
            total_duration: 0.0,
        }
    }

    /// 커브 추가
    pub fn add_curve(&mut self, curve: AnimationCurve) -> usize {
        let index = self.curves.len();
        self.total_duration = self.total_duration.max(curve.end_time());
        self.curves.push(curve);
        index
    }

    /// 루프 설정
    pub fn set_looping(&mut self, looping: bool) {
        self.is_looping = looping;
    }

    /// 재생 시작
    pub fn play(&mut self, current_time: f64) {
        self.start_time = current_time;
        self.is_playing = true;
        self.is_reversed = false;
    }

    /// 역재생 시작
    pub fn play_reverse(&mut self, current_time: f64) {
        self.start_time = current_time;
        self.is_playing = true;
        self.is_reversed = true;
    }

    /// 일시정지
    pub fn pause(&mut self) {
        self.is_playing = false;
    }

    /// 재개
    pub fn resume(&mut self) {
        self.is_playing = true;
    }

    /// 정지 (처음으로)
    pub fn stop(&mut self) {
        self.is_playing = false;
        self.start_time = 0.0;
    }

    /// 즉시 끝으로
    pub fn jump_to_end(&mut self) {
        self.is_playing = false;
    }

    /// 재생 중인지
    pub fn is_playing(&self) -> bool {
        self.is_playing
    }

    /// 완료 여부
    pub fn is_at_end(&self, current_time: f64) -> bool {
        if !self.is_playing {
            return true;
        }
        let elapsed = (current_time - self.start_time) as f32;
        elapsed >= self.total_duration
    }

    /// 완료 여부 (루프 미고려)
    pub fn is_complete(&self, current_time: f64) -> bool {
        !self.is_looping && self.is_at_end(current_time)
    }

    /// 현재 시간에서 특정 커브의 값 계산
    pub fn get_curve_value(&self, curve_index: usize, current_time: f64) -> f32 {
        let Some(curve) = self.curves.get(curve_index) else {
            return 0.0;
        };

        if !self.is_playing && self.start_time == 0.0 {
            return curve.start_value;
        }

        let mut elapsed = (current_time - self.start_time) as f32;

        // 루프 처리
        if self.is_looping && self.total_duration > 0.0 {
            elapsed = elapsed % self.total_duration;
        }

        // 역재생
        if self.is_reversed {
            elapsed = self.total_duration - elapsed;
        }

        curve.evaluate(elapsed.max(0.0))
    }

    /// 현재 보간값 (0.0 ~ 1.0)
    pub fn get_lerp(&self, current_time: f64) -> f32 {
        if self.total_duration <= 0.0 {
            return if self.is_reversed { 0.0 } else { 1.0 };
        }

        let mut elapsed = (current_time - self.start_time) as f32;

        if self.is_looping {
            elapsed = elapsed % self.total_duration;
        }

        let t = (elapsed / self.total_duration).clamp(0.0, 1.0);

        if self.is_reversed {
            1.0 - t
        } else {
            t
        }
    }

    /// 전체 지속 시간
    pub fn total_duration(&self) -> f32 {
        self.total_duration
    }
}

// ============================================================================
// SimpleAnimation
// ============================================================================

/// 간단한 단일 값 애니메이션
///
/// 복잡한 시퀀스 없이 단일 값을 애니메이션할 때 사용합니다.
#[derive(Debug, Clone)]
pub struct SimpleAnimation {
    /// 시작 값
    start_value: f32,
    /// 목표 값
    target_value: f32,
    /// 현재 값
    current_value: f32,
    /// 지속 시간
    duration: f32,
    /// 경과 시간
    elapsed: f32,
    /// 이징 함수
    easing: EasingFunction,
    /// 재생 중인지
    is_playing: bool,
}

impl SimpleAnimation {
    /// 새 애니메이션 생성
    pub fn new(initial_value: f32) -> Self {
        Self {
            start_value: initial_value,
            target_value: initial_value,
            current_value: initial_value,
            duration: 0.3,
            elapsed: 0.0,
            easing: EasingFunction::EaseInOut,
            is_playing: false,
        }
    }

    /// 목표 값으로 애니메이션 시작
    pub fn animate_to(&mut self, target: f32, duration: f32) {
        if (self.target_value - target).abs() < f32::EPSILON {
            return;
        }

        self.start_value = self.current_value;
        self.target_value = target;
        self.duration = duration;
        self.elapsed = 0.0;
        self.is_playing = true;
    }

    /// 이징 설정
    pub fn with_easing(mut self, easing: EasingFunction) -> Self {
        self.easing = easing;
        self
    }

    /// 즉시 값 설정 (애니메이션 없이)
    pub fn set_immediately(&mut self, value: f32) {
        self.start_value = value;
        self.target_value = value;
        self.current_value = value;
        self.is_playing = false;
    }

    /// 틱 (매 프레임 호출)
    pub fn tick(&mut self, delta_time: f32) {
        if !self.is_playing {
            return;
        }

        self.elapsed += delta_time;

        if self.elapsed >= self.duration {
            self.current_value = self.target_value;
            self.is_playing = false;
        } else {
            let t = self.elapsed / self.duration;
            let eased_t = self.easing.ease(t);
            self.current_value = self.start_value + (self.target_value - self.start_value) * eased_t;
        }
    }

    /// 현재 값
    pub fn value(&self) -> f32 {
        self.current_value
    }

    /// 재생 중인지
    pub fn is_playing(&self) -> bool {
        self.is_playing
    }

    /// 완료 여부
    pub fn is_complete(&self) -> bool {
        !self.is_playing
    }
}

impl Interpolator for SimpleAnimation {
    fn tick_interpolator(&mut self, delta_time: f32) -> bool {
        self.tick(delta_time);
        !self.is_playing()
    }
    fn is_interpolating(&self) -> bool {
        self.is_playing()
    }
}

// ============================================================================
// AnimatedColor
// ============================================================================

use crate::core::Color;

/// 색상 애니메이션
#[derive(Debug, Clone)]
pub struct AnimatedColor {
    r: SimpleAnimation,
    g: SimpleAnimation,
    b: SimpleAnimation,
    a: SimpleAnimation,
}

impl AnimatedColor {
    /// 새 색상 애니메이션
    pub fn new(initial: Color) -> Self {
        Self {
            r: SimpleAnimation::new(initial.r),
            g: SimpleAnimation::new(initial.g),
            b: SimpleAnimation::new(initial.b),
            a: SimpleAnimation::new(initial.a),
        }
    }

    /// 목표 색상으로 애니메이션
    pub fn animate_to(&mut self, target: Color, duration: f32) {
        self.r.animate_to(target.r, duration);
        self.g.animate_to(target.g, duration);
        self.b.animate_to(target.b, duration);
        self.a.animate_to(target.a, duration);
    }

    /// 이징 설정
    pub fn with_easing(mut self, easing: EasingFunction) -> Self {
        self.r = self.r.with_easing(easing);
        self.g = self.g.with_easing(easing);
        self.b = self.b.with_easing(easing);
        self.a = self.a.with_easing(easing);
        self
    }

    /// 즉시 설정
    pub fn set_immediately(&mut self, color: Color) {
        self.r.set_immediately(color.r);
        self.g.set_immediately(color.g);
        self.b.set_immediately(color.b);
        self.a.set_immediately(color.a);
    }

    /// 틱
    pub fn tick(&mut self, delta_time: f32) {
        self.r.tick(delta_time);
        self.g.tick(delta_time);
        self.b.tick(delta_time);
        self.a.tick(delta_time);
    }

    /// 현재 색상
    pub fn value(&self) -> Color {
        Color::rgba(self.r.value(), self.g.value(), self.b.value(), self.a.value())
    }

    /// 재생 중인지
    pub fn is_playing(&self) -> bool {
        self.r.is_playing() || self.g.is_playing() || self.b.is_playing() || self.a.is_playing()
    }
}

// ============================================================================
// AnimatedVec2
// ============================================================================

use glam::Vec2;

/// Vec2 애니메이션
#[derive(Debug, Clone)]
pub struct AnimatedVec2 {
    x: SimpleAnimation,
    y: SimpleAnimation,
}

impl AnimatedVec2 {
    /// 새 Vec2 애니메이션
    pub fn new(initial: Vec2) -> Self {
        Self {
            x: SimpleAnimation::new(initial.x),
            y: SimpleAnimation::new(initial.y),
        }
    }

    /// 목표로 애니메이션
    pub fn animate_to(&mut self, target: Vec2, duration: f32) {
        self.x.animate_to(target.x, duration);
        self.y.animate_to(target.y, duration);
    }

    /// 이징 설정
    pub fn with_easing(mut self, easing: EasingFunction) -> Self {
        self.x = self.x.with_easing(easing);
        self.y = self.y.with_easing(easing);
        self
    }

    /// 즉시 설정
    pub fn set_immediately(&mut self, pos: Vec2) {
        self.x.set_immediately(pos.x);
        self.y.set_immediately(pos.y);
    }

    /// 틱
    pub fn tick(&mut self, delta_time: f32) {
        self.x.tick(delta_time);
        self.y.tick(delta_time);
    }

    /// 현재 값
    pub fn value(&self) -> Vec2 {
        Vec2::new(self.x.value(), self.y.value())
    }

    /// 재생 중인지
    pub fn is_playing(&self) -> bool {
        self.x.is_playing() || self.y.is_playing()
    }
}

// ============================================================================
// Interpolation Helpers
// ============================================================================

/// 선형 보간
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// 색상 선형 보간
pub fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    Color::rgba(
        lerp(a.r, b.r, t),
        lerp(a.g, b.g, t),
        lerp(a.b, b.b, t),
        lerp(a.a, b.a, t),
    )
}

/// Vec2 선형 보간
pub fn lerp_vec2(a: Vec2, b: Vec2, t: f32) -> Vec2 {
    Vec2::new(lerp(a.x, b.x, t), lerp(a.y, b.y, t))
}

/// 스무스 스텝 (0에서 1 사이에서 부드럽게 전환)
pub fn smooth_step(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// 더 부드러운 스텝
pub fn smoother_step(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

// ============================================================================
// ArriveInterpolator
// ============================================================================

/// Arrive 보간기 (언리얼 TAttributeInterpolator<Arrive>)
///
/// 목표 값에 부드럽게 도착하는 보간기입니다.
/// 감속하면서 목표에 접근합니다.
#[derive(Debug, Clone)]
pub struct ArriveInterpolator {
    /// 현재 값
    current: f32,
    /// 목표 값
    target: f32,
    /// 속도 (초당 변화량)
    velocity: f32,
    /// 감속 거리 (이 거리 내에서 감속 시작)
    arrival_distance: f32,
    /// 최대 속도
    max_speed: f32,
    /// 도착 임계값 (이 이하면 도착한 것으로 간주)
    threshold: f32,
}

impl ArriveInterpolator {
    /// 새 Arrive 보간기
    pub fn new(initial: f32) -> Self {
        Self {
            current: initial,
            target: initial,
            velocity: 0.0,
            arrival_distance: 100.0,
            max_speed: 500.0,
            threshold: 0.01,
        }
    }

    /// 도착 거리 설정
    pub fn with_arrival_distance(mut self, distance: f32) -> Self {
        self.arrival_distance = distance.max(1.0);
        self
    }

    /// 최대 속도 설정
    pub fn with_max_speed(mut self, speed: f32) -> Self {
        self.max_speed = speed.max(1.0);
        self
    }

    /// 임계값 설정
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold.max(0.001);
        self
    }

    /// 목표 값 설정
    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    /// 즉시 값 설정
    pub fn set_immediately(&mut self, value: f32) {
        self.current = value;
        self.target = value;
        self.velocity = 0.0;
    }

    /// 틱 (매 프레임 호출)
    pub fn tick(&mut self, delta_time: f32) {
        let distance = self.target - self.current;
        let abs_distance = distance.abs();

        if abs_distance < self.threshold {
            self.current = self.target;
            self.velocity = 0.0;
            return;
        }

        // 감속 계산
        let desired_speed = if abs_distance < self.arrival_distance {
            // 도착 거리 내에서 감속 (비례 속도)
            let proportional = self.max_speed * (abs_distance / self.arrival_distance);
            // 최소 속도 보장: Zeno 역설 방지 (비례 속도가 0에 수렴하여 도착 불가능한 현상)
            let min_speed = self.max_speed * 0.02;
            proportional.max(min_speed)
        } else {
            self.max_speed
        };

        // 방향에 따른 속도 적용
        let desired_velocity = if distance > 0.0 {
            desired_speed
        } else {
            -desired_speed
        };

        // 부드러운 속도 변화 (가속도 제한)
        let acceleration = 2000.0 * delta_time;
        if (desired_velocity - self.velocity).abs() < acceleration {
            self.velocity = desired_velocity;
        } else if desired_velocity > self.velocity {
            self.velocity += acceleration;
        } else {
            self.velocity -= acceleration;
        }

        self.current += self.velocity * delta_time;
    }

    /// 현재 값
    pub fn value(&self) -> f32 {
        self.current
    }

    /// 목표 값
    pub fn target(&self) -> f32 {
        self.target
    }

    /// 도착 여부
    pub fn has_arrived(&self) -> bool {
        (self.current - self.target).abs() < self.threshold
    }

    /// 재생 중인지
    pub fn is_playing(&self) -> bool {
        !self.has_arrived()
    }
}

impl Interpolator for ArriveInterpolator {
    fn tick_interpolator(&mut self, delta_time: f32) -> bool {
        self.tick(delta_time);
        self.has_arrived()
    }
    fn is_interpolating(&self) -> bool {
        self.is_playing()
    }
}

// ============================================================================
// VerletInterpolator
// ============================================================================

/// Verlet 보간기 (물리 기반 스프링)
///
/// 언리얼의 TAttributeInterpolator<Verlet>에 해당합니다.
/// 스프링과 댐핑을 사용한 물리 기반 애니메이션입니다.
#[derive(Debug, Clone)]
pub struct VerletInterpolator {
    /// 현재 값
    current: f32,
    /// 이전 값 (Verlet 적분용)
    previous: f32,
    /// 목표 값
    target: f32,
    /// 스프링 강도 (높을수록 빠르게 수렴)
    stiffness: f32,
    /// 댐핑 (0~1, 높을수록 진동 감소)
    damping: f32,
    /// 질량 (높을수록 관성 증가)
    mass: f32,
    /// 정지 임계값
    threshold: f32,
}

impl VerletInterpolator {
    /// 새 Verlet 보간기
    pub fn new(initial: f32) -> Self {
        Self {
            current: initial,
            previous: initial,
            target: initial,
            stiffness: 180.0,
            damping: 0.85,
            mass: 1.0,
            threshold: 0.01,
        }
    }

    /// 스프링 강도 설정
    pub fn with_stiffness(mut self, stiffness: f32) -> Self {
        self.stiffness = stiffness.max(1.0);
        self
    }

    /// 댐핑 설정
    pub fn with_damping(mut self, damping: f32) -> Self {
        self.damping = damping.clamp(0.0, 1.0);
        self
    }

    /// 질량 설정
    pub fn with_mass(mut self, mass: f32) -> Self {
        self.mass = mass.max(0.01);
        self
    }

    /// 임계값 설정
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold.max(0.001);
        self
    }

    /// 프리셋: 부드러운 스프링
    pub fn soft() -> Self {
        Self::new(0.0)
            .with_stiffness(100.0)
            .with_damping(0.8)
    }

    /// 프리셋: 단단한 스프링
    pub fn stiff() -> Self {
        Self::new(0.0)
            .with_stiffness(300.0)
            .with_damping(0.9)
    }

    /// 프리셋: 탄성 있는 스프링
    pub fn bouncy() -> Self {
        Self::new(0.0)
            .with_stiffness(200.0)
            .with_damping(0.6)
    }

    /// 목표 값 설정
    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    /// 즉시 값 설정
    pub fn set_immediately(&mut self, value: f32) {
        self.current = value;
        self.previous = value;
        self.target = value;
    }

    /// 충격 적용 (순간적인 속도 추가)
    pub fn apply_impulse(&mut self, impulse: f32) {
        // Verlet에서 속도는 current - previous로 표현됨
        self.previous -= impulse;
    }

    /// 틱 (매 프레임 호출)
    pub fn tick(&mut self, delta_time: f32) {
        // 스프링 힘 계산
        let displacement = self.target - self.current;
        let spring_force = displacement * self.stiffness;

        // 가속도 (F = ma)
        let acceleration = spring_force / self.mass;

        // Verlet 적분
        let velocity = (self.current - self.previous) * self.damping;
        self.previous = self.current;
        self.current += velocity + acceleration * delta_time * delta_time;

        // 정지 조건 확인
        let speed = (self.current - self.previous).abs() / delta_time.max(0.001);
        if displacement.abs() < self.threshold && speed < self.threshold {
            self.current = self.target;
            self.previous = self.target;
        }
    }

    /// 현재 값
    pub fn value(&self) -> f32 {
        self.current
    }

    /// 목표 값
    pub fn target(&self) -> f32 {
        self.target
    }

    /// 현재 속도 추정
    pub fn velocity(&self) -> f32 {
        self.current - self.previous
    }

    /// 안정화 여부 (진동이 멈췄는지)
    pub fn is_settled(&self) -> bool {
        (self.current - self.target).abs() < self.threshold
            && (self.current - self.previous).abs() < self.threshold * 0.1
    }

    /// 재생 중인지
    pub fn is_playing(&self) -> bool {
        !self.is_settled()
    }
}

impl Interpolator for VerletInterpolator {
    fn tick_interpolator(&mut self, delta_time: f32) -> bool {
        self.tick(delta_time);
        self.is_settled()
    }
    fn is_interpolating(&self) -> bool {
        self.is_playing()
    }
}

// ============================================================================
// VerletVec2
// ============================================================================

/// Vec2용 Verlet 보간기
#[derive(Debug, Clone)]
pub struct VerletVec2 {
    x: VerletInterpolator,
    y: VerletInterpolator,
}

impl VerletVec2 {
    /// 새 Vec2 Verlet
    pub fn new(initial: Vec2) -> Self {
        Self {
            x: VerletInterpolator::new(initial.x),
            y: VerletInterpolator::new(initial.y),
        }
    }

    /// 스프링 강도 설정
    pub fn with_stiffness(mut self, stiffness: f32) -> Self {
        self.x = self.x.with_stiffness(stiffness);
        self.y = self.y.with_stiffness(stiffness);
        self
    }

    /// 댐핑 설정
    pub fn with_damping(mut self, damping: f32) -> Self {
        self.x = self.x.with_damping(damping);
        self.y = self.y.with_damping(damping);
        self
    }

    /// 목표 설정
    pub fn set_target(&mut self, target: Vec2) {
        self.x.set_target(target.x);
        self.y.set_target(target.y);
    }

    /// 즉시 설정
    pub fn set_immediately(&mut self, pos: Vec2) {
        self.x.set_immediately(pos.x);
        self.y.set_immediately(pos.y);
    }

    /// 충격 적용
    pub fn apply_impulse(&mut self, impulse: Vec2) {
        self.x.apply_impulse(impulse.x);
        self.y.apply_impulse(impulse.y);
    }

    /// 틱
    pub fn tick(&mut self, delta_time: f32) {
        self.x.tick(delta_time);
        self.y.tick(delta_time);
    }

    /// 현재 값
    pub fn value(&self) -> Vec2 {
        Vec2::new(self.x.value(), self.y.value())
    }

    /// 재생 중인지
    pub fn is_playing(&self) -> bool {
        self.x.is_playing() || self.y.is_playing()
    }
}

// ============================================================================
// AnimationManager
// ============================================================================

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::core::{Attribute, ActiveTimers};

/// 애니메이션 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnimationId(pub u64);

impl AnimationId {
    /// 유효하지 않은 ID
    pub const INVALID: Self = Self(0);

    /// 새 고유 ID 생성
    pub fn new_unique() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// 유효한지
    pub fn is_valid(&self) -> bool {
        self.0 != 0
    }
}

/// 애니메이션 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationState {
    /// 대기 중
    Pending,
    /// 재생 중
    Playing,
    /// 일시정지
    Paused,
    /// 완료
    Completed,
}

/// 관리되는 애니메이션 엔트리
struct ManagedAnimation {
    sequence: CurveSequence,
    state: AnimationState,
    on_complete: Option<Box<dyn FnOnce() + Send + Sync>>,
}

/// 애니메이션 관리자
///
/// 전역 애니메이션 틱 관리와 완료 콜백을 처리합니다.
pub struct AnimationManager {
    /// 관리되는 애니메이션들
    animations: HashMap<AnimationId, ManagedAnimation>,
    /// 현재 시간
    current_time: f64,
    /// 다음 프레임에 제거할 애니메이션들
    pending_removal: Vec<AnimationId>,
}

impl Default for AnimationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AnimationManager {
    /// 새 관리자
    pub fn new() -> Self {
        Self {
            animations: HashMap::new(),
            current_time: 0.0,
            pending_removal: Vec::new(),
        }
    }

    /// 현재 시간
    pub fn current_time(&self) -> f64 {
        self.current_time
    }

    /// 애니메이션 등록
    pub fn register(&mut self, sequence: CurveSequence) -> AnimationId {
        let id = AnimationId::new_unique();
        self.animations.insert(id, ManagedAnimation {
            sequence,
            state: AnimationState::Pending,
            on_complete: None,
        });
        id
    }

    /// 완료 콜백과 함께 등록
    pub fn register_with_callback<F>(&mut self, sequence: CurveSequence, on_complete: F) -> AnimationId
    where
        F: FnOnce() + Send + Sync + 'static,
    {
        let id = AnimationId::new_unique();
        self.animations.insert(id, ManagedAnimation {
            sequence,
            state: AnimationState::Pending,
            on_complete: Some(Box::new(on_complete)),
        });
        id
    }

    /// 애니메이션 재생
    pub fn play(&mut self, id: AnimationId) {
        if let Some(anim) = self.animations.get_mut(&id) {
            anim.sequence.play(self.current_time);
            anim.state = AnimationState::Playing;
        }
    }

    /// 애니메이션 일시정지
    pub fn pause(&mut self, id: AnimationId) {
        if let Some(anim) = self.animations.get_mut(&id) {
            anim.sequence.pause();
            anim.state = AnimationState::Paused;
        }
    }

    /// 애니메이션 재개
    pub fn resume(&mut self, id: AnimationId) {
        if let Some(anim) = self.animations.get_mut(&id) {
            anim.sequence.resume();
            anim.state = AnimationState::Playing;
        }
    }

    /// 애니메이션 정지 및 제거
    pub fn stop(&mut self, id: AnimationId) {
        self.pending_removal.push(id);
    }

    /// 애니메이션 상태 조회
    pub fn get_state(&self, id: AnimationId) -> Option<AnimationState> {
        self.animations.get(&id).map(|a| a.state)
    }

    /// 커브 값 조회
    pub fn get_curve_value(&self, id: AnimationId, curve_index: usize) -> Option<f32> {
        self.animations.get(&id).map(|a| {
            a.sequence.get_curve_value(curve_index, self.current_time)
        })
    }

    /// 보간 값 조회 (0.0 ~ 1.0)
    pub fn get_lerp(&self, id: AnimationId) -> Option<f32> {
        self.animations.get(&id).map(|a| {
            a.sequence.get_lerp(self.current_time)
        })
    }

    /// 틱 (매 프레임 호출)
    pub fn tick(&mut self, delta_time: f32) {
        self.current_time += delta_time as f64;

        // 대기 중인 제거 처리
        for id in self.pending_removal.drain(..) {
            self.animations.remove(&id);
        }

        // 완료된 애니메이션 처리
        let mut completed = Vec::new();
        for (id, anim) in self.animations.iter() {
            if anim.state == AnimationState::Playing && anim.sequence.is_complete(self.current_time) {
                completed.push(*id);
            }
        }

        for id in completed {
            if let Some(mut anim) = self.animations.remove(&id) {
                anim.state = AnimationState::Completed;
                if let Some(callback) = anim.on_complete.take() {
                    callback();
                }
            }
        }
    }

    /// 모든 애니메이션 정지
    pub fn stop_all(&mut self) {
        self.animations.clear();
        self.pending_removal.clear();
    }

    /// 활성 애니메이션 수
    pub fn active_count(&self) -> usize {
        self.animations.values()
            .filter(|a| a.state == AnimationState::Playing)
            .count()
    }

    /// 총 등록된 애니메이션 수
    pub fn total_count(&self) -> usize {
        self.animations.len()
    }
}

// ============================================================================
// InterpTo 헬퍼
// ============================================================================

/// 부드러운 보간 (Unreal의 FMath::FInterpTo)
///
/// 현재 값에서 목표 값으로 일정한 속도로 보간합니다.
pub fn interp_to(current: f32, target: f32, delta_time: f32, interp_speed: f32) -> f32 {
    if interp_speed <= 0.0 {
        return target;
    }

    let distance = target - current;
    if distance.abs() < 0.0001 {
        return target;
    }

    let delta_move = distance * (delta_time * interp_speed).clamp(0.0, 1.0);
    current + delta_move
}

/// 부드러운 Vec2 보간
pub fn interp_to_vec2(current: Vec2, target: Vec2, delta_time: f32, interp_speed: f32) -> Vec2 {
    Vec2::new(
        interp_to(current.x, target.x, delta_time, interp_speed),
        interp_to(current.y, target.y, delta_time, interp_speed),
    )
}

/// 부드러운 색상 보간
pub fn interp_to_color(current: Color, target: Color, delta_time: f32, interp_speed: f32) -> Color {
    Color::rgba(
        interp_to(current.r, target.r, delta_time, interp_speed),
        interp_to(current.g, target.g, delta_time, interp_speed),
        interp_to(current.b, target.b, delta_time, interp_speed),
        interp_to(current.a, target.a, delta_time, interp_speed),
    )
}

/// 지수 감쇠 보간 (FMath::FInterpConstantTo 대안)
pub fn exp_decay(current: f32, target: f32, decay: f32, delta_time: f32) -> f32 {
    target + (current - target) * (-decay * delta_time).exp()
}

/// 지수 감쇠 Vec2 보간
pub fn exp_decay_vec2(current: Vec2, target: Vec2, decay: f32, delta_time: f32) -> Vec2 {
    Vec2::new(
        exp_decay(current.x, target.x, decay, delta_time),
        exp_decay(current.y, target.y, decay, delta_time),
    )
}

// ============================================================================
// P1#12: AnimationContext — 애니메이션 자동 등록/평가 컨텍스트
// ============================================================================

thread_local! {
    /// 현재 프레임의 시간 (prepass 전에 설정, 애니메이션 바인딩에서 읽음)
    static ANIMATION_CONTEXT_TIME: RefCell<f64> = RefCell::new(0.0);
}

/// 애니메이션 컨텍스트 시간 설정 (prepass 시작 전 호출)
pub fn set_animation_context_time(time: f64) {
    ANIMATION_CONTEXT_TIME.with(|t| *t.borrow_mut() = time);
}

/// 애니메이션 컨텍스트 시간 읽기
pub fn get_animation_context_time() -> f64 {
    ANIMATION_CONTEXT_TIME.with(|t| *t.borrow())
}

// ============================================================================
// P1#8: AnimationBinding — CurveSequence → Attribute<T> 자동 바인딩
// ============================================================================

/// 공유 CurveSequence (스레드 안전 Arc 래핑)
pub type SharedCurveSequence = Arc<std::sync::RwLock<CurveSequence>>;

/// 공유 CurveSequence 생성 헬퍼
pub fn shared_sequence(seq: CurveSequence) -> SharedCurveSequence {
    Arc::new(std::sync::RwLock::new(seq))
}

/// 애니메이션 바인딩 — CurveSequence 출력을 Attribute<T>로 변환
///
/// CurveSequence의 보간 값을 자동으로 위젯 속성에 바인딩합니다.
/// thread-local `ANIMATION_CONTEXT_TIME`을 통해 현재 시간을 읽습니다.
///
/// # 사용 예시
/// ```rust,ignore
/// let seq = shared_sequence(CurveSequence::new());
/// // ...
/// let binding = AnimationBinding::new(seq.clone());
/// widget.opacity_attr(binding.to_lerp_attribute());
/// ```
pub struct AnimationBinding {
    sequence: SharedCurveSequence,
}

impl AnimationBinding {
    /// 새 애니메이션 바인딩
    pub fn new(sequence: SharedCurveSequence) -> Self {
        Self { sequence }
    }

    /// 보간 값 (0.0~1.0) Attribute 생성
    pub fn to_lerp_attribute(&self) -> Attribute<f32> {
        let seq = self.sequence.clone();
        Attribute::bind(move || {
            let time = get_animation_context_time();
            seq.read().map(|s| s.get_lerp(time)).unwrap_or(0.0)
        })
    }

    /// 특정 커브의 값 Attribute 생성
    pub fn to_curve_attribute(&self, curve_index: usize) -> Attribute<f32> {
        let seq = self.sequence.clone();
        Attribute::bind(move || {
            let time = get_animation_context_time();
            seq.read().map(|s| s.get_curve_value(curve_index, time)).unwrap_or(0.0)
        })
    }

    /// 변환 함수를 적용한 Attribute 생성 (예: lerp → Color, lerp → Vec2)
    pub fn to_mapped_attribute<T, F>(&self, f: F) -> Attribute<T>
    where
        T: Clone + Send + Sync + 'static,
        F: Fn(f32) -> T + Send + Sync + 'static,
    {
        let seq = self.sequence.clone();
        Attribute::bind(move || {
            let time = get_animation_context_time();
            let lerp = seq.read().map(|s| s.get_lerp(time)).unwrap_or(0.0);
            f(lerp)
        })
    }

    /// 커브 값에 변환 적용한 Attribute
    pub fn to_curve_mapped_attribute<T, F>(&self, curve_index: usize, f: F) -> Attribute<T>
    where
        T: Clone + Send + Sync + 'static,
        F: Fn(f32) -> T + Send + Sync + 'static,
    {
        let seq = self.sequence.clone();
        Attribute::bind(move || {
            let time = get_animation_context_time();
            let val = seq.read().map(|s| s.get_curve_value(curve_index, time)).unwrap_or(0.0);
            f(val)
        })
    }
}

// ============================================================================
// P1#12: AutoAnimatedSequence — CurveSequence + 자동 타이머 등록
// ============================================================================

/// 자동 타이머 관리 CurveSequence
///
/// `play()`/`play_reverse()` 호출 시 자동으로 ActiveTimer를 등록하고,
/// 애니메이션 완료 시 자동으로 타이머를 해제합니다.
///
/// # 사용 예시
/// ```rust,ignore
/// struct MyWidget {
///     animation: AutoAnimatedSequence,
///     active_timers: ActiveTimers,
/// }
///
/// // play 시 자동 타이머 등록
/// self.animation.play(current_time, &mut self.active_timers);
///
/// // tick_active_timers에서 자동 완료 감지
/// self.animation.tick(current_time, &mut self.active_timers);
/// ```
#[derive(Debug, Clone)]
pub struct AutoAnimatedSequence {
    /// 내부 CurveSequence
    sequence: CurveSequence,
    /// 등록된 타이머 ID
    timer_id: Option<u64>,
}

impl AutoAnimatedSequence {
    /// 새 자동 관리 시퀀스
    pub fn new() -> Self {
        Self {
            sequence: CurveSequence::new(),
            timer_id: None,
        }
    }

    /// CurveSequence에서 생성
    pub fn from_sequence(sequence: CurveSequence) -> Self {
        Self {
            sequence,
            timer_id: None,
        }
    }

    /// 커브 추가
    pub fn add_curve(&mut self, curve: AnimationCurve) -> usize {
        self.sequence.add_curve(curve)
    }

    /// 루프 설정
    pub fn set_looping(&mut self, looping: bool) {
        self.sequence.set_looping(looping);
    }

    /// 재생 시작 + 자동 타이머 등록
    pub fn play(&mut self, current_time: f64, timers: &mut ActiveTimers) {
        self.sequence.play(current_time);
        self.ensure_timer_registered(timers);
    }

    /// 역재생 시작 + 자동 타이머 등록
    pub fn play_reverse(&mut self, current_time: f64, timers: &mut ActiveTimers) {
        self.sequence.play_reverse(current_time);
        self.ensure_timer_registered(timers);
    }

    /// 일시정지 (타이머 유지)
    pub fn pause(&mut self) {
        self.sequence.pause();
    }

    /// 재개 (타이머 유지)
    pub fn resume(&mut self) {
        self.sequence.resume();
    }

    /// 정지 + 타이머 해제
    pub fn stop(&mut self, timers: &mut ActiveTimers) {
        self.sequence.stop();
        self.unregister_timer(timers);
    }

    /// 즉시 끝으로 + 타이머 해제
    pub fn jump_to_end(&mut self, timers: &mut ActiveTimers) {
        self.sequence.jump_to_end();
        self.unregister_timer(timers);
    }

    /// 틱 — 완료 시 자동 타이머 해제, 완료 여부 반환
    pub fn tick(&mut self, current_time: f64, timers: &mut ActiveTimers) -> bool {
        if self.sequence.is_playing() && self.sequence.is_complete(current_time) {
            self.sequence.pause();
            self.unregister_timer(timers);
            return true; // 완료됨
        }
        false
    }

    /// 내부 CurveSequence 참조
    pub fn sequence(&self) -> &CurveSequence {
        &self.sequence
    }

    /// 내부 CurveSequence 가변 참조
    pub fn sequence_mut(&mut self) -> &mut CurveSequence {
        &mut self.sequence
    }

    /// 재생 중인지
    pub fn is_playing(&self) -> bool {
        self.sequence.is_playing()
    }

    /// 타이머 등록 여부
    pub fn has_timer(&self) -> bool {
        self.timer_id.is_some()
    }

    /// 커브 값 조회
    pub fn get_curve_value(&self, curve_index: usize, current_time: f64) -> f32 {
        self.sequence.get_curve_value(curve_index, current_time)
    }

    /// 보간 값 (0.0~1.0) 조회
    pub fn get_lerp(&self, current_time: f64) -> f32 {
        self.sequence.get_lerp(current_time)
    }

    /// 전체 지속 시간
    pub fn total_duration(&self) -> f32 {
        self.sequence.total_duration()
    }

    fn ensure_timer_registered(&mut self, timers: &mut ActiveTimers) {
        if self.timer_id.is_none() {
            self.timer_id = Some(timers.register(0.0)); // 매 프레임
        }
    }

    fn unregister_timer(&mut self, timers: &mut ActiveTimers) {
        if let Some(id) = self.timer_id.take() {
            timers.unregister(id);
        }
    }
}

impl Default for AutoAnimatedSequence {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// AnimatedAttributeManager (UE5 FAnimatedAttributeManager 대응)
// ============================================================================

/// 중앙 애니메이션 매니저
///
/// 등록된 Interpolator를 매 프레임 일괄 tick하고, 완료된 것을 자동 제거합니다.
/// UE5의 FAnimatedAttributeManager에 해당합니다.
pub struct AnimatedAttributeManager {
    animators: Vec<Box<dyn Interpolator>>,
}

impl AnimatedAttributeManager {
    pub fn new() -> Self {
        Self { animators: Vec::new() }
    }

    /// 애니메이터 등록
    pub fn register(&mut self, anim: Box<dyn Interpolator>) {
        self.animators.push(anim);
    }

    /// 매 프레임 호출 — 완료된 애니메이터 자동 제거
    pub fn tick(&mut self, delta_time: f32) {
        self.animators.retain_mut(|anim| !anim.tick_interpolator(delta_time));
    }

    /// 활성 애니메이터 수
    pub fn active_count(&self) -> usize {
        self.animators.len()
    }

    /// 비어있는지
    pub fn is_empty(&self) -> bool {
        self.animators.is_empty()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_easing_functions() {
        // 모든 이징 함수가 0에서 0, 1에서 1을 반환하는지 확인
        let functions = [
            EasingFunction::Linear,
            EasingFunction::EaseIn,
            EasingFunction::EaseOut,
            EasingFunction::EaseInOut,
            EasingFunction::QuadIn,
            EasingFunction::QuadOut,
            EasingFunction::CubicIn,
            EasingFunction::CubicOut,
        ];

        for func in functions {
            assert!((func.ease(0.0) - 0.0).abs() < 0.001, "{:?} at 0", func);
            assert!((func.ease(1.0) - 1.0).abs() < 0.001, "{:?} at 1", func);
        }
    }

    #[test]
    fn test_simple_animation() {
        let mut anim = SimpleAnimation::new(0.0);
        anim.animate_to(100.0, 1.0);

        assert!(anim.is_playing());

        // 절반 시간
        anim.tick(0.5);
        assert!(anim.value() > 40.0 && anim.value() < 60.0);

        // 완료
        anim.tick(0.6);
        assert!(!anim.is_playing());
        assert!((anim.value() - 100.0).abs() < 0.001);
    }

    #[test]
    fn test_curve_sequence() {
        let mut seq = CurveSequence::new();
        seq.add_curve(AnimationCurve::new(1.0).from_to(0.0, 100.0));

        seq.play(0.0);

        // 절반
        let value = seq.get_curve_value(0, 0.5);
        assert!((value - 50.0).abs() < 1.0);

        // 완료
        let value = seq.get_curve_value(0, 1.5);
        assert!((value - 100.0).abs() < 0.001);
    }

    #[test]
    fn test_arrive_interpolator() {
        let mut arrive = ArriveInterpolator::new(0.0)
            .with_max_speed(100.0)
            .with_arrival_distance(50.0);

        arrive.set_target(100.0);
        assert!(arrive.is_playing());

        // 여러 틱 후 목표에 도달해야 함
        // arrival_distance=50 (전체 거리의 50%)이므로 감속 구간이 길어
        // 수렴에 충분한 시간이 필요함 (~3초, 200틱@60fps)
        for _ in 0..200 {
            arrive.tick(0.016); // ~60fps
        }

        assert!(arrive.has_arrived());
        assert!((arrive.value() - 100.0).abs() < 1.0);
    }

    #[test]
    fn test_verlet_interpolator() {
        let mut verlet = VerletInterpolator::new(0.0)
            .with_stiffness(200.0)
            .with_damping(0.9);

        verlet.set_target(100.0);
        assert!(verlet.is_playing());

        // 여러 틱 후 안정화되어야 함
        for _ in 0..200 {
            verlet.tick(0.016);
        }

        assert!(verlet.is_settled());
        assert!((verlet.value() - 100.0).abs() < 1.0);
    }

    #[test]
    fn test_verlet_presets() {
        let soft = VerletInterpolator::soft();
        assert_eq!(soft.stiffness, 100.0);

        let stiff = VerletInterpolator::stiff();
        assert_eq!(stiff.stiffness, 300.0);

        let bouncy = VerletInterpolator::bouncy();
        assert_eq!(bouncy.damping, 0.6);
    }

    #[test]
    fn test_animation_manager() {
        let mut manager = AnimationManager::new();

        let mut seq = CurveSequence::new();
        seq.add_curve(AnimationCurve::new(0.5).from_to(0.0, 1.0));

        let id = manager.register(seq);
        assert_eq!(manager.get_state(id), Some(AnimationState::Pending));

        manager.play(id);
        assert_eq!(manager.get_state(id), Some(AnimationState::Playing));
        assert_eq!(manager.active_count(), 1);

        // 완료까지 틱
        for _ in 0..50 {
            manager.tick(0.016);
        }

        // 완료 후 제거됨
        assert_eq!(manager.get_state(id), None);
        assert_eq!(manager.active_count(), 0);
    }

    #[test]
    fn test_interp_to() {
        let result = interp_to(0.0, 100.0, 0.1, 5.0);
        assert!(result > 0.0 && result < 100.0);

        // 속도 0이면 즉시 목표 도달
        let result = interp_to(0.0, 100.0, 0.1, 0.0);
        assert!((result - 100.0).abs() < 0.001);
    }

    #[test]
    fn test_exp_decay() {
        let result = exp_decay(100.0, 0.0, 5.0, 0.1);
        assert!(result < 100.0 && result > 0.0);

        // decay가 클수록 빠르게 감쇠
        let slow = exp_decay(100.0, 0.0, 1.0, 0.1);
        let fast = exp_decay(100.0, 0.0, 10.0, 0.1);
        assert!(fast < slow);
    }

    #[test]
    fn test_animation_context_time() {
        set_animation_context_time(1.5);
        assert!((get_animation_context_time() - 1.5).abs() < f64::EPSILON);

        set_animation_context_time(3.0);
        assert!((get_animation_context_time() - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_animation_binding_lerp() {
        let mut seq = CurveSequence::new();
        seq.add_curve(AnimationCurve::new(1.0).from_to(0.0, 100.0));
        seq.play(0.0);

        let shared = shared_sequence(seq);
        let binding = AnimationBinding::new(shared.clone());
        let attr = binding.to_lerp_attribute();

        // 시간 0.5 설정 → 보간값 ~0.5
        set_animation_context_time(0.5);
        let val = attr.get();
        assert!(val > 0.4 && val < 0.6, "lerp at 0.5s = {}", val);

        // 시간 1.5 설정 → 보간값 1.0 (완료)
        set_animation_context_time(1.5);
        let val = attr.get();
        assert!((val - 1.0).abs() < 0.01, "lerp at 1.5s = {}", val);
    }

    #[test]
    fn test_animation_binding_curve() {
        let mut seq = CurveSequence::new();
        seq.add_curve(AnimationCurve::new(1.0).from_to(10.0, 50.0));
        seq.play(0.0);

        let shared = shared_sequence(seq);
        let binding = AnimationBinding::new(shared);
        let attr = binding.to_curve_attribute(0);

        set_animation_context_time(0.5);
        let val = attr.get();
        assert!(val > 25.0 && val < 35.0, "curve at 0.5s = {}", val);
    }

    #[test]
    fn test_animation_binding_mapped() {
        let mut seq = CurveSequence::new();
        seq.add_curve(AnimationCurve::new(1.0).from_to(0.0, 1.0));
        seq.play(0.0);

        let shared = shared_sequence(seq);
        let binding = AnimationBinding::new(shared);
        // lerp → Color (투명 → 불투명)
        let attr = binding.to_mapped_attribute(|lerp| {
            Color::rgba(1.0, 1.0, 1.0, lerp)
        });

        set_animation_context_time(0.0);
        let c = attr.get();
        assert!(c.a < 0.01);

        set_animation_context_time(1.5);
        let c = attr.get();
        assert!((c.a - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_auto_animated_sequence_play_stop() {
        let mut timers = ActiveTimers::new();
        let mut anim = AutoAnimatedSequence::new();
        anim.add_curve(AnimationCurve::new(0.5).from_to(0.0, 1.0));

        assert!(!anim.has_timer());

        // play → 타이머 자동 등록
        anim.play(0.0, &mut timers);
        assert!(anim.is_playing());
        assert!(anim.has_timer());

        // stop → 타이머 자동 해제
        anim.stop(&mut timers);
        assert!(!anim.is_playing());
        assert!(!anim.has_timer());
    }

    #[test]
    fn test_auto_animated_sequence_auto_complete() {
        let mut timers = ActiveTimers::new();
        let mut anim = AutoAnimatedSequence::new();
        anim.add_curve(AnimationCurve::new(0.5).from_to(0.0, 1.0));

        anim.play(0.0, &mut timers);
        assert!(anim.has_timer());

        // 완료 전 tick
        let done = anim.tick(0.3, &mut timers);
        assert!(!done);
        assert!(anim.has_timer());

        // 완료 후 tick → 자동 해제
        let done = anim.tick(0.6, &mut timers);
        assert!(done);
        assert!(!anim.has_timer());
        assert!(!anim.is_playing());
    }

    #[test]
    fn test_auto_animated_sequence_reverse() {
        let mut timers = ActiveTimers::new();
        let mut anim = AutoAnimatedSequence::new();
        anim.add_curve(AnimationCurve::new(1.0).from_to(0.0, 100.0));

        anim.play_reverse(0.0, &mut timers);
        assert!(anim.is_playing());
        assert!(anim.has_timer());

        let val = anim.get_lerp(0.5);
        assert!(val > 0.4 && val < 0.6);
    }
}
