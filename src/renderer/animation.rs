// Animation System for SKOPE Engine
// Phase 11: Skeletal Animation Playback

#![allow(dead_code)]

use glam::{Mat4, Quat, Vec3};
use crate::gltf_loader::{Animation, AnimationChannel, AnimationProperty, Interpolation, KeyframeValue};

/// 애니메이션 플레이어 상태
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AnimationPlayer {
    /// 현재 재생 시간 (초)
    pub current_time: f32,
    /// 재생 속도 (1.0 = 정상)
    pub speed: f32,
    /// 루프 재생 여부
    pub looping: bool,
    /// 재생 중인지
    pub playing: bool,
}

impl Default for AnimationPlayer {
    fn default() -> Self {
        Self {
            current_time: 0.0,
            speed: 1.0,
            looping: true,
            playing: true,
        }
    }
}

impl AnimationPlayer {
    /// 시간 업데이트
    #[allow(dead_code)]
    pub fn update(&mut self, delta_seconds: f32, animation_duration: f32) {
        if !self.playing || animation_duration <= 0.0 {
            return;
        }

        self.current_time += delta_seconds * self.speed;

        if self.looping {
            // 루프: duration을 넘으면 처음으로
            while self.current_time >= animation_duration {
                self.current_time -= animation_duration;
            }
            while self.current_time < 0.0 {
                self.current_time += animation_duration;
            }
        } else {
            // 비루프: clamp
            self.current_time = self.current_time.clamp(0.0, animation_duration);
            if self.current_time >= animation_duration {
                self.playing = false;
            }
        }
    }
}

/// 노드별 애니메이션 결과 (로컬 트랜스폼)
#[derive(Debug, Clone, Default)]
pub struct NodeTransform {
    pub translation: Option<Vec3>,
    pub rotation: Option<Quat>,
    pub scale: Option<Vec3>,
}

/// 애니메이션 샘플링 - 특정 시간의 모든 노드 트랜스폼 계산
pub fn sample_animation(animation: &Animation, time: f32) -> std::collections::HashMap<usize, NodeTransform> {
    let mut result: std::collections::HashMap<usize, NodeTransform> = std::collections::HashMap::new();

    for channel in &animation.channels {
        let node_entry = result.entry(channel.node_index).or_default();

        match channel.property {
            AnimationProperty::Translation => {
                if let Some(value) = sample_channel_vec3(channel, time) {
                    node_entry.translation = Some(value);
                }
            }
            AnimationProperty::Rotation => {
                if let Some(value) = sample_channel_quat(channel, time) {
                    node_entry.rotation = Some(value);
                }
            }
            AnimationProperty::Scale => {
                if let Some(value) = sample_channel_vec3(channel, time) {
                    node_entry.scale = Some(value);
                }
            }
            AnimationProperty::MorphTargetWeights => {
                // Morph Target Weights는 별도 처리 (NodeTransform에 포함 안됨)
                // sample_morph_weights 함수로 별도 샘플링
            }
        }
    }

    result
}

/// Hermite CubicSpline 보간 (Vec3)
/// p(t) = (2t³-3t²+1)p0 + (t³-2t²+t)(dt·m0) + (-2t³+3t²)p1 + (t³-t²)(dt·m1)
#[allow(dead_code)]
fn cubic_spline_interpolate_vec3(
    p0: Vec3, m0: Vec3, p1: Vec3, m1: Vec3, t: f32, dt: f32,
) -> Vec3 {
    let t2 = t * t;
    let t3 = t2 * t;
    let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
    let h10 = t3 - 2.0 * t2 + t;
    let h01 = -2.0 * t3 + 3.0 * t2;
    let h11 = t3 - t2;
    p0 * h00 + m0 * (dt * h10) + p1 * h01 + m1 * (dt * h11)
}

/// Hermite CubicSpline 보간 (Quat)
#[allow(dead_code)]
fn cubic_spline_interpolate_quat(
    p0: Quat, m0: [f32; 4], p1: Quat, m1: [f32; 4], t: f32, dt: f32,
) -> Quat {
    let t2 = t * t;
    let t3 = t2 * t;
    let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
    let h10 = t3 - 2.0 * t2 + t;
    let h01 = -2.0 * t3 + 3.0 * t2;
    let h11 = t3 - t2;

    // 쿼터니언: 각 성분을 Hermite로 보간 후 정규화
    let p0a = [p0.x, p0.y, p0.z, p0.w];
    let p1a = [p1.x, p1.y, p1.z, p1.w];

    let mut result = [0.0f32; 4];
    for i in 0..4 {
        result[i] = p0a[i] * h00 + m0[i] * (dt * h10) + p1a[i] * h01 + m1[i] * (dt * h11);
    }

    Quat::from_array(result).normalize()
}

