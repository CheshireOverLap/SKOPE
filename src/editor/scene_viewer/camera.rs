//! 에디터 카메라 (Unreal 스타일 + 스무딩)
//!
//! ## 조작법
//! - 우클릭 + 마우스: Look around (FPS 스타일)
//! - 우클릭 + WASD: Fly-through 이동
//! - 중클릭 드래그: Pan (평행 이동)
//! - Alt + 좌클릭: Orbit (피벗 중심 회전)
//! - 스크롤 / Alt + 우클릭: Dolly/Zoom
//! - F: 선택된 오브젝트에 포커스
//!
//! ## 스무딩 시스템
//! Input → Target → Current (간접 구조)
//! - 지수 감쇠 (exponential decay) 기반 보간
//! - 프레임 독립적 처리

use glam::{Mat4, Vec2, Vec3};

// ============================================================
// 카메라 모드
// ============================================================

/// 카메라 조작 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CameraMode {
    #[default]
    Idle,
    /// Look around (우클릭 + 마우스)
    Looking,
    /// Fly-through (우클릭 + WASD)
    Flying,
    /// Pan (중클릭 드래그)
    Panning,
    /// Orbit (Alt + 좌클릭)
    Orbiting,
    /// Dolly/Zoom (Alt + 우클릭 드래그)
    Dollying,
}

// ============================================================
// 입력 이벤트
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    W, A, S, D,
    E, Q, R,
    Space, LShift,
    F,
    Other,
}

/// 통합 입력 이벤트
#[derive(Debug, Clone)]
pub enum CameraInput {
    MouseDown { button: MouseButton, pos: Vec2, alt_held: bool },
    MouseUp { button: MouseButton },
    MouseMove { pos: Vec2, delta: Vec2 },
    Scroll { delta: f32 },
    KeyDown { key: Key },
    KeyUp { key: Key },
}

// ============================================================
// 카메라 설정
// ============================================================

/// 카메라 설정 (감도, 스무딩 등)
#[derive(Debug, Clone)]
pub struct CameraSettings {
    // === 속도 ===
    /// 기본 이동 속도 (m/s)
    pub fly_speed: f32,
    /// Shift 부스트 배율
    pub boost_multiplier: f32,
    /// 마우스 감도 (Look/Orbit)
    pub look_sensitivity: f32,
    /// Pan 감도
    pub pan_sensitivity: f32,
    /// Dolly/Zoom 감도
    pub zoom_sensitivity: f32,

    // === 스무딩 (높을수록 빠르게 수렴) ===
    /// 위치 스무딩 (8~15)
    pub position_smoothing: f32,
    /// 회전 스무딩 (15~25)
    pub rotation_smoothing: f32,
    /// 가감속 스무딩 (5~10)
    pub velocity_smoothing: f32,

    // === 투영 ===
    pub fov: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for CameraSettings {
    fn default() -> Self {
        Self {
            fly_speed: 5.0,
            boost_multiplier: 3.0,
            look_sensitivity: 0.003,
            pan_sensitivity: 0.01,
            zoom_sensitivity: 0.15,

            position_smoothing: 12.0,
            rotation_smoothing: 20.0,
            velocity_smoothing: 8.0,

            fov: 60.0_f32.to_radians(),
            near: 0.1,
            far: 1000.0,
        }
    }
}

// ============================================================
// 에디터 카메라
// ============================================================

/// Unreal 스타일 에디터 카메라 (스무딩 포함)
pub struct EditorCamera {
    // === 현재 상태 (렌더링에 사용, 스무딩 적용됨) ===
    /// 현재 위치
    pub position: Vec3,
    /// 현재 yaw (스무딩됨)
    current_yaw: f32,
    /// 현재 pitch (스무딩됨)
    current_pitch: f32,

    // === 목표 상태 (입력이 갱신) ===
    target_position: Vec3,
    target_yaw: f32,
    target_pitch: f32,

    // === 속도 (Fly-through용) ===
    velocity: Vec3,
    target_velocity: Vec3,

    // === Orbit 피벗 ===
    pivot: Vec3,
    pivot_distance: f32,

    // === 상태 ===
    pub mode: CameraMode,
    last_mouse_pos: Vec2,

