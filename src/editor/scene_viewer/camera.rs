//! 에디터 카메라 (Unity/Unreal 스타일)
//!
//! - 우클릭 드래그: Orbit (회전)
//! - 중클릭 드래그: Pan (이동)
//! - 스크롤: Zoom (거리 조절)
//! - 우클릭 + WASD: Fly 모드

use glam::{Mat4, Quat, Vec2, Vec3};

/// 에디터용 Orbit 카메라
pub struct EditorCamera {
    /// 바라보는 타겟 점
    pub target: Vec3,
    /// 타겟으로부터의 거리
    pub distance: f32,
    /// 수평 회전 (라디안)
    pub yaw: f32,
    /// 수직 회전 (라디안, -89° ~ 89°)
    pub pitch: f32,

    /// FOV (라디안)
    pub fov: f32,
    /// Near plane
    pub near: f32,
    /// Far plane
    pub far: f32,

    /// 우클릭 드래그 중
    pub is_orbiting: bool,
    /// 중클릭 드래그 중
    pub is_panning: bool,
    /// Fly 모드 (우클릭 + WASD)
    pub is_flying: bool,

    /// 마지막 마우스 위치
    last_mouse_pos: Vec2,

    /// Orbit 감도
    pub orbit_sensitivity: f32,
    /// Pan 감도
    pub pan_sensitivity: f32,
    /// Zoom 감도
    pub zoom_sensitivity: f32,
    /// Fly 속도
    pub fly_speed: f32,
}

impl Default for EditorCamera {
    fn default() -> Self {
        Self {
            target: Vec3::ZERO,
            distance: 10.0,
            yaw: 0.0,
            pitch: -0.5, // 약간 아래를 봄

            fov: 45.0_f32.to_radians(),
            near: 0.1,
            far: 1000.0,

            is_orbiting: false,
            is_panning: false,
            is_flying: false,

            last_mouse_pos: Vec2::ZERO,

            orbit_sensitivity: 0.005,
            pan_sensitivity: 0.01,
            zoom_sensitivity: 0.1,
            fly_speed: 10.0,
        }
    }
}

impl EditorCamera {
    pub fn new() -> Self {
        Self::default()
    }

    /// 카메라 위치 계산
    pub fn position(&self) -> Vec3 {
        let x = self.distance * self.yaw.cos() * self.pitch.cos();
        let y = self.distance * self.pitch.sin();
        let z = self.distance * self.yaw.sin() * self.pitch.cos();
        self.target + Vec3::new(x, y, z)
    }

    /// 전방 벡터
    pub fn forward(&self) -> Vec3 {
        (self.target - self.position()).normalize()
    }

    /// 우측 벡터
    pub fn right(&self) -> Vec3 {
        self.forward().cross(Vec3::Y).normalize()
    }

    /// 상단 벡터
    pub fn up(&self) -> Vec3 {
        self.right().cross(self.forward()).normalize()
    }

