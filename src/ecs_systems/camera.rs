// 카메라 시스템
// 입력 처리, SpringArm 업데이트, CameraShake 적용, 렌더링 데이터 추출

use skope_ecs::prelude::*;
use glam::{Mat4, Vec3, Quat};
use winit::keyboard::KeyCode;

use crate::ecs_components::{Transform, Camera, CameraController, SpringArm};
use crate::ecs_resources::{
    Time, KeyboardInput, WindowSize, RenderExtractedData, ExtractedCamera,
    ActiveCameraShakes, ViewTargetBlend,
};
use crate::physics::PhysicsWorld;

/// 카메라 입력 시스템 - WASD 이동 및 마우스 회전
/// SpringArm이 있는 카메라는 spring_arm_update_system이 위치를 제어하므로 제외.
pub fn camera_input_system(
    time: Res<Time>,
    keyboard: Res<KeyboardInput>,
    mut query: Query<(&mut Transform, &CameraController), (With<Camera>, Without<SpringArm>)>,
) {
    let delta_time = time.delta_seconds;

    for (mut transform, controller) in query.iter_mut() {
        let move_speed = controller.move_speed * delta_time;

        // Forward/right 벡터 계산 (Z-up 좌표계: Blender 호환)
        // yaw=0, pitch=0일 때 -Y 방향 (Blender front view)
        let forward = Vec3::new(
            -controller.yaw.sin() * controller.pitch.cos(),
            -controller.yaw.cos() * controller.pitch.cos(),
            controller.pitch.sin(),
        )
        .normalize();

        let right = Vec3::new(
            controller.yaw.cos(),
            -controller.yaw.sin(),
            0.0,
        )
        .normalize();

        let up = Vec3::Z;

        // 키 입력에 따른 이동
        if keyboard.keys_pressed.contains(&KeyCode::KeyW) {
            transform.translation += forward * move_speed;
        }
        if keyboard.keys_pressed.contains(&KeyCode::KeyS) {
            transform.translation -= forward * move_speed;
        }
        if keyboard.keys_pressed.contains(&KeyCode::KeyA) {
            transform.translation -= right * move_speed;
        }
        if keyboard.keys_pressed.contains(&KeyCode::KeyD) {
            transform.translation += right * move_speed;
        }
        if keyboard.keys_pressed.contains(&KeyCode::Space) {
            transform.translation += up * move_speed;
        }
        if keyboard.keys_pressed.contains(&KeyCode::ShiftLeft) {
            transform.translation -= up * move_speed;
        }
    }
}

// ============ Spring Arm System (UE5 UpdateDesiredArmLocation) ============