/// Extract Vec3 value from keyframe (handles both Vec3 and CubicSplineVec3)
#[allow(dead_code)]
fn extract_vec3_value(value: &KeyframeValue) -> Option<Vec3> {
    match value {
        KeyframeValue::Vec3(v) => Some(Vec3::from_array(*v)),
        KeyframeValue::CubicSplineVec3 { value, .. } => Some(Vec3::from_array(*value)),
        _ => None,
    }
}

/// Extract Quat value from keyframe (handles both Quat and CubicSplineQuat)
#[allow(dead_code)]
fn extract_quat_value(value: &KeyframeValue) -> Option<Quat> {
    match value {
        KeyframeValue::Quat(q) => Some(Quat::from_array(*q)),
        KeyframeValue::CubicSplineQuat { value, .. } => Some(Quat::from_array(*value)),
        _ => None,
    }
}

/// Vec3 채널 샘플링 (Translation, Scale)
fn sample_channel_vec3(channel: &AnimationChannel, time: f32) -> Option<Vec3> {
    let keyframes = &channel.keyframes;
    if keyframes.is_empty() {
        return None;
    }

    // 시간 범위 밖인 경우 경계값 반환
    if time <= keyframes[0].time {
        return extract_vec3_value(&keyframes[0].value);
    }

    let last = keyframes.len() - 1;
    if time >= keyframes[last].time {
        return extract_vec3_value(&keyframes[last].value);
    }

    // 두 키프레임 사이 찾기
    for i in 0..last {
        let k0 = &keyframes[i];
        let k1 = &keyframes[i + 1];

        if time >= k0.time && time < k1.time {
            let t = (time - k0.time) / (k1.time - k0.time);
            let dt = k1.time - k0.time;

            match channel.interpolation {
                Interpolation::Step => {
                    return extract_vec3_value(&k0.value);
                }
                Interpolation::Linear => {
                    let v0 = extract_vec3_value(&k0.value)?;
                    let v1 = extract_vec3_value(&k1.value)?;
                    return Some(v0.lerp(v1, t));
                }
                Interpolation::CubicSpline => {
                    // CubicSpline: Hermite 보간
                    match (&k0.value, &k1.value) {
                        (
                            KeyframeValue::CubicSplineVec3 { value: v0, out_tangent: m0, .. },
                            KeyframeValue::CubicSplineVec3 { in_tangent: m1, value: v1, .. },
                        ) => {
                            return Some(cubic_spline_interpolate_vec3(
                                Vec3::from_array(*v0), Vec3::from_array(*m0),
                                Vec3::from_array(*v1), Vec3::from_array(*m1),
                                t, dt,
                            ));
                        }
                        // Fallback: Linear if data is plain Vec3 (shouldn't happen)
                        _ => {
                            let v0 = extract_vec3_value(&k0.value)?;
                            let v1 = extract_vec3_value(&k1.value)?;
                            return Some(v0.lerp(v1, t));
                        }
                    }
                }
            }
        }
    }

    None
}

/// Quaternion 채널 샘플링 (Rotation)
fn sample_channel_quat(channel: &AnimationChannel, time: f32) -> Option<Quat> {
    let keyframes = &channel.keyframes;
    if keyframes.is_empty() {
        return None;
    }

    // 시간 범위 밖인 경우 경계값 반환
    if time <= keyframes[0].time {
        return extract_quat_value(&keyframes[0].value);
    }

    let last = keyframes.len() - 1;
    if time >= keyframes[last].time {
        return extract_quat_value(&keyframes[last].value);
    }

    // 두 키프레임 사이 찾기
    for i in 0..last {
        let k0 = &keyframes[i];
        let k1 = &keyframes[i + 1];

        if time >= k0.time && time < k1.time {
            let t = (time - k0.time) / (k1.time - k0.time);
            let dt = k1.time - k0.time;

            match channel.interpolation {
                Interpolation::Step => {
                    return extract_quat_value(&k0.value);
                }
                Interpolation::Linear => {
                    let q0 = extract_quat_value(&k0.value)?;
                    let q1 = extract_quat_value(&k1.value)?;
                    return Some(q0.slerp(q1, t));
                }
                Interpolation::CubicSpline => {
                    match (&k0.value, &k1.value) {
                        (
                            KeyframeValue::CubicSplineQuat { value: q0, out_tangent: m0, .. },
                            KeyframeValue::CubicSplineQuat { in_tangent: m1, value: q1, .. },
                        ) => {
                            return Some(cubic_spline_interpolate_quat(
                                Quat::from_array(*q0), *m0,
                                Quat::from_array(*q1), *m1,
                                t, dt,
                            ));
                        }
                        // Fallback: slerp if data is plain Quat
                        _ => {
                            let q0 = extract_quat_value(&k0.value)?;
                            let q1 = extract_quat_value(&k1.value)?;
                            return Some(q0.slerp(q1, t));
                        }
                    }
                }
            }
        }
    }

    None
}

