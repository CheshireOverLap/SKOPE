// 애니메이션 업데이트 시스템
// 스켈레탈 애니메이션 샘플링 및 GPU 본 매트릭스 전송
// Phase: Animation Blending (AnimationMixer) 지원

use bevy_ecs::prelude::*;
use glam::Mat4;
use std::collections::HashMap;

use crate::renderer::animation;
use crate::renderer::animation_blend::{AnimationMixer, BlendedNodeTransform, CrossfadeTransition, BlendMode};
use crate::renderer::state_machine::{AnimatorStateMachine, LayerBlending};
use crate::gltf_loader::{Animation, Skin, SceneNode, Transform};
use crate::renderer::skinned_mesh;
use crate::ecs_resources::{Time, GpuContext};

/// 애니메이션 상태 리소스 (단일 애니메이션 - 레거시)
#[derive(Resource)]
pub struct AnimationState {
    pub player: animation::AnimationPlayer,
    pub animation: Animation,
    pub skin: Skin,
    pub nodes: Vec<SceneNode>,
}

/// 애니메이션 믹서 상태 리소스 (다중 애니메이션 블렌딩)
#[derive(Resource)]
pub struct AnimationMixerState {
    /// 애니메이션 믹서
    pub mixer: AnimationMixer,
    /// 모든 애니메이션 클립
    pub animations: Vec<Animation>,
    /// 스킨 데이터
    pub skin: Skin,
    /// 노드 계층 구조
    pub nodes: Vec<SceneNode>,
    /// 기본 트랜스폼 (블렌딩용)
    pub default_transforms: Vec<Transform>,
    /// 활성 크로스페이드 트랜지션
    pub active_transition: Option<CrossfadeTransition>,
}

impl AnimationMixerState {
    /// 새 믹서 상태 생성
    pub fn new(
        animations: Vec<Animation>,
        skin: Skin,
        nodes: Vec<SceneNode>,
    ) -> Self {
        // 기본 트랜스폼 추출
        let default_transforms: Vec<Transform> = nodes.iter()
            .map(|n| n.transform.clone())
            .collect();

        Self {
            mixer: AnimationMixer::new(),
            animations,
            skin,
            nodes,
            default_transforms,
            active_transition: None,
        }
    }

    /// 애니메이션 재생
    pub fn play(&mut self, animation_index: usize, weight: f32) -> usize {
        self.mixer.add_animation(animation_index, weight)
    }

    /// 크로스페이드 트랜지션 시작
    pub fn crossfade_to(&mut self, animation_index: usize, duration: f32) {
        // 현재 재생 중인 애니메이션 찾기
        let current_index = self.mixer.instances.iter()
            .position(|i| i.playing && i.weight > 0.5)
            .unwrap_or(0);

        // 새 애니메이션 추가 (가중치 0으로 시작)
        let new_index = self.mixer.add_animation(animation_index, 0.0);

        // 새 애니메이션 재생 시작
        if let Some(instance) = self.mixer.get_mut(new_index) {
            instance.playing = true;
            instance.time = 0.0;
        }

        // 트랜지션 설정
        self.active_transition = Some(CrossfadeTransition::new(
            current_index,
            new_index,
            duration,
        ));
    }

    /// 애니메이션 이름으로 인덱스 찾기
    pub fn find_animation(&self, name: &str) -> Option<usize> {
        self.animations.iter().position(|a| a.name == name)
    }

    /// 현재 재생 중인 애니메이션 이름들
    pub fn playing_animations(&self) -> Vec<&str> {
        self.mixer.instances.iter()
            .filter(|i| i.playing && i.weight > 0.0)
            .filter_map(|i| self.animations.get(i.animation_index))
            .map(|a| a.name.as_str())
            .collect()
    }
}

/// 애니메이터 상태 리소스 (상태 머신 기반)
#[derive(Resource)]
pub struct AnimatorStateRes {
    /// 상태 머신
    pub state_machine: AnimatorStateMachine,
    /// 모든 애니메이션 클립
    pub animations: Vec<Animation>,
    /// 스킨 데이터
    pub skin: Skin,
    /// 노드 계층 구조
    pub nodes: Vec<SceneNode>,
    /// 기본 트랜스폼 (블렌딩용)
    pub default_transforms: Vec<Transform>,
}

impl AnimatorStateRes {
    /// 새 애니메이터 상태 생성
    pub fn new(
        state_machine: AnimatorStateMachine,
        animations: Vec<Animation>,
        skin: Skin,
        nodes: Vec<SceneNode>,
    ) -> Self {
        let default_transforms: Vec<Transform> = nodes.iter()
            .map(|n| n.transform.clone())
            .collect();

        Self {
            state_machine,
            animations,
            skin,
            nodes,
            default_transforms,
        }
    }