/// UE5-style SpringArm update system.
///
/// For each entity with (SpringArm, CameraController, Transform):
/// 1. Compute desired rotation from CameraController yaw/pitch
/// 2. Apply rotation lag (optional)
/// 3. Compute arm origin = entity position + target_offset
/// 4. Apply position lag (optional, with sub-stepping)
/// 5. Position camera = origin - forward * arm_length + socket_offset
/// 6. (Collision sweep not yet implemented — placeholder)
///
/// The resulting Transform is where the camera entity should be placed.
/// This system runs BEFORE camera_extract_system.
pub fn spring_arm_update_system(
    time: Res<Time>,
    physics: Option<Res<PhysicsWorld>>,
    mut query: Query<(&mut SpringArm, &CameraController, &mut Transform), With<Camera>>,
) {
    let dt = time.delta_seconds;
    if dt <= 0.0 { return; }

    for (mut arm, controller, mut transform) in query.iter_mut() {
        // UE5 OnRegister: enforce reasonable limits to avoid div-by-zero
        arm.lag_max_time_step = arm.lag_max_time_step.max(1.0 / 200.0);
        arm.camera_lag_speed = arm.camera_lag_speed.max(0.0);

        // --- Step 1: Desired rotation from CameraController ---
        // Build a quaternion from yaw/pitch (Z-up convention)
        let desired_rot = Quat::from_euler(
            glam::EulerRot::ZXY,
            controller.yaw,
            controller.pitch,
            0.0,
        );

        // --- Step 2: Rotation lag ---
        let final_rot = if arm.enable_camera_rotation_lag && !arm.first_update {
            let lagged = apply_rotation_lag(
                arm.previous_desired_rot,
                desired_rot,
                dt,
                arm.camera_rotation_lag_speed,
                arm.use_lag_substepping,
                arm.lag_max_time_step,
            );
            arm.previous_desired_rot = lagged;
            lagged
        } else {
            arm.previous_desired_rot = desired_rot;
            desired_rot
        };

        // --- Step 3: Arm origin (target position + offset) ---
        // The arm origin is the current entity translation + target_offset.
        // For a third-person camera following a player, this entity
        // should be positioned at the player by camera_follow_player_system.
        let arm_origin = transform.translation + arm.target_offset;
        let mut desired_loc = arm_origin;

        // --- Step 4: Position lag ---
        if arm.enable_camera_lag && !arm.first_update {
            desired_loc = apply_position_lag(
                arm.previous_desired_loc,
                desired_loc,
                dt,
                arm.camera_lag_speed,
                arm.camera_lag_max_distance,
                arm_origin,
                arm.use_lag_substepping,
                arm.lag_max_time_step,
            );
        }
        arm.previous_arm_origin = arm_origin;
        arm.previous_desired_loc = desired_loc;

        // --- Step 5: Position camera along arm ---
        // Forward direction derived from final_rot (respects rotation lag).
        // Extract lagged yaw/pitch from the quaternion so the arm direction
        // follows the LAGGED rotation, not the raw controller input.
        let (lagged_yaw, lagged_pitch, _) = final_rot.to_euler(glam::EulerRot::ZXY);
        let forward = Vec3::new(
            -lagged_yaw.sin() * lagged_pitch.cos(),
            -lagged_yaw.cos() * lagged_pitch.cos(),
            lagged_pitch.sin(),
        ).normalize();

        // Camera is placed behind the target along -forward
        let mut cam_pos = desired_loc - forward * arm.target_arm_length;

        // Add socket offset in camera-local space
        let rot_mat = glam::Mat3::from_quat(final_rot);
        cam_pos += rot_mat * arm.socket_offset;

        // --- Step 6: Collision sweep ---
        // UE5 SpringArm collision: sphere sweep from arm origin to desired camera position.
        // If the sweep hits scene geometry, shorten the arm to the hit distance.
        arm.unfixed_camera_position = cam_pos;
        arm.is_camera_fixed = false;

        if arm.do_collision_test {
            if let Some(ref physics) = physics {
                let sweep_origin = desired_loc;
                let sweep_dir = cam_pos - sweep_origin;
                let sweep_dist = sweep_dir.length();
                if sweep_dist > 1e-4 {
                    if let Some(hit_dist) = physics.sphere_cast(
                        sweep_origin,
                        sweep_dir,
                        sweep_dist,
                        arm.probe_size,
                    ) {
                        cam_pos = sweep_origin + sweep_dir.normalize() * hit_dist;
                        arm.is_camera_fixed = true;
                    }
                }
            }
        }

        // Write final camera position
        transform.translation = cam_pos;
        transform.rotation = final_rot;

        arm.first_update = false;
    }
}

/// VInterpTo — exponential approach interpolation (UE5 FMath::VInterpTo)
fn vinterp_to(current: Vec3, target: Vec3, dt: f32, speed: f32) -> Vec3 {
    if speed <= 0.0 { return target; }
    let dist = target - current;
    if dist.length_squared() < 1e-8 { return target; }
    let alpha = (dt * speed).clamp(0.0, 1.0);
    current + dist * alpha
}

/// QInterpTo — quaternion slerp toward target (UE5 FMath::QInterpTo)
fn qinterp_to(current: Quat, target: Quat, dt: f32, speed: f32) -> Quat {
    if speed <= 0.0 { return target; }
    let alpha = (dt * speed).clamp(0.0, 1.0);
    current.slerp(target, alpha)
}

/// Apply position lag with optional sub-stepping
fn apply_position_lag(
    previous: Vec3,
    target: Vec3,
    dt: f32,
    speed: f32,
    max_distance: f32,
    arm_origin: Vec3,
    use_substepping: bool,
    max_time_step: f32,
) -> Vec3 {
    let mut result;

    if use_substepping && dt > max_time_step && speed > 0.0 {
        let move_step = (target - previous) / dt;
        let mut lerp_target = previous;
        let mut remaining = dt;
        result = previous;

        while remaining > 1e-4 {
            let step = remaining.min(max_time_step);
            lerp_target += move_step * step;
            remaining -= step;
            result = vinterp_to(result, lerp_target, step, speed);
        }
    } else {
        result = vinterp_to(previous, target, dt, speed);
    }

    // Clamp max lag distance
    if max_distance > 0.0 {
        let from_origin = result - arm_origin;
        let dist_sq = from_origin.length_squared();
        if dist_sq > max_distance * max_distance {
            let clamped = from_origin.normalize() * max_distance;
            result = arm_origin + clamped;
        }
    }

    result
}