/// 노드 트랜스폼을 Mat4로 변환
pub fn node_transform_to_matrix(transform: &NodeTransform, default_transform: &crate::gltf_loader::Transform) -> Mat4 {
    let translation = transform.translation.unwrap_or_else(|| Vec3::from_array(default_transform.translation));
    let rotation = transform.rotation.unwrap_or_else(|| Quat::from_array(default_transform.rotation));
    let scale = transform.scale.unwrap_or_else(|| Vec3::from_array(default_transform.scale));

    Mat4::from_scale_rotation_translation(scale, rotation, translation)
}

/// 노드 계층구조를 따라 글로벌 트랜스폼 계산
pub fn compute_global_transforms(
    nodes: &[crate::gltf_loader::SceneNode],
    local_transforms: &std::collections::HashMap<usize, NodeTransform>,
) -> Vec<Mat4> {
    let mut global_transforms = vec![Mat4::IDENTITY; nodes.len()];
    let mut computed = vec![false; nodes.len()];

    // 재귀적으로 계산
    fn compute_recursive(
        node_idx: usize,
        nodes: &[crate::gltf_loader::SceneNode],
        local_transforms: &std::collections::HashMap<usize, NodeTransform>,
        global_transforms: &mut [Mat4],
        computed: &mut [bool],
        parent_transform: Mat4,
    ) {
        if computed[node_idx] {
            return;
        }

        let node = &nodes[node_idx];

        // 로컬 트랜스폼 계산
        let local_matrix = if let Some(anim_transform) = local_transforms.get(&node_idx) {
            node_transform_to_matrix(anim_transform, &node.transform)
        } else {
            // 애니메이션 없으면 기본 트랜스폼 사용
            let t = Vec3::from_array(node.transform.translation);
            let r = Quat::from_array(node.transform.rotation);
            let s = Vec3::from_array(node.transform.scale);
            Mat4::from_scale_rotation_translation(s, r, t)
        };

        // 글로벌 = 부모 * 로컬
        global_transforms[node_idx] = parent_transform * local_matrix;
        computed[node_idx] = true;

        // 자식들 재귀 처리
        for &child_idx in &node.children {
            compute_recursive(
                child_idx,
                nodes,
                local_transforms,
                global_transforms,
                computed,
                global_transforms[node_idx],
            );
        }
    }

    // 모든 루트 노드에서 시작
    for node_idx in 0..nodes.len() {
        if !computed[node_idx] {
            // 부모가 없는 노드 찾기 (루트)
            let is_root = !nodes.iter().any(|n| n.children.contains(&node_idx));
            if is_root {
                compute_recursive(
                    node_idx,
                    nodes,
                    local_transforms,
                    &mut global_transforms,
                    &mut computed,
                    Mat4::IDENTITY,
                );
            }
        }
    }

    // 남은 노드들 처리 (혹시 고아 노드가 있다면)
    for node_idx in 0..nodes.len() {
        if !computed[node_idx] {
            compute_recursive(
                node_idx,
                nodes,
                local_transforms,
                &mut global_transforms,
                &mut computed,
                Mat4::IDENTITY,
            );
        }
    }

    global_transforms
}

/// 스킨(스켈레톤)용 조인트 매트릭스 계산
/// joint_matrix = global_transform * inverse_bind_matrix
pub fn compute_joint_matrices(
    skin: &crate::gltf_loader::Skin,
    global_transforms: &[Mat4],
) -> Vec<Mat4> {
    skin.joints.iter().map(|joint| {
        let global_transform = global_transforms.get(joint.node_index).copied().unwrap_or(Mat4::IDENTITY);
        let inverse_bind = Mat4::from_cols_array_2d(&joint.inverse_bind_matrix);
        global_transform * inverse_bind
    }).collect()
}

