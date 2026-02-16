// 애니메이션 업데이트 시스템
// 스켈레탈 애니메이션 샘플링 및 GPU 본 매트릭스 전송

#![allow(clippy::type_complexity)]

use skope_ecs::prelude::*;

use crate::ecs_resources::Time;
use crate::ecs_components::{AnimatorController, AiState, SkinnedMeshRenderer};
use crate::ecs_resources::{GpuContext, SkinnedModelRegistry};
use crate::renderer::{animation, skinned_mesh};

/// AI 상태 → 애니메이션 동기화 시스템
/// AI 상태가 변경되면 자동으로 AnimatorController 파라미터 설정
pub fn ai_animation_sync_system(world: &mut World) {
    // AI 상태가 변경된 엔티티들 처리
    let mut entities_to_update: Vec<(Entity, crate::ecs_components::AiStateType)> = Vec::new();

    // 먼저 변경된 AI 상태 수집
    {
        let query = world.query::<(Entity, &AiState, &AnimatorController)>();
        for (entity, ai_state, animator) in query.iter(world) {
            // AI 동기화가 활성화된 경우만
            if animator.ai_sync_enabled {
                // transition_pending이 있으면 변경 예정
                if ai_state.transition_pending.is_some() {
                    if let Some(new_state) = ai_state.transition_pending {
                        entities_to_update.push((entity, new_state));
                    }
                }
            }
        }
    }

    // 수집된 엔티티들에 대해 애니메이션 상태 적용
    for (entity, ai_state_type) in entities_to_update {
        if let Some(mut animator) = world.get_mut::<AnimatorController>(entity) {
            animator.apply_ai_state(&ai_state_type);
        }
    }
}

/// 통합 애니메이터 컨트롤러 업데이트 시스템
/// 각 엔티티의 AnimatorController를 독립적으로 업데이트
pub fn animator_controller_update_system(world: &mut World) {
    // Time 리소스에서 delta 가져오기
    let dt = world.get_resource::<Time>()
        .map(|t| t.delta_seconds)
        .unwrap_or(1.0 / 60.0);

    // SkinnedModelRegistry 가져오기
    let registry = world.get_resource::<SkinnedModelRegistry>();
    if registry.is_none() {
        return;
    }

    // 업데이트할 엔티티 정보 수집
    let mut entities_to_update: Vec<(Entity, String)> = Vec::new();

    {
        let query = world.query::<(Entity, &AnimatorController)>();
        for (entity, animator) in query.iter(world) {
            if animator.enabled && !animator.model_name.is_empty() {
                entities_to_update.push((entity, animator.model_name.clone()));
            }
        }
    }

    // 각 엔티티 업데이트
    for (entity, model_name) in entities_to_update {
        // 애니메이션 지속 시간 가져오기
        let animation_duration = {
            let registry = world.get_resource::<SkinnedModelRegistry>().unwrap();
            if let Some(model) = registry.get(&model_name) {
                if let Some(animator) = world.get::<AnimatorController>(entity) {
                    animator.current_animation_index()
                        .and_then(|idx| model.animations.get(idx))
                        .map(|a| a.duration)
                        .unwrap_or(1.0)
                } else {
                    1.0
                }
            } else {
                1.0
            }
        };

        // AnimatorController 업데이트
        if let Some(mut animator) = world.get_mut::<AnimatorController>(entity) {
            animator.update(dt, animation_duration);
        }
    }
}

/// AnimatorController 기반 GPU 본 매트릭스 업데이트 시스템
/// 블렌딩된 포즈 계산 및 GPU 버퍼 업로드
pub fn animator_controller_render_system(world: &mut World) {
    // 필요한 리소스 확인
    let has_gpu = world.get_resource::<GpuContext>().is_some();
    let has_registry = world.get_resource::<SkinnedModelRegistry>().is_some();

    if !has_gpu || !has_registry {
        return;
    }

    // 렌더링할 엔티티 정보 수집
    let mut render_data: Vec<(Entity, String, usize, f32, Option<(usize, f32, f32)>)> = Vec::new();

    {
        let query = world.query::<(Entity, &AnimatorController)>();
        for (entity, animator) in query.iter(world) {
            if !animator.enabled || animator.states.is_empty() {
                continue;
            }

            let model_name = animator.model_name.clone();
            let current_anim = animator.current_animation_index().unwrap_or(0);
            let current_time = animator.current_time;

            // 블렌딩 정보
            let blend_info = if animator.in_transition {
                let (prev_weight, curr_weight) = animator.blend_weights();
                animator.previous_animation_index().map(|prev_anim| {
                    (prev_anim, prev_weight, curr_weight)
                })
            } else {
                None
            };

            render_data.push((entity, model_name, current_anim, current_time, blend_info));
        }
    }

    // GPU 버퍼 업데이트
    for (entity, model_name, current_anim, current_time, blend_info) in render_data {
        // 모델 데이터 가져오기
        let joint_matrices = {
            let registry = world.get_resource::<SkinnedModelRegistry>().unwrap();
            let model = match registry.get(&model_name) {
                Some(m) => m,
                None => continue,
            };

            if model.animations.is_empty() {
                continue;
            }

            // 현재 애니메이션 샘플링
            let current_anim_data = match model.animations.get(current_anim) {
                Some(a) => a,
                None => continue,
            };

            let local_transforms = animation::sample_animation(
                current_anim_data,
                current_time,
            );

            // 블렌딩 처리 (전이 중인 경우)
            let final_transforms = if let Some((prev_anim, prev_weight, curr_weight)) = blend_info {
                if prev_weight > 0.001 {
                    if let Some(prev_anim_data) = model.animations.get(prev_anim) {
                        let prev_transforms = animation::sample_animation(
                            prev_anim_data,
                            0.0, // 이전 애니메이션 시간 (간단히 0으로)
                        );

                        // 선형 보간 (NodeTransform의 Option 필드 처리)
                        let mut blended = local_transforms.clone();
                        for (node_idx, curr) in blended.iter_mut() {
                            if let Some(prev) = prev_transforms.get(node_idx) {
                                // Translation 보간
                                if let (Some(c), Some(p)) = (curr.translation, prev.translation) {
                                    curr.translation = Some(p * prev_weight + c * curr_weight);
                                }
                                // Rotation 보간
                                if let (Some(c), Some(p)) = (curr.rotation, prev.rotation) {
                                    curr.rotation = Some(p.slerp(c, curr_weight));
                                }
                                // Scale 보간
                                if let (Some(c), Some(p)) = (curr.scale, prev.scale) {
                                    curr.scale = Some(p * prev_weight + c * curr_weight);
                                }
                            }
                        }
                        blended
                    } else {
                        local_transforms
                    }
                } else {
                    local_transforms
                }
            } else {
                local_transforms
            };

            // 글로벌 트랜스폼 계산
            let global_transforms = animation::compute_global_transforms(&model.nodes, &final_transforms);

            // 조인트 매트릭스 계산
            animation::compute_joint_matrices(&model.skin, &global_transforms)
        };

        // GPU 버퍼 업로드
        if let Some(renderer) = world.get::<SkinnedMeshRenderer>(entity) {
            let gpu = world.get_resource::<GpuContext>().unwrap();
            let joint_uniform = skinned_mesh::JointMatricesUniform::from_matrices(&joint_matrices);
            gpu.queue.write_buffer(
                &renderer.joint_buffer,
                0,
                bytemuck::cast_slice(&[joint_uniform]),
            );
        }
    }
}