    // === 키 입력 상태 ===
    keys_held: std::collections::HashSet<Key>,

    // === 설정 ===
    pub settings: CameraSettings,
}

impl Default for EditorCamera {
    fn default() -> Self {
        let initial_pos = Vec3::new(0.0, -10.0, 5.0);
        let initial_yaw = 0.0;
        // 위치 (0, -10, 5)에서 원점을 바라보려면 아래를 봐야 함
        // atan2(5, 10) ≈ 0.46 rad, 하지만 아래를 보는 것이므로 음수
        let initial_pitch = -0.3;

        Self {
            position: initial_pos,
            current_yaw: initial_yaw,
            current_pitch: initial_pitch,

            target_position: initial_pos,
            target_yaw: initial_yaw,
            target_pitch: initial_pitch,

            velocity: Vec3::ZERO,
            target_velocity: Vec3::ZERO,

            pivot: Vec3::ZERO,
            pivot_distance: 10.0,

            mode: CameraMode::Idle,
            last_mouse_pos: Vec2::ZERO,
            keys_held: std::collections::HashSet::new(),

            settings: CameraSettings::default(),
        }
    }
}

impl EditorCamera {
    pub fn new() -> Self {
        Self::default()
    }

    // ========================================
    // 스무딩 유틸리티
    // ========================================

    /// 지수 감쇠 기반 스무딩 (Vec3)
    #[inline]
    fn smooth_damp_vec3(current: Vec3, target: Vec3, smoothing: f32, dt: f32) -> Vec3 {
        let t = 1.0 - (-smoothing * dt).exp();
        current.lerp(target, t)
    }

    /// 지수 감쇠 기반 스무딩 (f32)
    #[inline]
    fn smooth_damp_f32(current: f32, target: f32, smoothing: f32, dt: f32) -> f32 {
        let t = 1.0 - (-smoothing * dt).exp();
        current + (target - current) * t
    }

    // ========================================
    // 업데이트 (매 프레임 호출)
    // ========================================

    /// 매 프레임 업데이트 - 스무딩 적용
    pub fn update(&mut self, dt: f32) {
        let s = &self.settings;

        // 1. 속도 스무딩 (가감속)
        self.velocity = Self::smooth_damp_vec3(
            self.velocity,
            self.target_velocity,
            s.velocity_smoothing,
            dt,
        );

        // 2. 속도를 위치에 적용
        self.target_position += self.velocity * dt;

        // 3. 위치 스무딩
        self.position = Self::smooth_damp_vec3(
            self.position,
            self.target_position,
            s.position_smoothing,
            dt,
        );

        // 4. 회전 스무딩 (yaw/pitch 각각)
        self.current_yaw = Self::smooth_damp_f32(
            self.current_yaw,
            self.target_yaw,
            s.rotation_smoothing,
            dt,
        );
        self.current_pitch = Self::smooth_damp_f32(
            self.current_pitch,
            self.target_pitch,
            s.rotation_smoothing,
            dt,
        );

        // 5. Fly 모드일 때 키 입력으로 속도 갱신
        if matches!(self.mode, CameraMode::Looking | CameraMode::Flying) {
            self.update_fly_velocity();
        }
    }

    /// Fly 모드 속도 업데이트
    fn update_fly_velocity(&mut self) {
        let mut move_dir = Vec3::ZERO;

        // 카메라 기준 방향 벡터
        let forward = self.forward();
        let right = self.right();
        let up = Vec3::Z;

        if self.keys_held.contains(&Key::W) {
            move_dir += forward;
        }
        if self.keys_held.contains(&Key::S) {
            move_dir -= forward;
        }
        if self.keys_held.contains(&Key::D) {
            move_dir += right;
        }
        if self.keys_held.contains(&Key::A) {
            move_dir -= right;
        }
        if self.keys_held.contains(&Key::E) || self.keys_held.contains(&Key::Space) {
            move_dir += up;
        }
        if self.keys_held.contains(&Key::Q) {
            move_dir -= up;
        }

        // 정규화 및 속도 적용
        if move_dir.length_squared() > 0.0001 {
            move_dir = move_dir.normalize();
            let speed = if self.keys_held.contains(&Key::LShift) {
                self.settings.fly_speed * self.settings.boost_multiplier
            } else {
                self.settings.fly_speed
            };
            self.target_velocity = move_dir * speed;
        } else {
            self.target_velocity = Vec3::ZERO;
        }
    }

