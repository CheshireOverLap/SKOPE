//! SKOPE Material GPU Sync
//!
//! MaterialRegistry의 변경사항을 GPU에 동기화

use crate::renderer::{GpuMaterial, MaterialEvalPipeline};

use super::registry::MaterialRegistry;

/// MaterialRegistry의 변경사항을 GPU에 동기화
///
/// dirty 플래그가 설정된 머티리얼만 업데이트합니다.
pub fn sync_materials_to_gpu(
    registry: &mut MaterialRegistry,
    pipeline: &MaterialEvalPipeline,
    queue: &wgpu::Queue,
) {
    let updates = registry.collect_dirty();

    if updates.is_empty() {
        return;
    }

    // 개별 업데이트 또는 전체 재업로드 선택
    // 소량의 변경은 개별 업데이트, 대량은 전체 재업로드가 효율적
    let update_threshold = 8; // 8개 이상이면 전체 재업로드
    let update_count = updates.len();

    if update_count < update_threshold {
        // 소량: 개별 업데이트
        for (index, gpu_mat) in updates {
            let offset = (index * std::mem::size_of::<GpuMaterial>()) as u64;
            queue.write_buffer(
                &pipeline.material_buffer,
                offset,
                bytemuck::cast_slice(&[gpu_mat]),
            );
        }
        log::debug!(
            "[MaterialSync] Updated {} materials individually",
            update_count
        );
    } else {
        // 대량: 전체 재업로드
        let all_materials = registry.to_gpu_materials();
        pipeline.update_materials(queue, &all_materials);
        log::debug!(
            "[MaterialSync] Bulk updated {} materials",
            all_materials.len()
        );
    }
}

/// 전체 머티리얼을 GPU에 강제 업로드
pub fn force_upload_all_materials(
    registry: &MaterialRegistry,
    pipeline: &MaterialEvalPipeline,
    queue: &wgpu::Queue,
) {
    let all_materials = registry.to_gpu_materials();

    if all_materials.is_empty() {
        return;
    }

    pipeline.update_materials(queue, &all_materials);
    log::info!(
        "[MaterialSync] Force uploaded {} materials to GPU",
        all_materials.len()
    );
}
