//! Skinned Model Loader
//!
//! glTF 스킨드 메시 로딩 및 ECS 스폰 기능
//!
//! 일부 헬퍼 함수 미사용 - 에디터/인스펙터 연동 시 활용 예정

#![allow(dead_code)]

use std::path::Path;
use std::sync::Arc;

use bevy_ecs::prelude::*;
use glam::{Mat4, Quat, Vec3};
use wgpu::util::DeviceExt;

use crate::ecs_components::*;
use crate::ecs_resources::*;
use crate::renderer::skinned_mesh::upload_skinned_mesh;

// gltf_loader alias
use skope_gltf as gltf_loader;

/// Material uniform parameters (GPU용)
#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct MaterialParams {
    base_color_factor: [f32; 4],
    emissive_factor: [f32; 3],
    metallic_factor: f32,
    roughness_factor: f32,
    _padding: [f32; 3],  // 16바이트 정렬
}

unsafe impl bytemuck::Pod for MaterialParams {}
unsafe impl bytemuck::Zeroable for MaterialParams {}

/// 스킨드 모델 로드 에러
#[derive(Debug)]
pub enum SkinnedLoadError {
    IoError(String),
    NoSkinnedMeshes,
    NoSkins,
    GpuError(String),
}

impl std::fmt::Display for SkinnedLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkinnedLoadError::IoError(e) => write!(f, "IO error: {}", e),
            SkinnedLoadError::NoSkinnedMeshes => write!(f, "No skinned meshes in model"),
            SkinnedLoadError::NoSkins => write!(f, "No skins in model"),
            SkinnedLoadError::GpuError(e) => write!(f, "GPU error: {}", e),
        }
    }
}

impl std::error::Error for SkinnedLoadError {}

/// 스킨드 모델 로드 컨텍스트 (GPU 리소스 참조)
pub struct SkinnedLoadContext<'a> {
    pub device: &'a Arc<wgpu::Device>,
    pub queue: &'a Arc<wgpu::Queue>,
    pub texture_bind_group_layout: &'a wgpu::BindGroupLayout,
    pub material_bind_group_layout: &'a wgpu::BindGroupLayout,
    pub skinned_uniform_layout: &'a wgpu::BindGroupLayout,
    pub uniform_buffer: &'a wgpu::Buffer,
}

/// glTF 파일에서 스킨드 모델 로드 및 레지스트리에 등록
///
/// # Arguments
/// * `path` - glTF 파일 경로
/// * `ctx` - GPU 리소스 컨텍스트
/// * `skinned_mesh_assets` - 스킨드 메시 에셋 레지스트리
/// * `skin_assets` - 스킨 에셋 레지스트리
/// * `skinned_model_registry` - 스킨드 모델 레지스트리
///
/// # Returns
/// 등록된 모델 이름
pub fn load_skinned_model(
    path: &Path,
    ctx: &SkinnedLoadContext,
    skinned_mesh_assets: &mut SkinnedMeshAssets,
    skin_assets: &mut SkinAssets,
    skinned_model_registry: &mut SkinnedModelRegistry,
) -> Result<String, SkinnedLoadError> {
    // 모델 이름 추출 (파일명에서)
    let model_name = path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    log::info!("Loading skinned model: {} from {:?}", model_name, path);

    // glTF 로드
    let gltf_model = gltf_loader::load_gltf(path)
        .map_err(|e| SkinnedLoadError::IoError(e.to_string()))?;

    // 스킨드 메시 확인
    if gltf_model.skinned_meshes.is_empty() {
        return Err(SkinnedLoadError::NoSkinnedMeshes);
    }
    if gltf_model.skins.is_empty() {
        return Err(SkinnedLoadError::NoSkins);
    }

    log::info!("  {} skinned meshes, {} skins, {} textures, {} materials, {} animations",
        gltf_model.skinned_meshes.len(),
        gltf_model.skins.len(),
        gltf_model.textures.len(),
        gltf_model.materials.len(),
        gltf_model.animations.len());

    // 머티리얼 바인드 그룹 생성
    let material_bind_groups = create_material_bind_groups(
        &model_name,
        &gltf_model,
        ctx,
    );

    // 스킨드 메시 GPU 업로드
    let mut mesh_indices = Vec::new();
    for (i, skinned_mesh) in gltf_model.skinned_meshes.iter().enumerate() {
        let skin = &gltf_model.skins[skinned_mesh.skin_index];

        let render_data = upload_skinned_mesh(
            ctx.device,
            skinned_mesh,
            skin,
            ctx.uniform_buffer,
            ctx.skinned_uniform_layout,
        );

        let mesh_name = format!("{}_{}", model_name, i);
        let mesh_index = skinned_mesh_assets.meshes.len();
        skinned_mesh_assets.register(&mesh_name, render_data.gpu_data);
        mesh_indices.push(mesh_index);

        log::info!("  Uploaded skinned mesh {}: {} vertices, {} indices",
            i, skinned_mesh.vertices.len(), skinned_mesh.indices.len());
    }

    // 스킨 에셋 등록
    let first_skin = &gltf_model.skins[0];
    let skin_index = skin_assets.skins.len();
    skin_assets.skins.push(SkinData {
        name: first_skin.name.clone(),
        joint_count: first_skin.joints.len(),
        inverse_bind_matrices: first_skin.joints.iter()
            .map(|j| Mat4::from_cols_array_2d(&j.inverse_bind_matrix))
            .collect(),
    });

    // SkinnedModelData 생성 및 등록
    let model_data = SkinnedModelData {
        name: model_name.clone(),
        mesh_indices,
        skin_index,
        animations: gltf_model.animations.clone(),
        nodes: gltf_model.nodes.clone(),
        skin: first_skin.clone(),
        material_bind_groups,
    };

    skinned_model_registry.register(model_data);

    log::info!("Registered skinned model '{}' with {} animations",
        model_name, gltf_model.animations.len());

    Ok(model_name)
}