    /// View 행렬
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.position(), self.target, Vec3::Y)
    }

    /// Projection 행렬
    pub fn projection_matrix(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh(self.fov, aspect, self.near, self.far)
    }

    /// View-Projection 행렬
    pub fn view_projection_matrix(&self, aspect: f32) -> Mat4 {
        self.projection_matrix(aspect) * self.view_matrix()
    }

    /// 스크린 좌표를 월드 Ray로 변환
    pub fn screen_to_ray(&self, screen_pos: Vec2, screen_size: Vec2) -> Ray {
        // NDC 좌표로 변환 (-1 ~ 1)
        let ndc_x = (screen_pos.x / screen_size.x) * 2.0 - 1.0;
        let ndc_y = 1.0 - (screen_pos.y / screen_size.y) * 2.0;

        let aspect = screen_size.x / screen_size.y;
        let inv_proj = self.projection_matrix(aspect).inverse();
        let inv_view = self.view_matrix().inverse();

        // Near plane에서의 점
        let ray_clip = glam::Vec4::new(ndc_x, ndc_y, -1.0, 1.0);
        let ray_eye = inv_proj * ray_clip;
        let ray_eye = glam::Vec4::new(ray_eye.x, ray_eye.y, -1.0, 0.0);

        let ray_world = inv_view * ray_eye;
        let direction = Vec3::new(ray_world.x, ray_world.y, ray_world.z).normalize();

        Ray {
            origin: self.position(),
            direction,
        }
    }

    /// 마우스 버튼 눌림 처리
    pub fn on_mouse_button_down(&mut self, button: MouseButton, pos: Vec2) {
        self.last_mouse_pos = pos;
        match button {
            MouseButton::Right => {
                self.is_orbiting = true;
            }
            MouseButton::Middle => {
                self.is_panning = true;
            }
            _ => {}
        }
    }

    /// 마우스 버튼 떼기 처리
    pub fn on_mouse_button_up(&mut self, button: MouseButton) {
        match button {
            MouseButton::Right => {
                self.is_orbiting = false;
                self.is_flying = false;
            }
            MouseButton::Middle => {
                self.is_panning = false;
            }
            _ => {}
        }
    }

    /// 마우스 이동 처리
    pub fn on_mouse_move(&mut self, pos: Vec2) {
        let delta = pos - self.last_mouse_pos;
        self.last_mouse_pos = pos;

        if self.is_orbiting && !self.is_flying {
            // Orbit 모드
            self.yaw -= delta.x * self.orbit_sensitivity;
            self.pitch -= delta.y * self.orbit_sensitivity;

            // Pitch 제한 (-89° ~ 89°)
            let limit = 89.0_f32.to_radians();
            self.pitch = self.pitch.clamp(-limit, limit);
        } else if self.is_panning {
            // Pan 모드
            let right = self.right();
            let up = self.up();
            let pan_speed = self.pan_sensitivity * self.distance * 0.1;
            self.target -= right * delta.x * pan_speed;
            self.target += up * delta.y * pan_speed;
        }
    }

    /// 스크롤 처리 (Zoom)
    pub fn on_scroll(&mut self, delta: f32) {
        self.distance *= 1.0 - delta * self.zoom_sensitivity;
        self.distance = self.distance.clamp(0.1, 1000.0);
    }

    /// 키 입력 처리 (Fly 모드)
    pub fn on_key(&mut self, key: Key, pressed: bool, dt: f32) {
        if !self.is_orbiting {
            return;
        }

        self.is_flying = true;
        if !pressed {
            return;
        }

        let speed = self.fly_speed * dt;
        let forward = self.forward();
        let right = self.right();

        match key {
            Key::W => {
                self.target += forward * speed;
            }
            Key::S => {
                self.target -= forward * speed;
            }
            Key::A => {
                self.target -= right * speed;
            }
            Key::D => {
                self.target += right * speed;
            }
            Key::E | Key::Space => {
                self.target += Vec3::Y * speed;
            }
            Key::Q | Key::LShift => {
                self.target -= Vec3::Y * speed;
            }
            _ => {}
        }
    }

    /// 오브젝트에 Focus
    pub fn focus_on(&mut self, position: Vec3, size: f32) {
        self.target = position;
        // 오브젝트 크기에 맞게 거리 조절
        self.distance = size * 2.5;
    }

    /// 초기화
    pub fn reset(&mut self) {
        self.target = Vec3::ZERO;
        self.distance = 10.0;
        self.yaw = 0.0;
        self.pitch = -0.5;
    }
}

/// 마우스 버튼
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// 키보드 키
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    W,
    A,
    S,
    D,
    E,
    Q,
    R,
    Space,
    LShift,
    F,
    Other,
}

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
            return None; // 평행
        }
        let t = (plane_point - self.origin).dot(plane_normal) / denom;
        if t >= 0.0 {
            Some(t)
        } else {
            None
        }
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
