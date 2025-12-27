// Animation System for SKOPE Engine
// Phase 11: Skeletal Animation Playback

#![allow(dead_code)]

use glam::{Mat4, Quat, Vec3};
use crate::gltf_loader::{Animation, AnimationChannel, AnimationProperty, Interpolation, KeyframeValue};

/// 애니메이션 플레이어 상태
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
        }
    }

    result
}

/// Vec3 채널 샘플링 (Translation, Scale)
fn sample_channel_vec3(channel: &AnimationChannel, time: f32) -> Option<Vec3> {
    let keyframes = &channel.keyframes;
    if keyframes.is_empty() {
        return None;
    }

    // 시간 범위 밖인 경우 경계값 반환
    if time <= keyframes[0].time {
        return match &keyframes[0].value {
            KeyframeValue::Vec3(v) => Some(Vec3::from_array(*v)),
            _ => None,
        };
    }

    let last = keyframes.len() - 1;
    if time >= keyframes[last].time {
        return match &keyframes[last].value {
            KeyframeValue::Vec3(v) => Some(Vec3::from_array(*v)),
            _ => None,
        };
    }

    // 두 키프레임 사이 찾기
    for i in 0..last {
        let k0 = &keyframes[i];
        let k1 = &keyframes[i + 1];

        if time >= k0.time && time < k1.time {
            let t = (time - k0.time) / (k1.time - k0.time);

            match (&k0.value, &k1.value) {
                (KeyframeValue::Vec3(v0), KeyframeValue::Vec3(v1)) => {
                    let v0 = Vec3::from_array(*v0);
                    let v1 = Vec3::from_array(*v1);

                    return Some(match channel.interpolation {
                        Interpolation::Step => v0,
                        Interpolation::Linear | Interpolation::CubicSpline => v0.lerp(v1, t),
                    });
                }
                _ => return None,
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
        return match &keyframes[0].value {
            KeyframeValue::Quat(q) => Some(Quat::from_array(*q)),
            _ => None,
        };
    }

    let last = keyframes.len() - 1;
    if time >= keyframes[last].time {
        return match &keyframes[last].value {
            KeyframeValue::Quat(q) => Some(Quat::from_array(*q)),
            _ => None,
        };
    }

    // 두 키프레임 사이 찾기
    for i in 0..last {
        let k0 = &keyframes[i];
        let k1 = &keyframes[i + 1];

        if time >= k0.time && time < k1.time {
            let t = (time - k0.time) / (k1.time - k0.time);

            match (&k0.value, &k1.value) {
                (KeyframeValue::Quat(q0), KeyframeValue::Quat(q1)) => {
                    let q0 = Quat::from_array(*q0);
                    let q1 = Quat::from_array(*q1);

                    return Some(match channel.interpolation {
                        Interpolation::Step => q0,
                        Interpolation::Linear | Interpolation::CubicSpline => q0.slerp(q1, t),
                    });
                }
                _ => return None,
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