    // ========================================
    // 방향 벡터 (Z-up 좌표계)
    // ========================================

    /// 전방 벡터 (카메라가 바라보는 방향)
    /// Z-up 좌표계에서 yaw=0, pitch=0이면 -Y 방향 (Blender 전방)
    pub fn forward(&self) -> Vec3 {
        let (sy, cy) = self.current_yaw.sin_cos();
        let (sp, cp) = self.current_pitch.sin_cos();

        // yaw=0: -Y 방향
        // yaw=PI/2: +X 방향 (오른쪽으로 90도 회전)
        // pitch > 0: 위를 봄 (+Z 성분 증가)
        Vec3::new(
            sy * cp,   // X
            -cy * cp,  // Y (yaw=0이면 -Y)
            sp,        // Z (pitch가 양수면 위)
        )
    }

    /// 우측 벡터
    pub fn right(&self) -> Vec3 {
        let (sy, cy) = self.current_yaw.sin_cos();
        // Z-up에서 오른쪽은 forward를 Z축 기준 90도 회전
        Vec3::new(cy, sy, 0.0)
    }

    /// 상단 벡터
    pub fn up(&self) -> Vec3 {
        self.right().cross(self.forward()).normalize()
    }

    // ========================================
    // 행렬 계산
    // ========================================

    /// View 행렬
    pub fn view_matrix(&self) -> Mat4 {
        let target = self.position + self.forward();
        Mat4::look_at_rh(self.position, target, Vec3::Z)
    }

    /// Projection 행렬
    pub fn projection_matrix(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh(self.settings.fov, aspect, self.settings.near, self.settings.far)
    }

    /// View-Projection 행렬
    pub fn view_projection_matrix(&self, aspect: f32) -> Mat4 {
        self.projection_matrix(aspect) * self.view_matrix()
    }

    // ========================================
    // 입력 처리
    // ========================================

    /// 입력 이벤트 처리 (단일 진입점)
    pub fn handle_input(&mut self, input: CameraInput) {
        match input {
            CameraInput::MouseDown { button, pos, alt_held } => {
                self.on_mouse_down(button, pos, alt_held);
            }
            CameraInput::MouseUp { button } => {
                self.on_mouse_up(button);
            }
            CameraInput::MouseMove { pos, delta } => {
                self.on_mouse_move(pos, delta);
            }
            CameraInput::Scroll { delta } => {
                self.on_scroll(delta);
            }
            CameraInput::KeyDown { key } => {
                self.on_key_down(key);
            }
            CameraInput::KeyUp { key } => {
                self.on_key_up(key);
            }
        }
    }

    fn on_mouse_down(&mut self, button: MouseButton, pos: Vec2, alt_held: bool) {
        self.last_mouse_pos = pos;

        let new_mode = match (button, alt_held) {
            (MouseButton::Right, false) => CameraMode::Looking,
            (MouseButton::Right, true) => CameraMode::Dollying,
            (MouseButton::Middle, _) => CameraMode::Panning,
            (MouseButton::Left, true) => {
                // Orbit 시작 - 피벗 설정
                self.pivot = self.position + self.forward() * self.pivot_distance;
                CameraMode::Orbiting
            }
            (MouseButton::Left, false) => return, // 좌클릭만은 카메라 조작 아님
        };

        if self.mode != new_mode {
            self.mode = new_mode;
        }
    }

    fn on_mouse_up(&mut self, button: MouseButton) {
        let should_reset = match button {
            MouseButton::Right => matches!(self.mode, CameraMode::Looking | CameraMode::Flying | CameraMode::Dollying),
            MouseButton::Middle => matches!(self.mode, CameraMode::Panning),
            MouseButton::Left => matches!(self.mode, CameraMode::Orbiting),
        };

        if should_reset {
            self.mode = CameraMode::Idle;
            self.target_velocity = Vec3::ZERO;
        }
    }