    /// Bool 파라미터 설정
    pub fn set_bool(&mut self, name: &str, value: bool) {
        self.state_machine.set_bool(name, value);
    }

    /// Float 파라미터 설정
    pub fn set_float(&mut self, name: &str, value: f32) {
        self.state_machine.set_float(name, value);
    }

    /// Int 파라미터 설정
    pub fn set_int(&mut self, name: &str, value: i32) {
        self.state_machine.set_int(name, value);
    }

    /// Trigger 발동
    pub fn trigger(&mut self, name: &str) {
        self.state_machine.set_trigger(name);
    }

    /// 현재 상태 이름
    pub fn current_state(&self) -> &str {
        self.state_machine.current_state_name()
    }

    /// 애니메이션 지속 시간 배열 얻기
    fn animation_durations(&self) -> Vec<f32> {
        self.animations.iter().map(|a| a.duration).collect()
    }
}

/// 스킨드 메시 렌더링 데이터 리소스
#[derive(Resource)]
#[allow(dead_code)]
pub struct SkinnedMeshRenderDataRes {
    pub joint_buffer: wgpu::Buffer,
    pub joint_bind_group: wgpu::BindGroup,
    pub joint_count: usize,
}

/// 애니메이션 업데이트 시스템
///
/// 매 프레임:
/// 1. 애니메이션 시간 진행
/// 2. 키프레임 샘플링으로 로컬 트랜스폼 계산
/// 3. 노드 계층 구조에서 글로벌 트랜스폼 계산
/// 4. 조인트 매트릭스 계산 (inverse bind * global)
/// 5. GPU 버퍼에 업로드
pub fn animation_update_system(world: &mut World) {
    // delta_seconds 가져오기
    let delta_seconds = world.get_resource::<Time>()
        .map(|t| t.delta_seconds)
        .unwrap_or(0.016);

    // GPU Context 가져오기 (버퍼 업로드용)
    let queue = world.get_resource::<GpuContext>()
        .map(|ctx| ctx.queue.clone());

    // AnimationState가 있으면 업데이트
    if let Some(mut anim_state) = world.remove_resource::<AnimationState>() {
        // 1. 애니메이션 시간 업데이트
        anim_state.player.update(delta_seconds, anim_state.animation.duration);

        // 2. 현재 시간의 노드 트랜스폼 샘플링
        let local_transforms = animation::sample_animation(
            &anim_state.animation,
            anim_state.player.current_time,
        );

        // 3. 글로벌 트랜스폼 계산
        let global_transforms = animation::compute_global_transforms(
            &anim_state.nodes,
            &local_transforms,
        );

        // 4. 조인트 매트릭스 계산
        let joint_matrices = animation::compute_joint_matrices(
            &anim_state.skin,
            &global_transforms,
        );

        // 5. GPU 버퍼에 조인트 매트릭스 전송
        if let (Some(ref queue), Some(skinned_render_data)) =
            (&queue, world.get_resource::<SkinnedMeshRenderDataRes>())
        {
            let joint_uniform = skinned_mesh::JointMatricesUniform::from_matrices(&joint_matrices);
            queue.write_buffer(
                &skinned_render_data.joint_buffer,
                0,
                bytemuck::cast_slice(&[joint_uniform]),
            );
        }

        // AnimationState 다시 넣기
        world.insert_resource(anim_state);
    }
}

