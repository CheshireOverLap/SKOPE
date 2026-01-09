//! Scene Viewer 모듈
//!
//! 에디터 카메라, 그리드, Gizmo를 통합하는 3D 뷰포트

pub mod camera;
pub mod grid;
pub mod orientation_gizmo;

pub use camera::{CameraInput, EditorCamera, Key, MouseButton, Ray};
pub use grid::GridRenderer;

use crate::editor::command::{Command, MoveCommand, RotateCommand, ScaleCommand};
use crate::editor::gizmo::{GizmoAxis, GizmoMode, MoveGizmo, RotateGizmo, ScaleGizmo};
use crate::editor::selection::{pick_entity, Selection, SelectionModifier};
use crate::ecs_components::Transform;
use bevy_ecs::prelude::Entity;
use bevy_ecs::world::World;
use glam::{Quat, Vec2, Vec3};

/// Scene Viewer 통합 구조체
pub struct SceneViewer {
    /// 에디터 카메라
    pub camera: EditorCamera,
    /// 그리드 렌더러
    pub grid: GridRenderer,
    /// Move Gizmo
    pub move_gizmo: MoveGizmo,
    /// Rotate Gizmo
    pub rotate_gizmo: RotateGizmo,
    /// Scale Gizmo
    pub scale_gizmo: ScaleGizmo,
    /// Gizmo 모드
    pub gizmo_mode: GizmoMode,
    /// 선택 상태
    pub selection: Selection,
    /// 화면 크기
    screen_size: (u32, u32),
    /// 마우스 위치
    last_mouse_pos: Vec2,
    /// Gizmo 드래그 중
    is_dragging_gizmo: bool,
    /// 클릭 시작 위치 (드래그 구분용)
    click_start_pos: Option<Vec2>,
    /// 드래그 시작 시 Gizmo 위치 (Command 생성용)
    drag_start_gizmo_pos: Vec3,
    /// 드래그 대상 엔티티들 (드래그 시작 시 캡처)
    drag_entities: Vec<Entity>,
    /// 드래그 시작 시 회전 (Rotate Gizmo용)
    drag_start_rotation: Quat,
    /// 드래그 시작 시 스케일 (Scale Gizmo용)
    drag_start_scale: Vec3,
    /// 누적 회전 각도 (드래그 중)
    accumulated_rotation: Quat,
    /// 누적 스케일 팩터 (드래그 중)
    accumulated_scale: Vec3,
    /// 로컬 좌표계 모드 (true=Local, false=World)
    pub local_space: bool,
    /// 그리드 스냅 활성화
    pub snap_enabled: bool,
    /// 이동 스냅 단위 (미터)
    pub snap_translate: f32,
    /// 회전 스냅 단위 (도)
    pub snap_rotate: f32,
    /// 스케일 스냅 단위
    pub snap_scale: f32,
}

