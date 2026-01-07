// 애니메이션 업데이트 시스템
// 스켈레탈 애니메이션 샘플링 및 GPU 본 매트릭스 전송

use bevy_ecs::prelude::*;

use crate::renderer::animation;
use crate::gltf_loader::{Animation, Skin, SceneNode};
use crate::renderer::skinned_mesh;
use crate::ecs_resources::{Time, GpuContext};

/// 애니메이션 상태 리소스 (main.rs에서 정의됨, 여기서 재정의)
#[derive(Resource)]
pub struct AnimationState {
    pub player: animation::AnimationPlayer,
    pub animation: Animation,
    pub skin: Skin,
    pub nodes: Vec<SceneNode>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animation_system_no_state() {
        // AnimationState 없이도 패닉 안 함
        let mut world = World::new();
        animation_update_system(&mut world);
    }
}