/// 애니메이션 믹서 업데이트 시스템 (다중 애니메이션 블렌딩)
///
/// AnimationMixerState가 있으면 블렌딩된 애니메이션 처리
pub fn animation_mixer_update_system(world: &mut World) {
    // delta_seconds 가져오기
    let delta_seconds = world.get_resource::<Time>()
        .map(|t| t.delta_seconds)
        .unwrap_or(0.016);

    // GPU Context 가져오기 (버퍼 업로드용)
    let queue = world.get_resource::<GpuContext>()
        .map(|ctx| ctx.queue.clone());

    // AnimationMixerState가 있으면 업데이트
    if let Some(mut mixer_state) = world.remove_resource::<AnimationMixerState>() {
        // 1. 크로스페이드 트랜지션 업데이트
        if let Some(ref mut transition) = mixer_state.active_transition {
            transition.update(delta_seconds, &mut mixer_state.mixer);
            if transition.is_completed() {
                mixer_state.active_transition = None;
            }
        }

        // 2. 모든 애니메이션 인스턴스 시간 업데이트
        mixer_state.mixer.update(delta_seconds, &mixer_state.animations);

        // 3. 블렌딩된 포즈 샘플링
        let blended_transforms = mixer_state.mixer.sample(
            &mixer_state.animations,
            &mixer_state.default_transforms,
        );

        // 4. BlendedNodeTransform → NodeTransform 변환하여 글로벌 트랜스폼 계산
        let local_transforms: HashMap<usize, animation::NodeTransform> = blended_transforms
            .iter()
            .map(|(&node_idx, blended)| {
                (node_idx, animation::NodeTransform {
                    translation: Some(blended.translation),
                    rotation: Some(blended.rotation),
                    scale: Some(blended.scale),
                })
            })
            .collect();

        // 5. 글로벌 트랜스폼 계산
        let global_transforms = animation::compute_global_transforms(
            &mixer_state.nodes,
            &local_transforms,
        );

        // 6. 조인트 매트릭스 계산
        let joint_matrices = animation::compute_joint_matrices(
            &mixer_state.skin,
            &global_transforms,
        );

        // 7. GPU 버퍼에 조인트 매트릭스 전송
        if let (Some(ref queue), Some(skinned_render_data)) =
            (&queue, world.get_resource::<SkinnedMeshRenderDataRes>())
        {
            let joint_uniform = skinned_mesh::JointMatricesUniform::from_matrices(&joint_matrices);
            queue.write_buffer(
                &skinned_render_data.joint_buffer,
                0,
                bytemuck::cast_slice(&[joint_uniform]),
            );
        }

        // AnimationMixerState 다시 넣기
        world.insert_resource(mixer_state);
    }
}

/// BlendedNodeTransform에서 Mat4로 변환
#[allow(dead_code)]
fn blended_transform_to_matrix(transform: &BlendedNodeTransform) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        transform.scale,
        transform.rotation,
        transform.translation,
    )
}

/// 애니메이터 상태 머신 업데이트 시스템
///
/// AnimatorStateRes가 있으면 상태 머신 기반 애니메이션 처리
pub fn animator_state_machine_update_system(world: &mut World) {
    // delta_seconds 가져오기
    let delta_seconds = world.get_resource::<Time>()
        .map(|t| t.delta_seconds)
        .unwrap_or(0.016);

    // GPU Context 가져오기 (버퍼 업로드용)
    let queue = world.get_resource::<GpuContext>()
        .map(|ctx| ctx.queue.clone());

    // AnimatorStateRes가 있으면 업데이트
    if let Some(mut animator_state) = world.remove_resource::<AnimatorStateRes>() {
        // 1. 애니메이션 지속 시간 배열 생성
        let durations = animator_state.animation_durations();

        // 2. 상태 머신 업데이트 (전이 검사, 시간 진행)
        animator_state.state_machine.update(delta_seconds, &durations);

        // 3. 현재 재생해야 할 애니메이션 가중치 얻기
        let current_animations = animator_state.state_machine.get_current_animations();

        // 4. 블렌딩된 로컬 트랜스폼 계산
        let local_transforms = compute_blended_transforms_from_state_machine(
            &current_animations,
            &animator_state.animations,
            &animator_state.default_transforms,
            &animator_state.state_machine,
        );

        // 5. 글로벌 트랜스폼 계산
        let global_transforms = animation::compute_global_transforms(
            &animator_state.nodes,
            &local_transforms,
        );

        // 6. 조인트 매트릭스 계산
        let joint_matrices = animation::compute_joint_matrices(
            &animator_state.skin,
            &global_transforms,
        );

        // 7. GPU 버퍼에 조인트 매트릭스 전송
        if let (Some(ref queue), Some(skinned_render_data)) =
            (&queue, world.get_resource::<SkinnedMeshRenderDataRes>())
        {
            let joint_uniform = skinned_mesh::JointMatricesUniform::from_matrices(&joint_matrices);
            queue.write_buffer(
                &skinned_render_data.joint_buffer,
                0,
                bytemuck::cast_slice(&[joint_uniform]),
            );
        }

        // AnimatorStateRes 다시 넣기
        world.insert_resource(animator_state);
    }
}

