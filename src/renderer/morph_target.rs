//! Morph Target (Shape Keys) GPU Rendering System
//!
//! GPU-side morph target computation for real-time shape key animation.
//! Blender에서 export된 Shape Key를 실시간으로 GPU에서 연산.
//!
//! ## 구조
//! - MorphTargetBuffer: 모프 타겟 델타 데이터를 GPU Storage Buffer에 저장
//! - MorphWeightsUniform: 현재 가중치를 Uniform Buffer에 저장
//!
//! ## 셰이더 통합
//! Vertex Shader에서:
//! ```wgsl
//! final_position = base_position + sum(weight[i] * delta[i])
//! ```

use wgpu::util::DeviceExt;
use crate::gltf_loader::{MorphTarget, MorphTargetData};

/// 최대 모프 타겟 수 (GPU 메모리 절약)
pub const MAX_MORPH_TARGETS: usize = 8;

/// 모프 타겟 가중치 Uniform (GPU)
/// WGSL에서 vec4<f32> 배열로 접근
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MorphWeightsUniform {
    /// 가중치 (최대 8개, vec4 2개)
    pub weights: [[f32; 4]; 2],  // 8 weights packed as 2x vec4
    /// 활성 모프 타겟 수
    pub count: u32,
    /// 패딩
    pub _pad: [u32; 3],
}

impl Default for MorphWeightsUniform {
    fn default() -> Self {
        Self {
            weights: [[0.0; 4]; 2],
            count: 0,
            _pad: [0; 3],
        }
    }
}

impl MorphWeightsUniform {
    /// 가중치 설정
    pub fn set_weights(&mut self, weights: &[f32]) {
        self.count = weights.len().min(MAX_MORPH_TARGETS) as u32;
        for (i, &weight) in weights.iter().take(self.count as usize).enumerate() {
            if i < 4 {
                self.weights[0][i] = weight;
            } else {
                self.weights[1][i - 4] = weight;
            }
        }
    }

    /// 특정 인덱스의 가중치 가져오기
    pub fn get_weight(&self, index: usize) -> f32 {
        if index >= MAX_MORPH_TARGETS {
            return 0.0;
        }
        if index < 4 {
            self.weights[0][index]
        } else {
            self.weights[1][index - 4]
        }
    }
}

/// GPU 모프 타겟 델타 (Storage Buffer용)
/// 각 vertex마다 MAX_MORPH_TARGETS개의 delta position 저장
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuMorphDelta {
    /// Position delta (vec3 + padding)
    pub position: [f32; 4],  // xyz = delta, w = padding
    /// Normal delta (vec3 + padding)
    pub normal: [f32; 4],    // xyz = delta, w = padding
}

impl Default for GpuMorphDelta {
    fn default() -> Self {
        Self {
            position: [0.0; 4],
            normal: [0.0; 4],
        }
    }
}

/// 메시별 GPU 모프 타겟 버퍼
pub struct MorphTargetBuffer {
    /// 모프 타겟 델타 Storage Buffer
    /// Layout: [vertex_count * MAX_MORPH_TARGETS] GpuMorphDelta
    pub delta_buffer: wgpu::Buffer,
    /// 가중치 Uniform Buffer
    pub weights_buffer: wgpu::Buffer,
    /// Bind Group
    pub bind_group: wgpu::BindGroup,
    /// 버텍스 수
    pub vertex_count: u32,
    /// 모프 타겟 수
    pub target_count: u32,
}

impl MorphTargetBuffer {
    /// Bind Group Layout 생성
    pub fn bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Morph Target Bind Group Layout"),
            entries: &[
                // Delta Buffer (Storage, read-only)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Weights Uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        })
    }