/// 머티리얼 바인드 그룹 생성
fn create_material_bind_groups(
    model_name: &str,
    gltf_model: &gltf_loader::Model,
    ctx: &SkinnedLoadContext,
) -> Vec<SkinnedMaterialBindGroups> {
    let mut bind_groups = Vec::new();

    // 기본 샘플러 생성
    let default_sampler = ctx.device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    // Dummy textures 생성
    let (dummy_white, dummy_normal) = create_dummy_textures(ctx.device, ctx.queue);
    let dummy_white_view = dummy_white.create_view(&wgpu::TextureViewDescriptor::default());
    let dummy_normal_view = dummy_normal.create_view(&wgpu::TextureViewDescriptor::default());

    // 각 머티리얼에 대해 바인드 그룹 생성
    let materials: Vec<_> = if gltf_model.materials.is_empty() {
        // 기본 머티리얼
        vec![gltf_loader::Material {
            name: "default".to_string(),
            base_color_factor: [1.0, 1.0, 1.0, 1.0],
            base_color_texture: None,
            metallic_factor: 0.0,
            roughness_factor: 0.5,
            metallic_roughness_texture: None,
            normal_texture: None,
            occlusion_texture: None,
            emissive_texture: None,
            emissive_factor: [0.0, 0.0, 0.0],
        }]
    } else {
        gltf_model.materials.clone()
    };

    for (mat_idx, mat) in materials.iter().enumerate() {
        // 베이스 컬러 텍스처 로드
        let base_color_view = if let Some(tex_idx) = mat.base_color_texture {
            if tex_idx < gltf_model.textures.len() {
                let tex_data = &gltf_model.textures[tex_idx];
                let texture = create_texture_from_data(
                    ctx.device,
                    ctx.queue,
                    tex_data,
                    &format!("{}_basecolor_{}", model_name, mat_idx),
                    true, // sRGB
                );
                texture.create_view(&wgpu::TextureViewDescriptor::default())
            } else {
                dummy_white_view.clone()
            }
        } else {
            dummy_white_view.clone()
        };

        // 노멀 텍스처
        let normal_view = if let Some(tex_idx) = mat.normal_texture {
            if tex_idx < gltf_model.textures.len() {
                let tex_data = &gltf_model.textures[tex_idx];
                let texture = create_texture_from_data(
                    ctx.device,
                    ctx.queue,
                    tex_data,
                    &format!("{}_normal_{}", model_name, mat_idx),
                    false, // linear
                );
                texture.create_view(&wgpu::TextureViewDescriptor::default())
            } else {
                dummy_normal_view.clone()
            }
        } else {
            dummy_normal_view.clone()
        };

        // Texture bind group
        let texture_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{}_texture_bg_{}", model_name, mat_idx)),
            layout: ctx.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&base_color_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&default_sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&dummy_white_view) }, // metallic-roughness
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&default_sampler) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&normal_view) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::Sampler(&default_sampler) },
                wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::TextureView(&dummy_white_view) }, // occlusion
                wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::Sampler(&default_sampler) },
                wgpu::BindGroupEntry { binding: 8, resource: wgpu::BindingResource::TextureView(&dummy_white_view) }, // emissive
                wgpu::BindGroupEntry { binding: 9, resource: wgpu::BindingResource::Sampler(&default_sampler) },
            ],
        });

        // Material uniform buffer
        let mat_params = MaterialParams {
            base_color_factor: mat.base_color_factor,
            emissive_factor: mat.emissive_factor,
            metallic_factor: mat.metallic_factor,
            roughness_factor: mat.roughness_factor,
            _padding: [0.0; 3],
        };
        let mat_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{}_material_buffer_{}", model_name, mat_idx)),
            contents: bytemuck::cast_slice(&[mat_params]),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let material_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{}_material_bg_{}", model_name, mat_idx)),
            layout: ctx.material_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: mat_buffer.as_entire_binding(),
            }],
        });

        bind_groups.push(SkinnedMaterialBindGroups {
            texture_bind_group,
            material_bind_group,
        });
    }

    bind_groups
}