    fn on_mouse_move(&mut self, pos: Vec2, delta: Vec2) {
        self.last_mouse_pos = pos;

        match self.mode {
            CameraMode::Idle => {}
            CameraMode::Looking | CameraMode::Flying => self.do_look(delta),
            CameraMode::Panning => self.do_pan(delta),
            CameraMode::Orbiting => self.do_orbit(delta),
            CameraMode::Dollying => self.do_dolly_drag(delta),
        }
    }

    fn on_scroll(&mut self, delta: f32) {
        // 스크롤로 전방 이동 (Dolly)
        let forward = self.forward();
        let move_amount = delta * self.settings.zoom_sensitivity * self.pivot_distance * 0.5;
        self.target_position += forward * move_amount;
    }

    fn on_key_down(&mut self, key: Key) {
        self.keys_held.insert(key);

        // Looking 모드에서 키 입력 시 Flying으로 전환
        if self.mode == CameraMode::Looking {
            if matches!(key, Key::W | Key::A | Key::S | Key::D | Key::E | Key::Q | Key::Space) {
                self.mode = CameraMode::Flying;
            }
        }
    }

    fn on_key_up(&mut self, key: Key) {
        self.keys_held.remove(&key);
    }

    // ========================================
    // 카메라 조작 구현
    // ========================================

    /// Look around: 시선 방향 변경 (위치 고정)
    fn do_look(&mut self, delta: Vec2) {
        let sens = self.settings.look_sensitivity;

        // Yaw: 좌우 (X 델타)
        self.target_yaw -= delta.x * sens;

        // Pitch: 상하 (Y 델타) - 마우스 위로 = 위를 봄
        self.target_pitch += delta.y * sens;
        self.clamp_pitch();
    }

    /// Pan: 카메라를 평행 이동
    fn do_pan(&mut self, delta: Vec2) {
        let sens = self.settings.pan_sensitivity * self.pivot_distance * 0.1;
        let right = self.right();
        let up = self.up();

        // 마우스 방향과 반대로 이동 (자연스러운 드래그)
        self.target_position -= right * delta.x * sens;
        self.target_position += up * delta.y * sens;
    }

    /// Orbit: 피벗 중심 회전
    fn do_orbit(&mut self, delta: Vec2) {
        let sens = self.settings.look_sensitivity;

        // Yaw/Pitch 업데이트
        self.target_yaw -= delta.x * sens;
        self.target_pitch += delta.y * sens;
        self.clamp_pitch();

        // 피벗으로부터 새 위치 계산 (target_yaw/pitch에서 backward 직접 계산)
        let backward = self.calculate_backward(self.target_yaw, self.target_pitch);
        self.target_position = self.pivot + backward * self.pivot_distance;
    }

    /// Dolly (Alt + 우클릭 드래그): 전후 이동
    fn do_dolly_drag(&mut self, delta: Vec2) {
        let forward = self.forward();
        let move_amount = -delta.y * self.settings.zoom_sensitivity * 0.5;
        self.target_position += forward * move_amount;
    }

    /// Pitch 제한 (-89° ~ 89°)
    fn clamp_pitch(&mut self) {
        let limit = 89.0_f32.to_radians();
        self.target_pitch = self.target_pitch.clamp(-limit, limit);
    }

    // ========================================
    // 유틸리티
    // ========================================

    /// 오브젝트에 Focus (F 키)
    pub fn focus_on(&mut self, target: Vec3, size: f32) {
        self.pivot = target;
        self.pivot_distance = (size * 2.5).max(2.0);

        // 현재 방향 유지하면서 새 위치로 이동
        let backward = self.calculate_backward(self.target_yaw, self.target_pitch);
        self.target_position = self.pivot + backward * self.pivot_distance;
    }

    /// yaw/pitch에서 backward 벡터 계산 (forward의 반대)
    fn calculate_backward(&self, yaw: f32, pitch: f32) -> Vec3 {
        let (sy, cy) = yaw.sin_cos();
        let (sp, cp) = pitch.sin_cos();
        // forward의 반대
        Vec3::new(-sy * cp, cy * cp, -sp)
    }

    /// 초기화
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// 카메라가 활성 상태인지 (드래그 중)
    pub fn is_active(&self) -> bool {
        self.mode != CameraMode::Idle
    }