// ============ Morph Target (Shape Key) Animation Functions ============

/// 특정 노드의 Morph Target Weights 채널 찾기
pub fn find_morph_weight_channel(animation: &Animation, node_index: usize) -> Option<&AnimationChannel> {
    animation.channels.iter().find(|c|
        c.node_index == node_index && c.property == AnimationProperty::MorphTargetWeights
    )
}

/// Morph Target Weights 샘플링
/// 주어진 시간에 대해 보간된 가중치 배열 반환
pub fn sample_morph_weights(animation: &Animation, node_index: usize, time: f32) -> Option<Vec<f32>> {
    let channel = find_morph_weight_channel(animation, node_index)?;
    sample_weights_channel(channel, time)
}

/// Weights 채널 샘플링 (MorphTargetWeights용)
fn sample_weights_channel(channel: &AnimationChannel, time: f32) -> Option<Vec<f32>> {
    let keyframes = &channel.keyframes;

    if keyframes.is_empty() {
        return None;
    }

    // 첫 키프레임 이전
    if time <= keyframes[0].time {
        return match &keyframes[0].value {
            KeyframeValue::Weights(w) => Some(w.clone()),
            _ => None,
        };
    }

    // 마지막 키프레임 이후
    if time >= keyframes.last()?.time {
        return match &keyframes.last()?.value {
            KeyframeValue::Weights(w) => Some(w.clone()),
            _ => None,
        };
    }

    // 보간할 두 키프레임 찾기
    for i in 0..keyframes.len() - 1 {
        let k0 = &keyframes[i];
        let k1 = &keyframes[i + 1];

        if time >= k0.time && time < k1.time {
            let t = (time - k0.time) / (k1.time - k0.time);

            match (&k0.value, &k1.value) {
                (KeyframeValue::Weights(w0), KeyframeValue::Weights(w1)) => {
                    if w0.len() != w1.len() {
                        return None;
                    }

                    return Some(match channel.interpolation {
                        Interpolation::Step => w0.clone(),
                        Interpolation::Linear | Interpolation::CubicSpline => {
                            // 선형 보간
                            w0.iter()
                                .zip(w1.iter())
                                .map(|(&a, &b)| a + (b - a) * t)
                                .collect()
                        }
                    });
                }
                _ => return None,
            }
        }
    }

    None
}

/// 현재 Morph Target Weights를 적용하여 정점 위치 계산
/// base_positions + sum(weight[i] * delta_positions[i])
pub fn apply_morph_targets(
    base_positions: &[[f32; 3]],
    morph_targets: &[crate::gltf_loader::MorphTarget],
    weights: &[f32],
) -> Vec<[f32; 3]> {
    let mut result: Vec<[f32; 3]> = base_positions.to_vec();

    for (target_idx, target) in morph_targets.iter().enumerate() {
        let weight = weights.get(target_idx).copied().unwrap_or(0.0);
        if weight.abs() < 0.0001 {
            continue; // 무시할 수 있는 가중치는 건너뜀
        }

        for (i, delta) in target.position_deltas.iter().enumerate() {
            if i < result.len() {
                result[i][0] += delta[0] * weight;
                result[i][1] += delta[1] * weight;
                result[i][2] += delta[2] * weight;
            }
        }
    }

    result
}

/// Normals에도 Morph Target 적용 (optional normals)
pub fn apply_morph_targets_normals(
    base_normals: &[[f32; 3]],
    morph_targets: &[crate::gltf_loader::MorphTarget],
    weights: &[f32],
) -> Vec<[f32; 3]> {
    let mut result: Vec<[f32; 3]> = base_normals.to_vec();

    for (target_idx, target) in morph_targets.iter().enumerate() {
        let weight = weights.get(target_idx).copied().unwrap_or(0.0);
        if weight.abs() < 0.0001 {
            continue;
        }

        if let Some(ref normal_deltas) = target.normal_deltas {
            for (i, delta) in normal_deltas.iter().enumerate() {
                if i < result.len() {
                    result[i][0] += delta[0] * weight;
                    result[i][1] += delta[1] * weight;
                    result[i][2] += delta[2] * weight;
                }
            }
        }
    }

    // 정규화
    for normal in &mut result {
        let len = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
        if len > 0.0001 {
            normal[0] /= len;
            normal[1] /= len;
            normal[2] /= len;
        }
    }

    result
}