/// Dummy 텍스처 생성 (1x1 흰색, 1x1 플랫 노멀)
fn create_dummy_textures(device: &wgpu::Device, queue: &wgpu::Queue) -> (wgpu::Texture, wgpu::Texture) {
    // 1x1 White texture
    let white_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Skinned Dummy White 1x1"),
        size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo { texture: &white_tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        &[255u8, 255, 255, 255],
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4), rows_per_image: Some(1) },
        wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
    );

    // 1x1 Flat normal (128, 128, 255, 255)
    let normal_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Skinned Dummy Normal 1x1"),
        size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo { texture: &normal_tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        &[128u8, 128, 255, 255],
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4), rows_per_image: Some(1) },
        wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
    );

    (white_tex, normal_tex)
}

/// 텍스처 데이터에서 GPU 텍스처 생성
fn create_texture_from_data(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    tex_data: &gltf_loader::TextureData,
    label: &str,
    srgb: bool,
) -> wgpu::Texture {
    let format = if srgb {
        wgpu::TextureFormat::Rgba8UnormSrgb
    } else {
        wgpu::TextureFormat::Rgba8Unorm
    };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: tex_data.width,
            height: tex_data.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &tex_data.data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * tex_data.width),
            rows_per_image: Some(tex_data.height),
        },
        wgpu::Extent3d {
            width: tex_data.width,
            height: tex_data.height,
            depth_or_array_layers: 1,
        },
    );

    texture
}

