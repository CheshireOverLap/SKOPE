//! SKOPE Material GPU Sync
//!
//! MaterialRegistry의 변경사항을 GPU에 동기화

use crate::renderer::{GpuMaterial, MaterialEvalPipeline};

use super::registry::MaterialRegistry;

/// MaterialRegistry의 변경사항을 GPU에 동기화
///
/// dirty 플래그가 설정된 머티리얼만 개별 write로 업데이트합니다.
/// bulk write는 Phase 10.3 슬롯을 덮어쓸 수 있으므로 항상 개별 write 사용.
pub fn sync_materials_to_gpu(
    registry: &mut MaterialRegistry,
    pipeline: &MaterialEvalPipeline,
    queue: &wgpu::Queue,
) {
    let updates = registry.collect_dirty();
    if updates.is_empty() {
        return;
    }

    for (index, gpu_mat) in &updates {
        let offset = (*index * std::mem::size_of::<GpuMaterial>()) as u64;
        queue.write_buffer(
            &pipeline.material_buffer,
            offset,
            bytemuck::cast_slice(&[*gpu_mat]),
        );
    }
    log::debug!("[MaterialSync] Updated {} materials", updates.len());
}

/// 전체 머티리얼을 GPU에 강제 업로드 (개별 write)
pub fn force_upload_all_materials(
    registry: &MaterialRegistry,
    pipeline: &MaterialEvalPipeline,
    queue: &wgpu::Queue,
) {
    let mut count = 0;
    for (_, entry) in registry.iter() {
        let gpu_mat = entry.to_gpu_material();
        let offset = (entry.gpu_index * std::mem::size_of::<GpuMaterial>()) as u64;
        queue.write_buffer(
            &pipeline.material_buffer,
            offset,
            bytemuck::cast_slice(&[gpu_mat]),
        );
        count += 1;
    }
    if count > 0 {
        log::info!("[MaterialSync] Force uploaded {} materials to GPU", count);
    }
}