impl SceneViewer {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        screen_size: (u32, u32),
    ) -> Self {
        Self {
            camera: EditorCamera::new(),
            grid: GridRenderer::new(device, format, depth_format),
            move_gizmo: MoveGizmo::new(device, format, depth_format),
            rotate_gizmo: RotateGizmo::new(device, format, depth_format),
            scale_gizmo: ScaleGizmo::new(device, format, depth_format),
            gizmo_mode: GizmoMode::Move,
            selection: Selection::new(),
            screen_size,
            last_mouse_pos: Vec2::ZERO,
            is_dragging_gizmo: false,
            click_start_pos: None,
            drag_start_gizmo_pos: Vec3::ZERO,
            drag_entities: Vec::new(),
            drag_start_rotation: Quat::IDENTITY,
            drag_start_scale: Vec3::ONE,
            accumulated_rotation: Quat::IDENTITY,
            accumulated_scale: Vec3::ONE,
            local_space: false,
            snap_enabled: false,
            snap_translate: 1.0,
            snap_rotate: 15.0,
            snap_scale: 0.1,
        }
    }

    /// 화면 크기 변경
    pub fn resize(&mut self, width: u32, height: u32) {
        self.screen_size = (width, height);
    }

    /// 좌표계 토글 (Local/World)
    pub fn toggle_space(&mut self) {
        self.local_space = !self.local_space;
        log::info!(
            "[Gizmo] Space: {}",
            if self.local_space { "Local" } else { "World" }
        );
    }

    /// 스냅 토글
    pub fn toggle_snap(&mut self) {
        self.snap_enabled = !self.snap_enabled;
        log::info!(
            "[Gizmo] Snap: {}",
            if self.snap_enabled { "ON" } else { "OFF" }
        );
    }

    /// 커서 캡처가 필요한지 확인 (카메라 조작 중)
    pub fn should_capture_cursor(&self) -> bool {
        self.camera.should_capture_cursor()
    }

    /// 값 스냅
    fn snap_value(value: f32, step: f32) -> f32 {
        (value / step).round() * step
    }

    /// 마우스 버튼 이벤트 (선택 피킹은 try_pick에서 처리)
    /// 드래그 종료 시 Command 반환 (Undo/Redo용)
    pub fn on_mouse_button(&mut self, button: MouseButton, pressed: bool, pos: Vec2, alt_held: bool) -> Option<Box<dyn Command>> {
        let mut result_command: Option<Box<dyn Command>> = None;

        // 마우스 버튼을 누를 때 위치 기록 (delta 계산용)
        if pressed {
            self.last_mouse_pos = pos;
        }

        // 왼쪽 버튼: Gizmo 드래그 또는 선택
        if button == MouseButton::Left {
            if pressed {
                self.click_start_pos = Some(pos);

                // Gizmo 피킹 (현재 모드에 따라)
                match self.gizmo_mode {
                    GizmoMode::Move => {
                        if self.move_gizmo.hovered_axis != GizmoAxis::None {
                            self.move_gizmo.begin_drag(self.move_gizmo.hovered_axis);
                            self.is_dragging_gizmo = true;
                            self.drag_start_gizmo_pos = self.move_gizmo.position;
                            self.drag_entities = self.selection.entities.clone();
                        }
                    }
                    GizmoMode::Rotate => {
                        if self.rotate_gizmo.hovered_axis != GizmoAxis::None {
                            self.rotate_gizmo.begin_drag(self.rotate_gizmo.hovered_axis);
                            self.is_dragging_gizmo = true;
                            self.drag_start_gizmo_pos = self.rotate_gizmo.position;
                            self.drag_entities = self.selection.entities.clone();
                            self.accumulated_rotation = Quat::IDENTITY;
                        }
                    }
                    GizmoMode::Scale => {
                        if self.scale_gizmo.hovered_axis != GizmoAxis::None {
                            self.scale_gizmo.begin_drag(self.scale_gizmo.hovered_axis, Vec3::ONE);
                            self.is_dragging_gizmo = true;
                            self.drag_start_gizmo_pos = self.scale_gizmo.position;
                            self.drag_entities = self.selection.entities.clone();
                            self.accumulated_scale = Vec3::ONE;
                        }
                    }
                    GizmoMode::Select => {}
                }
            } else {
                if self.is_dragging_gizmo {
                    // 드래그 종료 - 적절한 Command 생성
                    match self.gizmo_mode {
                        GizmoMode::Move => {
                            let delta = self.move_gizmo.position - self.drag_start_gizmo_pos;
                            if delta.length() > 0.001 && !self.drag_entities.is_empty() {
                                result_command = Some(Box::new(MoveCommand::new(
                                    self.drag_entities.clone(),
                                    delta,
                                )));
                            }
                            self.move_gizmo.end_drag();
                        }
                        GizmoMode::Rotate => {
                            if self.accumulated_rotation != Quat::IDENTITY && !self.drag_entities.is_empty() {
                                result_command = Some(Box::new(RotateCommand::new(
                                    self.drag_entities.clone(),
                                    self.accumulated_rotation,
                                    self.drag_start_gizmo_pos,
                                )));
                            }
                            self.rotate_gizmo.end_drag();
                            self.accumulated_rotation = Quat::IDENTITY;
                        }
                        GizmoMode::Scale => {
                            if self.accumulated_scale != Vec3::ONE && !self.drag_entities.is_empty() {
                                result_command = Some(Box::new(ScaleCommand::new(
                                    self.drag_entities.clone(),
                                    self.accumulated_scale,
                                    self.drag_start_gizmo_pos,
                                )));
                            }
                            self.scale_gizmo.end_drag();
                            self.accumulated_scale = Vec3::ONE;
                        }
                        GizmoMode::Select => {}
                    }

                    self.is_dragging_gizmo = false;
                    self.drag_entities.clear();
                }
                self.click_start_pos = None;
            }
        }

        // 카메라 컨트롤 (우클릭/중클릭)
        if !self.is_dragging_gizmo {
            let input = if pressed {
                CameraInput::MouseDown { button, pos, alt_held }
            } else {
                CameraInput::MouseUp { button }
            };
            self.camera.handle_input(input);
        }

        result_command
    }

    /// 현재 활성 기즈모의 호버 상태 확인
    fn get_current_hovered_axis(&self) -> GizmoAxis {
        match self.gizmo_mode {
            GizmoMode::Move => self.move_gizmo.hovered_axis,
            GizmoMode::Rotate => self.rotate_gizmo.hovered_axis,
            GizmoMode::Scale => self.scale_gizmo.hovered_axis,
            GizmoMode::Select => GizmoAxis::None,
        }
    }

    /// 마우스 클릭으로 오브젝트 선택 (World 접근 필요)
    /// 드래그가 아닌 클릭일 때만 호출
    pub fn try_pick(&mut self, world: &mut World, pos: Vec2, modifier: SelectionModifier) -> bool {
        // 드래그 중이면 선택하지 않음
        if self.is_dragging_gizmo {
            return false;
        }

        // Gizmo 위에 있으면 선택하지 않음
        if self.get_current_hovered_axis() != GizmoAxis::None {
            return false;
        }

        // 클릭 시작 위치와 현재 위치 비교 (드래그 구분)
        if let Some(start) = self.click_start_pos {
            let delta = (pos - start).length();
            if delta > 5.0 {
                // 드래그로 간주
                return false;
            }
        }

        // Ray 생성 및 피킹
        let screen_size = Vec2::new(self.screen_size.0 as f32, self.screen_size.1 as f32);
        let ray = self.camera.screen_to_ray(pos, screen_size);

        let picked = pick_entity(world, &ray, &mut self.selection, modifier);

        // 선택된 엔티티의 중심으로 모든 Gizmo 이동
        if let Some(center) = self.selection.center(world) {
            self.update_all_gizmo_positions(center);
        }

        picked
    }

    /// 모든 기즈모 위치 업데이트
    fn update_all_gizmo_positions(&mut self, position: Vec3) {
        self.move_gizmo.set_position(position);
        self.rotate_gizmo.set_position(position);
        self.scale_gizmo.set_position(position);
    }

    /// 선택 상태에 따라 Gizmo 위치 업데이트
    pub fn update_gizmo_from_selection(&mut self, world: &World) {
        if let Some(center) = self.selection.center(world) {
            self.update_all_gizmo_positions(center);
        }
    }

    /// 선택된 엔티티에 카메라 포커스 (F키)
    pub fn focus_on_selection(&mut self, world: &World) {
        if self.selection.entities.is_empty() {
            return;
        }

        // 선택된 엔티티들의 중심점 계산
        if let Some(center) = self.selection.center(world) {
            // 선택 영역 크기 계산
            let mut max_distance = 1.0f32;
            for &entity in &self.selection.entities {
                if let Some(transform) = world.get::<Transform>(entity) {
                    let dist = (transform.translation - center).length();
                    max_distance = max_distance.max(dist);
                }
            }

            // 최소 거리 보장
            let size = (max_distance * 2.0).max(1.0);

            // 카메라 포커스
            self.camera.focus_on(center, size);

            log::info!("[Camera] Focused on selection: {:?}", center);
        }
    }

    /// 마우스 이동 이벤트 (World 접근으로 실제 Transform 업데이트)
    pub fn on_mouse_move(&mut self, pos: Vec2, world: &mut World) {
        let prev_pos = self.last_mouse_pos;
        self.last_mouse_pos = pos;

        let screen_size = Vec2::new(self.screen_size.0 as f32, self.screen_size.1 as f32);

        // Gizmo 드래그 중 - 선택된 엔티티들의 Transform 업데이트
        if self.is_dragging_gizmo {
            match self.gizmo_mode {
                GizmoMode::Move => {
                    let offset = self.move_gizmo.calculate_drag_offset(&self.camera, prev_pos, pos, screen_size);
                    self.apply_move_transform(world, offset);
                }
                GizmoMode::Rotate => {
                    let angle = self.rotate_gizmo.calculate_rotation_angle(&self.camera, prev_pos, pos, screen_size);
                    let rotation = self.rotate_gizmo.get_drag_rotation(angle);
                    self.apply_rotate_transform(world, rotation);
                    self.accumulated_rotation = rotation * self.accumulated_rotation;
                }
                GizmoMode::Scale => {
                    let scale_factor = self.scale_gizmo.calculate_scale_factor(&self.camera, prev_pos, pos, screen_size);
                    self.apply_scale_transform(world, scale_factor);
                    self.accumulated_scale *= scale_factor;
                }
                GizmoMode::Select => {}
            }
            return;
        }

        // Gizmo 호버 검사 (현재 모드에 따라)
        let ray = self.camera.screen_to_ray(pos, screen_size);
        match self.gizmo_mode {
            GizmoMode::Move => {
                self.move_gizmo.hovered_axis = self.move_gizmo.pick(&ray);
            }
            GizmoMode::Rotate => {
                self.rotate_gizmo.hovered_axis = self.rotate_gizmo.pick(&ray);
            }
            GizmoMode::Scale => {
                self.scale_gizmo.hovered_axis = self.scale_gizmo.pick(&ray);
            }
            GizmoMode::Select => {}
        }

        // 카메라 컨트롤 (delta 계산)
        let delta = pos - prev_pos;
        self.camera.handle_input(CameraInput::MouseMove { pos, delta });
    }

    /// Move Gizmo 드래그로 선택된 엔티티들의 Transform 이동
    fn apply_move_transform(&mut self, world: &mut World, offset: Vec3) {
        // 스냅 적용
        let offset = if self.snap_enabled {
            Vec3::new(
                Self::snap_value(offset.x, self.snap_translate),
                Self::snap_value(offset.y, self.snap_translate),
                Self::snap_value(offset.z, self.snap_translate),
            )
        } else {
            offset
        };

        if offset.length() < 0.0001 {
            return;
        }

        for &entity in &self.selection.entities {
            if let Some(mut transform) = world.get_mut::<Transform>(entity) {
                transform.translation += offset;
            }
        }

        // 모든 Gizmo 위치도 업데이트
        if let Some(center) = self.selection.center(world) {
            self.update_all_gizmo_positions(center);
        }
    }

    /// Rotate Gizmo 드래그로 선택된 엔티티들의 Transform 회전
    fn apply_rotate_transform(&mut self, world: &mut World, rotation: Quat) {
        if rotation == Quat::IDENTITY {
            return;
        }

        let pivot = self.rotate_gizmo.position;

        for &entity in &self.selection.entities {
            if let Some(mut transform) = world.get_mut::<Transform>(entity) {
                // 피벗 중심 회전
                let offset = transform.translation - pivot;
                let rotated_offset = rotation * offset;
                transform.translation = pivot + rotated_offset;

                // 오브젝트 자체 회전
                transform.rotation = rotation * transform.rotation;
            }
        }
    }

    /// Scale Gizmo 드래그로 선택된 엔티티들의 Transform 스케일
    fn apply_scale_transform(&mut self, world: &mut World, scale_factor: Vec3) {
        if scale_factor == Vec3::ONE {
            return;
        }

        let pivot = self.scale_gizmo.position;

        for &entity in &self.selection.entities {
            if let Some(mut transform) = world.get_mut::<Transform>(entity) {
                // 피벗 중심 스케일
                let offset = transform.translation - pivot;
                let scaled_offset = offset * scale_factor;
                transform.translation = pivot + scaled_offset;

                // 오브젝트 스케일 변경
                transform.scale *= scale_factor;
            }
        }
    }

    /// 스크롤 이벤트
    pub fn on_scroll(&mut self, delta: f32) {
        self.camera.handle_input(CameraInput::Scroll { delta });
    }

    /// 키 이벤트 (카메라 + 기즈모 모드 전환)
    pub fn on_key(&mut self, key: Key, pressed: bool) {
        // 기즈모 모드 전환 (눌렀을 때만, 카메라가 비활성 상태일 때만)
        if pressed && !self.camera.is_active() {
            match key {
                Key::W => {
                    self.gizmo_mode = GizmoMode::Move;
                    log::info!("[Gizmo] Mode: Move (W)");
                }
                Key::E => {
                    self.gizmo_mode = GizmoMode::Rotate;
                    log::info!("[Gizmo] Mode: Rotate (E)");
                }
                Key::R => {
                    self.gizmo_mode = GizmoMode::Scale;
                    log::info!("[Gizmo] Mode: Scale (R)");
                }
                Key::Q => {
                    self.gizmo_mode = GizmoMode::Select;
                    log::info!("[Gizmo] Mode: Select (Q)");
                }
                _ => {}
            }
        }

        // 카메라 컨트롤 (활성 상태일 때만 WASD 작동)
        let input = if pressed {
            CameraInput::KeyDown { key }
        } else {
            CameraInput::KeyUp { key }
        };
        self.camera.handle_input(input);
    }

    /// Gizmo 위치 설정 (선택된 오브젝트 위치)
    pub fn set_gizmo_position(&mut self, position: glam::Vec3) {
        self.update_all_gizmo_positions(position);
    }

    /// 업데이트 (매 프레임 호출 - 스무딩 적용)
    pub fn update(&mut self, dt: f32) {
        // 카메라 스무딩 업데이트
        self.camera.update(dt);

        // 모든 Gizmo 스케일 업데이트 (화면 크기 고정)
        self.move_gizmo.update_scale(&self.camera, self.screen_size);
        self.rotate_gizmo.update_scale(&self.camera, self.screen_size);
        self.scale_gizmo.update_scale(&self.camera, self.screen_size);
    }

    /// 오버레이 렌더링 (그리드 + 기즈모)
    pub fn render_overlay(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_target: &wgpu::TextureView,
        depth_target: &wgpu::TextureView,
        show_grid: bool,
    ) {
        let aspect = self.screen_size.0 as f32 / self.screen_size.1 as f32;

        // 모든 Gizmo 스케일 업데이트
        self.move_gizmo.update_scale(&self.camera, self.screen_size);
        self.rotate_gizmo.update_scale(&self.camera, self.screen_size);
        self.scale_gizmo.update_scale(&self.camera, self.screen_size);

        // 그리드 렌더링 (토글 연동)
        if show_grid {
            self.grid.render(
                device,
                queue,
                encoder,
                color_target,
                depth_target,
                &self.camera,
                aspect,
            );
        }

        // 선택된 오브젝트가 없으면 Gizmo 렌더링 안함
        if self.selection.entities.is_empty() {
            return;
        }

        // 현재 모드에 따라 해당 Gizmo 렌더링
        match self.gizmo_mode {
            GizmoMode::Move => {
                self.move_gizmo.render(
                    device,
                    queue,
                    encoder,
                    color_target,
                    depth_target,
                    &self.camera,
                    aspect,
                );
            }
            GizmoMode::Rotate => {
                self.rotate_gizmo.render(
                    device,
                    queue,
                    encoder,
                    color_target,
                    depth_target,
                    &self.camera,
                    aspect,
                );
            }
            GizmoMode::Scale => {
                self.scale_gizmo.render(
                    device,
                    queue,
                    encoder,
                    color_target,
                    depth_target,
                    &self.camera,
                    aspect,
                );
            }
            GizmoMode::Select => {
                // Select 모드에서는 Gizmo 표시 안함
            }
        }
    }
}