    /// MorphTargetData에서 GPU 버퍼 생성
    pub fn from_morph_data(
        device: &wgpu::Device,
        morph_data: &MorphTargetData,
        vertex_count: usize,
        layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let target_count = morph_data.targets.len().min(MAX_MORPH_TARGETS);

        // Delta 데이터 준비
        let mut deltas: Vec<GpuMorphDelta> = vec![GpuMorphDelta::default(); vertex_count * MAX_MORPH_TARGETS];

        for (target_idx, target) in morph_data.targets.iter().enumerate().take(target_count) {
            for (vert_idx, delta_pos) in target.position_deltas.iter().enumerate() {
                if vert_idx >= vertex_count {
                    break;
                }
                let idx = vert_idx * MAX_MORPH_TARGETS + target_idx;
                deltas[idx].position = [delta_pos[0], delta_pos[1], delta_pos[2], 0.0];

                // Normal delta (if available)
                if let Some(ref normal_deltas) = target.normal_deltas {
                    if let Some(delta_norm) = normal_deltas.get(vert_idx) {
                        deltas[idx].normal = [delta_norm[0], delta_norm[1], delta_norm[2], 0.0];
                    }
                }
            }
        }

        // Delta Storage Buffer 생성
        let delta_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Morph Delta Buffer"),
            contents: bytemuck::cast_slice(&deltas),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        // Weights Uniform Buffer 생성
        let mut weights_uniform = MorphWeightsUniform::default();
        weights_uniform.set_weights(&morph_data.current_weights);

        let weights_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Morph Weights Buffer"),
            contents: bytemuck::cast_slice(&[weights_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Bind Group 생성
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Morph Target Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: delta_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: weights_buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            delta_buffer,
            weights_buffer,
            bind_group,
            vertex_count: vertex_count as u32,
            target_count: target_count as u32,
        }
    }

    /// 가중치 업데이트
    pub fn update_weights(&self, queue: &wgpu::Queue, weights: &[f32]) {
        let mut uniform = MorphWeightsUniform::default();
        uniform.set_weights(weights);
        queue.write_buffer(&self.weights_buffer, 0, bytemuck::cast_slice(&[uniform]));
    }

    /// 모든 가중치를 0으로 리셋
    pub fn reset_weights(&self, queue: &wgpu::Queue) {
        let uniform = MorphWeightsUniform::default();
        queue.write_buffer(&self.weights_buffer, 0, bytemuck::cast_slice(&[uniform]));
    }
}

/// 빈 모프 타겟 버퍼 생성 (모프 타겟이 없는 메시용)
pub fn create_empty_morph_buffer(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
) -> MorphTargetBuffer {
    // 최소 크기 버퍼 생성
    let dummy_delta = [GpuMorphDelta::default(); 1];
    let delta_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Empty Morph Delta Buffer"),
        contents: bytemuck::cast_slice(&dummy_delta),
        usage: wgpu::BufferUsages::STORAGE,
    });

    let weights_uniform = MorphWeightsUniform::default();
    let weights_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Empty Morph Weights Buffer"),
        contents: bytemuck::cast_slice(&[weights_uniform]),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Empty Morph Bind Group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: delta_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: weights_buffer.as_entire_binding(),
            },
        ],
    });

    MorphTargetBuffer {
        delta_buffer,
        weights_buffer,
        bind_group,
        vertex_count: 0,
        target_count: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_morph_weights_uniform() {
        let mut uniform = MorphWeightsUniform::default();

        // 가중치 설정 테스트
        uniform.set_weights(&[0.5, 0.3, 0.0, 0.8, 0.1]);

        assert_eq!(uniform.count, 5);
        assert_eq!(uniform.get_weight(0), 0.5);
        assert_eq!(uniform.get_weight(1), 0.3);
        assert_eq!(uniform.get_weight(2), 0.0);
        assert_eq!(uniform.get_weight(3), 0.8);
        assert_eq!(uniform.get_weight(4), 0.1);
        assert_eq!(uniform.get_weight(5), 0.0); // 설정 안됨
    }

    #[test]
    fn test_max_morph_targets() {
        let mut uniform = MorphWeightsUniform::default();

        // 최대 개수 초과 테스트
        let weights: Vec<f32> = (0..10).map(|i| i as f32 * 0.1).collect();
        uniform.set_weights(&weights);

        assert_eq!(uniform.count, MAX_MORPH_TARGETS as u32);
    }
}