/// 스킨드 모델 엔티티 스폰
///
/// # Arguments
/// * `world` - ECS World
/// * `model_name` - SkinnedModelRegistry에 등록된 모델 이름
/// * `position` - 월드 위치
/// * `scale` - 스케일 (glTF 모델은 보통 작게 스케일링 필요)
/// * `ctx` - GPU 컨텍스트 (조인트 버퍼 생성용)
///
/// # Returns
/// 스켈레톤 루트 엔티티 ID
pub fn spawn_skinned_model(
    world: &mut World,
    model_name: &str,
    position: Vec3,
    scale: f32,
    ctx: &SkinnedLoadContext,
) -> Option<Entity> {
    // SkinnedModelRegistry에서 모델 데이터 조회 및 필요 정보 추출
    let (skin_index, joint_count, mesh_indices, animation_names) = {
        let registry = world.get_resource::<SkinnedModelRegistry>()?;
        let model_data = registry.get(model_name)?;
        let anim_names: Vec<String> = model_data.animations
            .iter()
            .map(|a| a.name.clone())
            .collect();
        (
            model_data.skin_index,
            model_data.skin.joints.len(),
            model_data.mesh_indices.clone(),
            anim_names,
        )
    };

    // SkinnedMeshRenderer용 GPU 리소스 생성
    use wgpu::util::DeviceExt;
    use crate::renderer::skinned_mesh::{JointMatricesUniform, MAX_JOINTS};

    // 조인트 매트릭스 버퍼 생성
    let joint_uniform = JointMatricesUniform::default();
    let joint_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(&format!("{} Joint Buffer", model_name)),
        contents: bytemuck::cast_slice(&[joint_uniform]),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    // 조인트 바인드 그룹 생성
    let joint_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(&format!("{} Joint Bind Group", model_name)),
        layout: ctx.skinned_uniform_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: ctx.uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: joint_buffer.as_entire_binding(),
            },
        ],
    });

    // AnimatorController 생성 (GLTF 애니메이션 자동 등록 + AI 매핑)
    let animator = crate::ecs_components::AnimatorController::new(model_name)
        .with_animations(&animation_names)
        .with_default_ai_mappings();

    log::info!("[spawn_skinned_model] Created AnimatorController for '{}' with {} states, AI sync enabled",
        model_name, animation_names.len());

    // SkinnedMeshRenderer 생성
    let skinned_renderer = SkinnedMeshRenderer {
        model_name: model_name.to_string(),
        mesh_index: mesh_indices.first().copied().unwrap_or(0),
        joint_buffer,
        joint_bind_group,
    };

    // 스켈레톤 엔티티 생성 (SkinnedMeshRenderer 포함)
    let skeleton_entity = world.spawn((
        Transform {
            translation: position,
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(scale),
        },
        GlobalTransform::default(),
        Skeleton {
            model_name: model_name.to_string(),
            skin_index,
            joint_entities: Vec::new(),
        },
        JointMatrices {
            matrices: vec![Mat4::IDENTITY; joint_count],
        },
        animator,  // AnimatorController (상태 머신 + AI 연동)
        AnimationController::new(model_name),  // 레거시 호환 (기존 시스템용)
        skinned_renderer,  // GPU 렌더링용 (조인트 버퍼 포함)
        NodeName(format!("{}_Skeleton", model_name)),
    )).id();

    // 스킨드 메시 렌더러 엔티티 생성 (SkinnedMeshRenderer는 Phase 3에서 활성화)
    for (i, &mesh_index) in mesh_indices.iter().enumerate() {
        world.spawn((
            Transform::default(),
            GlobalTransform::default(),
            SkinnedMeshInstance {
                skinned_mesh_index: mesh_index,
                skeleton_entity,
            },
            NodeName(format!("{}_{}", model_name, i)),
        ));
    }

    log::info!("Spawned skinned model '{}' at {:?} with scale {}",
        model_name, position, scale);

    Some(skeleton_entity)
}

/// glTF 파일이 스킨드 메시를 포함하는지 확인
pub fn has_skinned_meshes(path: &Path) -> bool {
    match gltf_loader::load_gltf(path) {
        Ok(model) => !model.skinned_meshes.is_empty() && !model.skins.is_empty(),
        Err(_) => false,
    }
}

/// SkinnedModelRegistry에서 애니메이션 클립 이름 목록 반환
pub fn get_animation_names(world: &World, model_name: &str) -> Vec<String> {
    world.get_resource::<SkinnedModelRegistry>()
        .and_then(|registry| registry.get(model_name))
        .map(|model| {
            model.animations.iter()
                .map(|a| a.name.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// 모델의 애니메이션 클립 개수 반환
pub fn get_animation_count(world: &World, model_name: &str) -> usize {
    world.get_resource::<SkinnedModelRegistry>()
        .and_then(|registry| registry.get(model_name))
        .map(|model| model.animations.len())
        .unwrap_or(0)
}

/// 특정 애니메이션 클립의 duration 반환
pub fn get_animation_duration(world: &World, model_name: &str, anim_index: usize) -> f32 {
    world.get_resource::<SkinnedModelRegistry>()
        .and_then(|registry| registry.get(model_name))
        .and_then(|model| model.animations.get(anim_index))
        .map(|anim| anim.duration)
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skinned_load_error_display() {
        let err = SkinnedLoadError::NoSkinnedMeshes;
        assert_eq!(format!("{}", err), "No skinned meshes in model");
    }
}