/// Apply rotation lag with optional sub-stepping
fn apply_rotation_lag(
    previous: Quat,
    target: Quat,
    dt: f32,
    speed: f32,
    use_substepping: bool,
    max_time_step: f32,
) -> Quat {
    if use_substepping && dt > max_time_step && speed > 0.0 {
        // Substep rotation interpolation
        let mut result = previous;
        let mut remaining = dt;
        // Compute per-second "velocity" — slerp direction
        while remaining > 1e-4 {
            let step = remaining.min(max_time_step);
            remaining -= step;
            // Interpolate from current toward target
            let t_frac = if dt > 0.0 { 1.0 - remaining / dt } else { 1.0 };
            let sub_target = previous.slerp(target, t_frac);
            result = qinterp_to(result, sub_target, step, speed);
        }
        result
    } else {
        qinterp_to(previous, target, dt, speed)
    }
}

// ============ Camera Shake System ============

/// Clean up finished camera shakes.
/// Runs before camera_extract_system (chained) so expired shakes are removed
/// before the extract step computes and applies shake offsets.
pub fn camera_shake_update_system(
    mut shakes: Option<ResMut<ActiveCameraShakes>>,
) {
    let Some(ref mut shakes) = shakes else { return; };
    shakes.cleanup();
}

// ============ Camera Extract System ============

/// 카메라 데이터 추출 시스템 — 렌더링용 뷰/프로젝션 매트릭스 계산.
///
/// 적용 순서:
/// 1. ECS Transform + CameraController에서 기본 카메라 상태 읽기
/// 2. ViewTargetBlend 적용 (카메라 전환 보간)
/// 3. CameraShake 적용 (진동 오프셋)
/// 4. ExtractedCamera에 최종 결과 기록
pub fn camera_extract_system(
    time: Res<Time>,
    camera_query: Query<(&Transform, &CameraController, &Camera)>,
    window_size: Option<Res<WindowSize>>,
    mut extracted_data: ResMut<RenderExtractedData>,
    mut shakes: Option<ResMut<ActiveCameraShakes>>,
    mut blend: Option<ResMut<ViewTargetBlend>>,
) {
    let dt = time.delta_seconds;

    // 기본 화면 비율
    let aspect = window_size
        .map(|ws| ws.width as f32 / ws.height.max(1) as f32)
        .unwrap_or(16.0 / 9.0);

    for (transform, controller, camera) in camera_query.iter() {
        if !camera.is_active {
            continue;
        }

        let mut cam_pos = transform.translation;
        let mut yaw = controller.yaw;
        let mut pitch = controller.pitch;
        let mut fov = camera.fov;

        // --- Stage 1: ViewTargetBlend (카메라 전환 보간) ---
        if let Some(ref mut blend) = blend {
            if let Some(blended) = blend.update(dt) {
                cam_pos = blended.position;
                yaw = blended.yaw;
                pitch = blended.pitch;
                fov = blended.fov;
            }
        }

        // --- Stage 2: CameraShake (진동 오프셋 적용) ---
        if let Some(ref mut shakes) = shakes {
            let mut total_loc = Vec3::ZERO;
            let mut total_rot = Vec3::ZERO;
            let mut total_fov = 0.0f32;

            for shake in shakes.shakes.iter_mut() {
                let (loc, rot, fov_offset) = shake.compute(dt);
                total_loc += loc;
                total_rot += rot;
                total_fov += fov_offset;
            }

            // Apply location offset in camera-local space
            let fwd = Vec3::new(
                -yaw.sin() * pitch.cos(),
                -yaw.cos() * pitch.cos(),
                pitch.sin(),
            ).normalize();
            let right = Vec3::new(yaw.cos(), -yaw.sin(), 0.0).normalize();
            let up = fwd.cross(right).normalize();

            cam_pos += right * total_loc.x + up * total_loc.z + fwd * total_loc.y;

            // Apply rotation offset (degrees → radians)
            pitch += total_rot.x.to_radians();
            yaw += total_rot.y.to_radians();
            pitch = pitch.clamp(-89.9f32.to_radians(), 89.9f32.to_radians());

            // Apply FOV offset
            fov += total_fov.to_radians();
        }

        // --- Stage 3: 최종 매트릭스 계산 ---
        let forward = Vec3::new(
            -yaw.sin() * pitch.cos(),
            -yaw.cos() * pitch.cos(),
            pitch.sin(),
        )
        .normalize();

        let view = Mat4::look_at_rh(
            cam_pos,
            cam_pos + forward,
            Vec3::Z,
        );

        let projection = Mat4::perspective_rh(
            fov,
            aspect,
            camera.near,
            camera.far,
        );

        extracted_data.camera = Some(ExtractedCamera {
            position: cam_pos,
            view_matrix: view,
            projection_matrix: projection,
            view_projection: projection * view,
            forward,
            yaw,
            pitch,
            near: camera.near,
            far: camera.far,
            fov,
        });

        break; // 첫 번째 활성 카메라만
    }
}