    /// 현재 타겟 위치 (편의용)
    pub fn target(&self) -> Vec3 {
        self.position + self.forward() * self.pivot_distance
    }

    /// Yaw 값 (디버그용)
    pub fn yaw(&self) -> f32 {
        self.target_yaw
    }

    /// Pitch 값 (디버그용)
    pub fn pitch(&self) -> f32 {
        self.target_pitch
    }

    // ========================================
    // 뷰 프리셋 (Z-up 좌표계)
    // ========================================

    /// Top 뷰 (위에서 아래로) - Numpad 7
    pub fn set_top_view(&mut self) {
        self.target_yaw = 0.0;
        self.target_pitch = std::f32::consts::FRAC_PI_2 - 0.01; // 거의 수직
    }

    /// Front 뷰 (정면, -Y에서 origin 봄) - Numpad 1
    pub fn set_front_view(&mut self) {
        self.target_yaw = 0.0;
        self.target_pitch = 0.0;
    }

    /// Right 뷰 (+X에서 origin 봄) - Numpad 3
    pub fn set_right_view(&mut self) {
        self.target_yaw = -std::f32::consts::FRAC_PI_2;
        self.target_pitch = 0.0;
    }

    /// Perspective 뷰 (기본 대각선) - Numpad 0
    pub fn set_perspective_view(&mut self) {
        self.target_yaw = std::f32::consts::FRAC_PI_4;
        self.target_pitch = 0.3;
    }

    // ========================================
    // Ray Casting
    // ========================================

    /// 스크린 좌표를 월드 Ray로 변환
    pub fn screen_to_ray(&self, screen_pos: Vec2, screen_size: Vec2) -> Ray {
        let ndc_x = (screen_pos.x / screen_size.x) * 2.0 - 1.0;
        let ndc_y = 1.0 - (screen_pos.y / screen_size.y) * 2.0;

        let aspect = screen_size.x / screen_size.y;
        let inv_proj = self.projection_matrix(aspect).inverse();
        let inv_view = self.view_matrix().inverse();

        let ray_clip = glam::Vec4::new(ndc_x, ndc_y, -1.0, 1.0);
        let ray_eye = inv_proj * ray_clip;
        let ray_eye = glam::Vec4::new(ray_eye.x, ray_eye.y, -1.0, 0.0);

        let ray_world = inv_view * ray_eye;
        let direction = Vec3::new(ray_world.x, ray_world.y, ray_world.z).normalize();

        Ray {
            origin: self.position,
            direction,
        }
    }
}

// ============================================================
// Ray
// ============================================================

/// Ray (광선)
#[derive(Debug, Clone, Copy)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray {
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self {
            origin,
            direction: direction.normalize(),
        }
    }

    /// Ray 위의 점
    pub fn at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }

    /// 평면과의 교차점
    pub fn intersect_plane(&self, plane_normal: Vec3, plane_point: Vec3) -> Option<f32> {
        let denom = plane_normal.dot(self.direction);
        if denom.abs() < 1e-6 {
            return None;
        }
        let t = (plane_point - self.origin).dot(plane_normal) / denom;
        if t >= 0.0 { Some(t) } else { None }
    }

    /// AABB와의 교차 테스트
    pub fn intersect_aabb(&self, min: Vec3, max: Vec3) -> Option<f32> {
        let inv_dir = Vec3::new(
            1.0 / self.direction.x,
            1.0 / self.direction.y,
            1.0 / self.direction.z,
        );

        let t1 = (min.x - self.origin.x) * inv_dir.x;
        let t2 = (max.x - self.origin.x) * inv_dir.x;
        let t3 = (min.y - self.origin.y) * inv_dir.y;
        let t4 = (max.y - self.origin.y) * inv_dir.y;
        let t5 = (min.z - self.origin.z) * inv_dir.z;
        let t6 = (max.z - self.origin.z) * inv_dir.z;

        let tmin = t1.min(t2).max(t3.min(t4)).max(t5.min(t6));
        let tmax = t1.max(t2).min(t3.max(t4)).min(t5.max(t6));

        if tmax < 0.0 || tmin > tmax {
            None
        } else {
            Some(if tmin < 0.0 { tmax } else { tmin })
        }
    }
}