/// 상태 머신에서 블렌딩된 트랜스폼 계산
fn compute_blended_transforms_from_state_machine(
    animations_info: &[(usize, f32, LayerBlending, Option<&[usize]>)],
    animations: &[Animation],
    default_transforms: &[Transform],
    state_machine: &AnimatorStateMachine,
) -> HashMap<usize, animation::NodeTransform> {
    let mut result: HashMap<usize, animation::NodeTransform> = HashMap::new();

    // Override 레이어의 총 가중치
    let total_override_weight: f32 = animations_info.iter()
        .filter(|(_, _, blending, _)| *blending == LayerBlending::Override)
        .map(|(_, w, _, _)| w)
        .sum();

    // 각 애니메이션에 대해 블렌딩
    for &(anim_idx, weight, blending, bone_mask) in animations_info {
        if weight <= 0.0 {
            continue;
        }

        let anim = match animations.get(anim_idx) {
            Some(a) => a,
            None => continue,
        };

        // 현재 레이어의 시간 찾기 (첫 번째 레이어 사용)
        let current_time = state_machine.layers.first()
            .map(|l| l.current_time)
            .unwrap_or(0.0);

        // 애니메이션 샘플링
        let sampled = animation::sample_animation(anim, current_time);

        // 정규화된 가중치 (Override 모드)
        let normalized_weight = if blending == LayerBlending::Override && total_override_weight > 0.0 {
            weight / total_override_weight
        } else {
            weight
        };

        // 각 노드에 대해 블렌딩
        for (&node_idx, node_transform) in &sampled {
            // 본 마스크 검사
            if let Some(mask) = bone_mask {
                if !mask.contains(&node_idx) {
                    continue; // 마스크에 없는 본은 스킵
                }
            }

            let default_transform = default_transforms.get(node_idx)
                .cloned()
                .unwrap_or_default();

            match blending {
                LayerBlending::Override => {
                    // Override 블렌딩
                    let entry = result.entry(node_idx).or_insert_with(|| animation::NodeTransform {
                        translation: None,
                        rotation: None,
                        scale: None,
                    });

                    // Translation 블렌딩
                    if let Some(new_trans) = node_transform.translation {
                        let current = entry.translation.unwrap_or_else(||
                            glam::Vec3::from_array(default_transform.translation));
                        entry.translation = Some(current.lerp(new_trans, normalized_weight));
                    }

                    // Rotation 블렌딩
                    if let Some(new_rot) = node_transform.rotation {
                        let current = entry.rotation.unwrap_or_else(||
                            glam::Quat::from_array(default_transform.rotation));
                        entry.rotation = Some(current.slerp(new_rot, normalized_weight));
                    }

                    // Scale 블렌딩
                    if let Some(new_scale) = node_transform.scale {
                        let current = entry.scale.unwrap_or_else(||
                            glam::Vec3::from_array(default_transform.scale));
                        entry.scale = Some(current.lerp(new_scale, normalized_weight));
                    }
                }
                LayerBlending::Additive => {
                    // Additive 블렌딩 (기준 포즈와의 차이를 더함)
                    let reference_trans = glam::Vec3::from_array(default_transform.translation);
                    let reference_rot = glam::Quat::from_array(default_transform.rotation);
                    let reference_scale = glam::Vec3::from_array(default_transform.scale);

                    let entry = result.entry(node_idx).or_insert_with(|| animation::NodeTransform {
                        translation: Some(reference_trans),
                        rotation: Some(reference_rot),
                        scale: Some(reference_scale),
                    });

                    // Translation: 차이를 더함
                    if let Some(new_trans) = node_transform.translation {
                        let delta = new_trans - reference_trans;
                        let current = entry.translation.unwrap_or(reference_trans);
                        entry.translation = Some(current + delta * weight);
                    }

                    // Rotation: 차이를 곱함
                    if let Some(new_rot) = node_transform.rotation {
                        let delta = reference_rot.inverse() * new_rot;
                        let current = entry.rotation.unwrap_or(reference_rot);
                        entry.rotation = Some(current * glam::Quat::IDENTITY.slerp(delta, weight));
                    }

                    // Scale: 차이를 더함
                    if let Some(new_scale) = node_transform.scale {
                        let delta = new_scale - reference_scale;
                        let current = entry.scale.unwrap_or(reference_scale);
                        entry.scale = Some(current + delta * weight);
                    }
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animation_system_no_state() {
        // AnimationState 없이도 패닉 안 함
        let mut world = World::new();
        animation_update_system(&mut world);
    }

    #[test]
    fn test_animation_mixer_system_no_state() {
        // AnimationMixerState 없이도 패닉 안 함
        let mut world = World::new();
        animation_mixer_update_system(&mut world);
    }

    #[test]
    fn test_animator_state_machine_system_no_state() {
        // AnimatorStateRes 없이도 패닉 안 함
        let mut world = World::new();
        animator_state_machine_update_system(&mut world);
    }
}
