//! State render function
//!
//! Main rendering logic extracted from state.rs

#![allow(clippy::type_complexity)]

use std::collections::HashMap;
use wgpu::util::DeviceExt;
use bevy_ecs::prelude::*;
use bevy_hierarchy::prelude::*;

use super::State;
// data_types is re-exported from mod.rs (super)
use super::{Uniforms, AnimationState, CameraRenderData, SkinnedMeshRenderDataRes};

use crate::gltf_loader;
use crate::ecs_components;
use crate::ecs_resources;
use crate::assets;
use crate::skope_data;
use crate::physics;
use crate::hair;
use crate::renderer;
use crate::debug;
use crate::ui;
use crate::scripting;
use crate::particles;
use skope_effects as effects;
use skope_magic as magic;
use crate::prefab;
use crate::editor;
use crate::paths;

impl State {
    pub fn render(
        &mut self,
        world: &mut World,
        debug_ui: &mut debug::ui::DebugUi,
        game_ui: &mut ui::UiSystem,
        ui_hot_reloader: &mut ui::HotReloader,
        mut scene_viewer: Option<&mut editor::scene_viewer::SceneViewer>,
        _command_stack: &mut editor::command::CommandStack,
        editor_debug_viz: &editor::debug_viz::EditorDebugViz,
        magic_builder: Option<&mut crate::game::MagicCircleBuilderState>,
        delta_time: f32,
        viewport_size_override: Option<(u32, u32)>,
    ) -> Result<(), wgpu::SurfaceError> {
        // Frame count for debugging
        static mut FRAME_COUNT: u32 = 0;
        unsafe {
            FRAME_COUNT += 1;
        }

        // NOTE: Physics simulation is now handled in ECS physics_step_system

        // ============ Transform Propagation (Transform -> GlobalTransform) ============
        crate::ecs_systems::transform_propagate_system(world);

        // ============ Viewport Texture resize and setup ============
        // NOTE: 뷰포트 패널 크기에 맞게 텍스처 리사이즈
        // UE FlushRenderingCommands 패턴: EngineHandler의 도킹 패널에서 직접 읽은
        // 최신 뷰포트 크기를 사용하여 1프레임 지연 제거
        {
            // viewport_size_override: EngineHandler의 dock_panel에서 읽은 최신 크기
            // (State.editor_ui_state의 stale rect 대신 사용)
            let (vp_w, vp_h) = if let Some((w, h)) = viewport_size_override {
                (w, h)
            } else if let Some(ref ui_state) = self.editor_ui_state {
                let (_, _, w, h) = ui_state.get_viewport_rect();
                (w as u32, h as u32)
            } else {
                // fallback: 현재 텍스처 크기 유지
                self.viewport_texture.size
            };
            let current_tex_size = self.viewport_texture.size;

            // 뷰포트 패널 크기가 변경되면 텍스처 리사이즈
            if vp_w > 0 && vp_h > 0 && (current_tex_size.0 != vp_w || current_tex_size.1 != vp_h) {
                log::info!("[Viewport] Resizing texture: {}x{} -> {}x{}",
                    current_tex_size.0, current_tex_size.1, vp_w, vp_h);

                // 뷰포트 텍스처 리사이즈
                self.viewport_texture.resize(&self.device, (vp_w, vp_h));
                self.game_viewport_texture.resize(&self.device, (vp_w, vp_h));

                // Deferred 렌더러도 새 크기로 리사이즈
                self.deferred_renderer.resize(&self.device, vp_w, vp_h);

                // skope_ui에 뷰포트 텍스처 업데이트
                if let Some(ref mut ui_state) = self.editor_ui_state {
                    ui_state.update_viewport_texture(
                        &self.device,
                        &self.viewport_texture.view,
                        (vp_w, vp_h),
                    );
                }

                log::info!("[Viewport] Texture updated");

                // scene_viewer도 새 크기로 업데이트
                if let Some(ref mut sv) = scene_viewer {
                    sv.resize(vp_w, vp_h);
                }
            }
        }

        // ============ Phase 11: Animation update and bone matrices GPU transfer ============
        {
            // Get delta_seconds
            let delta_seconds = world.get_resource::<ecs_resources::Time>()
                .map(|t| t.delta_seconds)
                .unwrap_or(0.016);

            // ECS AnimationController component update (new system)
            {
                // 1. First create animation duration map from Registry
                let duration_map: HashMap<(String, usize), f32> = {
                    if let Some(registry) = world.get_resource::<ecs_resources::SkinnedModelRegistry>() {
                        registry.models.iter()
                            .flat_map(|(name, data)| {
                                data.animations.iter().enumerate()
                                    .map(move |(idx, anim)| ((name.clone(), idx), anim.duration))
                            })
                            .collect()
                    } else {
                        HashMap::new()
                    }
                };

                // 2. Collect entities that need updates
                let updates: Vec<(bevy_ecs::entity::Entity, f32)> = {
                    let mut query = world.query::<(bevy_ecs::entity::Entity, &ecs_components::Skeleton, &ecs_components::AnimationController)>();

                    query.iter(world)
                        .filter(|(_, _, ctrl)| ctrl.playing)
                        .map(|(entity, skeleton, ctrl)| {
                            let duration = duration_map
                                .get(&(skeleton.model_name.clone(), ctrl.current_animation))
                                .copied()
                                .unwrap_or(1.0);
                            (entity, duration)
                        })
                        .collect()
                };

                // 3. Update AnimationController
                for (entity, duration) in updates {
                    if let Some(mut anim_ctrl) = world.get_mut::<ecs_components::AnimationController>(entity) {
                        anim_ctrl.update(delta_seconds, duration);
                    }
                }
            }

            // AnimationState update if exists (legacy compatibility)
            if let Some(mut anim_state) = world.remove_resource::<AnimationState>() {
                // Sync with AnimationController (find Fox entity)
                let sync_time = {
                    let mut query = world.query::<(&ecs_components::Skeleton, &ecs_components::AnimationController)>();
                    query.iter(world)
                        .find(|(s, _)| s.model_name == "Fox")
                        .map(|(_, ctrl)| ctrl.current_time)
                };

                // Sync to AnimationController time (if exists)
                if let Some(ctrl_time) = sync_time {
                    anim_state.player.current_time = ctrl_time;
                } else {
                    // Legacy: player self update
                    anim_state.player.update(delta_seconds, anim_state.animation.duration);
                }

                // 2. Sample node transforms at current time
                let local_transforms = renderer::animation::sample_animation(
                    &anim_state.animation,
                    anim_state.player.current_time,
                );

                // 3. Compute global transforms
                let global_transforms = renderer::animation::compute_global_transforms(
                    &anim_state.nodes,
                    &local_transforms,
                );

                // 4. Compute joint matrices
                let joint_matrices = renderer::animation::compute_joint_matrices(
                    &anim_state.skin,
                    &global_transforms,
                );

                // 5. Transfer joint matrices to GPU buffer (현재 + 이전 프레임 for TAA velocity)
                if let Some(mut skinned_render_data) = world.remove_resource::<SkinnedMeshRenderDataRes>() {
                    // 이전 프레임 매트릭스와 함께 업로드 (TAA velocity용)
                    let joint_uniform = renderer::skinned_mesh::JointMatricesUniform::from_matrices_with_prev(
                        &joint_matrices,
                        &skinned_render_data.prev_joint_matrices,
                    );
                    self.queue.write_buffer(
                        &skinned_render_data.joint_buffer,
                        0,
                        bytemuck::cast_slice(&[joint_uniform]),
                    );

                    // 현재 매트릭스를 다음 프레임의 "이전"으로 저장
                    skinned_render_data.prev_joint_matrices = joint_matrices.clone();

                    // 리소스 복원
                    world.insert_resource(skinned_render_data);
                }

                // Debug: print every 60 frames
                // Animation playback (debug logs removed for cleaner output)

                // Put AnimationState back
                world.insert_resource(anim_state);
            }
        }

        // ============ Phase 4: Get camera info (Scene View / Game View separation) ============
        let (vp_w, vp_h) = self.viewport_texture.size;
        let scene_aspect = if vp_w > 0 && vp_h > 0 {
            vp_w as f32 / vp_h as f32
        } else {
            self.size.width as f32 / self.size.height as f32
        };

        // Scene View camera (EditorCamera)
        let scene_camera = if let Some(ref sv) = scene_viewer {
            let cam = &sv.camera;
            CameraRenderData {
                view: cam.view_matrix(),
                proj: cam.projection_matrix(scene_aspect),
                position: cam.position,
            }
        } else {
            // Fallback: default camera
            let pos = glam::Vec3::new(0.0, -10.0, 5.0);
            let view = glam::Mat4::look_at_rh(pos, glam::Vec3::ZERO, glam::Vec3::Z);
            let proj = glam::Mat4::perspective_rh(45.0_f32.to_radians(), scene_aspect, 0.1, 100.0);
            CameraRenderData { view, proj, position: pos }
        };

        // Game View camera (ECS Camera)
        let (game_vp_w, game_vp_h) = self.game_viewport_texture.size;
        let game_aspect = if game_vp_w > 0 && game_vp_h > 0 {
            game_vp_w as f32 / game_vp_h as f32
        } else {
            scene_aspect
        };
        let game_camera = CameraRenderData::from_ecs_camera(world, game_aspect);

        // Camera for main rendering (use Scene View)
        let (view, proj, camera_pos) = (scene_camera.view, scene_camera.proj, scene_camera.position);

        // Camera positions updated (debug logs removed for cleaner output)

        // ============ Phase 6: ECS Query to collect mesh instances with Frustum Culling ============
        // Create view frustum for culling
        let view_proj = proj * view;
        let frustum = renderer::frustum::Frustum::from_view_proj(view_proj);

        // Query ECS entities directly instead of scene node traversal
        let mesh_instances: Vec<(usize, usize, glam::Mat4)> = {
            // First try with MeshBounds for precise culling
            let mut query_with_bounds = world.query_filtered::<(
                Entity,
                &ecs_components::MeshInstance,
                &ecs_components::MaterialHandle,
                &ecs_components::GlobalTransform,
                &ecs_components::MeshBounds,
            ), Without<ecs_components::Hidden>>();

            let mut results: Vec<_> = query_with_bounds
                .iter(world)
                .filter(|(_, _, _, global_transform, bounds)| {
                    // Transform bounding sphere to world space and test against frustum
                    let world_center = global_transform.0.transform_point3(bounds.sphere_center);
                    let scale = glam::Vec3::new(
                        global_transform.0.x_axis.truncate().length(),
                        global_transform.0.y_axis.truncate().length(),
                        global_transform.0.z_axis.truncate().length(),
                    );
                    let max_scale = scale.x.max(scale.y).max(scale.z);
                    let world_radius = bounds.sphere_radius * max_scale;
                    frustum.test_sphere(world_center, world_radius)
                })
                .map(|(_, mesh_instance, material_handle, global_transform, _)| {
                    (
                        mesh_instance.mesh_index,
                        material_handle.material_index,
                        global_transform.0,
                    )
                })
                .collect();

            // Also include entities without MeshBounds (no culling for them)
            let mut query_without_bounds = world.query_filtered::<(
                Entity,
                &ecs_components::MeshInstance,
                &ecs_components::MaterialHandle,
                &ecs_components::GlobalTransform,
            ), (Without<ecs_components::Hidden>, Without<ecs_components::MeshBounds>)>();

            let additional: Vec<_> = query_without_bounds
                .iter(world)
                .map(|(_, mesh_instance, material_handle, global_transform)| {
                    (
                        mesh_instance.mesh_index,
                        material_handle.material_index,
                        global_transform.0,
                    )
                })
                .collect();

            results.extend(additional);
            results
        };

        // ============ Phase 17a: Update LightManager (before borrowing other resources) ============
        {
            // Clone the Arc'd device/queue for use in this scope
            let gpu_ctx = world.get_resource::<ecs_resources::GpuContext>().unwrap();
            let device = gpu_ctx.device.clone();
            let queue = gpu_ctx.queue.clone();
            let _ = gpu_ctx;  // Release immutable borrow

            if let Some(mut light_manager_res) = world.get_resource_mut::<ecs_resources::LightManagerRes>() {
                light_manager_res.manager.update_gpu_buffers(&device, &queue);

                if let (Some(light_buf), Some(count_buf)) = (
                    light_manager_res.manager.light_buffer(),
                    light_manager_res.manager.light_count_buffer(),
                ) {
                    self.deferred_renderer.update_light_buffers(
                        &device,
                        light_buf,
                        count_buf,
                    );
                }

                // Phase 14: Update clustered lighting for V-Buffer renderer
                // Phase 28: Pass texture array views to maintain per-frame binding
                self.deferred_renderer.update_clustered_lighting(
                    &device,
                    &queue,
                    &mut light_manager_res.manager,
                    view,
                    proj,
                    Some((
                        &self.texture_array_manager.albedo_array.view,
                        &self.texture_array_manager.normal_array.view,
                        &self.texture_array_manager.metallic_roughness_array.view,
                    )),
                );
            }
        }

        // ============ Skinned mesh instance query (ECS based) ============
        // Also store skeleton_entity to find SkinnedMeshRenderer
        let skinned_instances: Vec<(usize, glam::Mat4, bevy_ecs::entity::Entity)> = {
            let mut instances = Vec::new();
            for (instance, transform) in world.query::<(&ecs_components::SkinnedMeshInstance, &ecs_components::Transform)>().iter(world) {
                let model_matrix = glam::Mat4::from_scale_rotation_translation(
                    transform.scale,
                    transform.rotation,
                    transform.translation,
                );
                instances.push((instance.skinned_mesh_index, model_matrix, instance.skeleton_entity));
            }
            instances
        };

        // ============ Phase 5: Get GPU data from ECS Resources ============
        let mesh_assets = world.get_resource::<ecs_resources::MeshAssets>().unwrap();
        let material_assets = world.get_resource::<ecs_resources::MaterialAssets>().unwrap();
        let gpu_context = world.get_resource::<ecs_resources::GpuContext>().unwrap();

        // Rendering info collection (debug logs removed for cleaner output)

        // Surface가 있으면 swapchain output 가져오기 (headless 모드에서는 None)
        let (output, texture_view) = if let Some(ref surface) = self.surface {
            let output = surface.get_current_texture()?;
            let view = output
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            (Some(output), Some(view))
        } else {
            (None, None)
        };

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // ============ Shadow Pass ============
        let sun_direction = glam::Vec3::new(-0.5, -1.0, -0.3).normalize();
        {
            // Calculate cascade matrices
            let cascades = self.shadow_map.calculate_cascade_matrices(
                view,
                proj,
                sun_direction,
                0.1,   // near
                100.0, // far
            );

            // Update shadow uniforms
            self.shadow_map.update_uniforms(&self.queue, &cascades);

            // Collect shadow casters
            let shadow_meshes: Vec<(glam::Mat4, &wgpu::Buffer, &wgpu::Buffer, u32)> = mesh_instances
                .iter()
                .map(|(mesh_idx, _mat_idx, world_transform)| {
                    let mesh_data = &mesh_assets.meshes[*mesh_idx];
                    (*world_transform, &mesh_data.vertex_buffer, &mesh_data.index_buffer, mesh_data.num_indices)
                })
                .collect();

            // Render shadow maps (using uniform buffer approach)
            self.shadow_map.render_shadows(&mut encoder, &self.queue, &shadow_meshes);
        }

        // ============ Phase 17: Deferred Rendering ============
        {
            // Read lighting settings from Environment resource
            let env = world.get_resource::<ecs_resources::Environment>()
                .cloned()
                .unwrap_or_default();

            // Sun light (TODO: read from ECS Light component)
            let sun_color = glam::Vec3::new(1.0, 1.0, 1.0);
            let sun_intensity = 4.0;

            // Use debug_ui's debug_view (visible in F3 panel)
            let debug_mode = debug_ui.debug_view.to_shader_mode();

            // Log on debug mode change
            {
                use std::sync::atomic::{AtomicU32, Ordering};
                static LAST_DEBUG_MODE: AtomicU32 = AtomicU32::new(0);
                let prev = LAST_DEBUG_MODE.swap(debug_mode, Ordering::Relaxed);
                if debug_mode != prev {
                    log::info!("[DEBUG] debug_mode changed: {} -> {}", prev, debug_mode);
                }
            }

            self.deferred_renderer.update_lighting_with_env(
                &self.queue,
                view,
                proj,
                camera_pos,
                sun_direction,
                sun_color,
                sun_intensity,
                &env,
                // PBR Debug parameters from UI
                debug_ui.intensity_scale,
                debug_ui.d_ggx_max,
                debug_ui.specular_max,
                debug_ui.roughness_min,
                debug_mode,
            );

            // Update blit params for tonemapping bypass in debug mode
            self.deferred_renderer.update_blit_params(&self.queue, debug_mode);

            // Prepare mesh render data for deferred rendering
            let mut mesh_render_data: Vec<(
                wgpu::Buffer,     // camera uniform buffer
                wgpu::Buffer,     // model uniform buffer
                wgpu::BindGroup,  // camera bind group
                usize,            // mesh_idx
                usize,            // material_idx
                [[f32; 4]; 4],    // model_matrix (for World Space UV)
            )> = Vec::new();

            for (i, (mesh_idx, material_idx, world_transform)) in mesh_instances.iter().enumerate() {
                // Debug: first frame only
                static mut FIRST_FRAME: bool = true;
                unsafe {
                    if FIRST_FRAME {
                        let pos = world_transform.w_axis;
                        let scale_x = world_transform.x_axis.length();
                        let scale_y = world_transform.y_axis.length();
                        let scale_z = world_transform.z_axis.length();
                        log::info!("[RENDER] Instance {}: mesh={}, mat={}, pos=({:.2},{:.2},{:.2}), scale=({:.2},{:.2},{:.2})",
                                 i, mesh_idx, material_idx, pos.x, pos.y, pos.z, scale_x, scale_y, scale_z);
                        if i == mesh_instances.len() - 1 {
                            FIRST_FRAME = false;
                        }
                    }
                }

                // Camera uniform
                let camera_uniform = renderer::CameraUniform::new(
                    view,
                    proj,
                    camera_pos,
                    (self.size.width, self.size.height),
                    0.1,
                    100.0,
                );

                let camera_buffer = gpu_context.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("Camera Uniform Buffer {}", i)),
                    contents: bytemuck::cast_slice(&[camera_uniform]),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

                // Model uniform
                let model_uniform = renderer::ModelUniform::new(*world_transform);

                let model_buffer = gpu_context.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("Model Uniform Buffer {}", i)),
                    contents: bytemuck::cast_slice(&[model_uniform]),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

                // Camera + Model bind group
                let camera_bind_group = gpu_context.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(&format!("Camera Bind Group {}", i)),
                    layout: self.deferred_renderer.camera_bind_group_layout(),
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: camera_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: model_buffer.as_entire_binding(),
                        },
                    ],
                });

                // Convert world_transform to column-major array
                let model_matrix = world_transform.to_cols_array_2d();
                mesh_render_data.push((camera_buffer, model_buffer, camera_bind_group, *mesh_idx, *material_idx, model_matrix));
            }

            // Build MeshRenderData slice
            // Only glTF meshes are in the unified geometry buffer (front part of mesh_assets)
            // Only set geometry_mesh_idx when mesh_idx < num_gltf_meshes
            let num_gltf_meshes = self.deferred_renderer.geometry_buffer
                .as_ref()
                .map(|g| g.mesh_infos.len())
                .unwrap_or(0);

            let render_meshes: Vec<renderer::MeshRenderData> = mesh_render_data
                .iter()
                .map(|(_, _, camera_bind_group, mesh_idx, material_idx, model_matrix)| {
                    let mesh_data = &mesh_assets.meshes[*mesh_idx];
                    // For standalone materials (only in gpu_materials), use default material bind group
                    // V-Buffer evaluates actual materials via gpu_materials array so this is safe
                    let material = if *material_idx < material_assets.materials.len() {
                        &material_assets.materials[*material_idx]
                    } else {
                        &material_assets.materials[0]  // default white material
                    };

                    // glTF mesh: use index if mesh_idx is within geometry buffer range
                    // Procedural mesh (Cube, Sphere, etc.): None since not in geometry buffer
                    let geometry_mesh_idx = if *mesh_idx < num_gltf_meshes {
                        Some(*mesh_idx)
                    } else {
                        None
                    };

                    renderer::MeshRenderData {
                        vertex_buffer: &mesh_data.vertex_buffer,
                        index_buffer: &mesh_data.index_buffer,
                        index_count: mesh_data.num_indices,
                        camera_bind_group,
                        material_bind_group: material.deferred_bind_group.as_ref()
                            .unwrap_or(&material.material_bind_group),
                        geometry_mesh_idx,
                        material_index: *material_idx as u32,
                        model_matrix: *model_matrix,
                    }
                })
                .collect();

            // Initialize viewport_texture depth buffer (used by forward pass and grid)
            {
                let _ = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Viewport Depth Clear"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: self.viewport_texture.depth_target(),
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
            }

            // Sync debug UI screen-space effect settings to renderer
            // Note: SSAO replaced by GTAO
            self.deferred_renderer.settings.enable_gtao = debug_ui.ssao_enabled;
            // TODO: Add SSR, Contact Shadows, Volumetric, SSS toggles to DebugUi
            // Currently using RenderSettings defaults

            // Call V-Buffer renderer
            // Render to viewport texture (displayed in UI panel)
            self.deferred_renderer.render_vbuffer(
                &self.device,
                &mut encoder,
                self.viewport_texture.render_target(),  // render to viewport texture
                &render_meshes,
                &self.queue,
                view,
                proj,
                sun_direction,
                sun_color,
            );

            // Copy V-Buffer depth to viewport_texture depth
            // So skinned mesh, grid, gizmo can depth test correctly afterwards
            self.deferred_renderer.copy_depth_to(
                &mut encoder,
                &self.viewport_texture.depth_texture,
            );

            // Debug: first frame
            // V-Buffer pipeline complete (debug logs removed)
        }

        // ============ Skinned Mesh Forward Pass (Scene View) - ECS based ============
        // Render only when SkinnedMeshInstance entities exist
        if !skinned_instances.is_empty() {
            if let (Some(skinned_pipeline), Some(skinned_assets), Some(skinned_render_data), Some(uniform_buffer)) = (
                world.get_resource::<ecs_resources::SkinnedPipelineRes>(),
                world.get_resource::<ecs_resources::SkinnedMeshAssets>(),
                world.get_resource::<SkinnedMeshRenderDataRes>(),
                world.get_resource::<ecs_resources::UniformBuffer>(),
            ) {
                // Fox specific material or default material
                let fox_material = world.get_resource::<ecs_resources::FoxMaterialRes>();

                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Skinned Mesh Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: self.viewport_texture.render_target(),
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,  // Keep existing V-Buffer result
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.viewport_texture.depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                render_pass.set_pipeline(&skinned_pipeline.pipeline);

                // Render each skinned mesh instance
                for (mesh_index, model_matrix, skeleton_entity) in &skinned_instances {
                    // Check if mesh exists in assets
                    if *mesh_index >= skinned_assets.meshes.len() {
                        continue;
                    }

                    let gpu_data = &skinned_assets.meshes[*mesh_index];

                    // Update MVP uniform
                    let mvp = proj * view * *model_matrix;
                    let uniforms = Uniforms {
                        model_view_proj: mvp.to_cols_array_2d(),
                        model: model_matrix.to_cols_array_2d(),
                        view_pos: [camera_pos.x, camera_pos.y, camera_pos.z],
                        _padding: 0.0,
                    };
                    self.queue.write_buffer(&uniform_buffer.buffer, 0, bytemuck::cast_slice(&[uniforms]));

                    // Look up SkinnedMeshRenderer from skeleton_entity and use joint_bind_group
                    // Fall back to skinned_render_data if not found
                    if let Some(renderer) = world.get::<ecs_components::SkinnedMeshRenderer>(*skeleton_entity) {
                        render_pass.set_bind_group(0, &renderer.joint_bind_group, &[]);
                    } else {
                        render_pass.set_bind_group(0, &skinned_render_data.joint_bind_group, &[]);
                    }

                    // Use Fox material if exists, otherwise default material
                    if let Some(fox_mat) = fox_material {
                        render_pass.set_bind_group(1, &fox_mat.texture_bind_group, &[]);
                        render_pass.set_bind_group(2, &fox_mat.material_bind_group, &[]);
                    } else {
                        let default_material = &material_assets.materials[0];
                        render_pass.set_bind_group(1, &default_material.texture_bind_group, &[]);
                        render_pass.set_bind_group(2, &default_material.material_bind_group, &[]);
                    }

                    render_pass.set_vertex_buffer(0, gpu_data.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(gpu_data.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..gpu_data.num_indices, 0, 0..1);
                }

                // Skinned mesh rendering complete (debug logs removed)
            }
        }

        // ============ Game View rendering (ECS Camera) ============
        // Conditional rendering: only when camera exists (for potential game view tab)
        let should_render_game = game_camera.is_some();
        if let Some(game_cam) = should_render_game.then_some(()).and(game_camera.as_ref()) {
            // Create mesh render data for Game View
            let mut game_mesh_render_data: Vec<(
                wgpu::Buffer,     // camera buffer
                wgpu::Buffer,     // model buffer
                wgpu::BindGroup,  // camera bind group
                usize,            // mesh_idx
                usize,            // material_idx
                [[f32; 4]; 4],    // model_matrix
            )> = Vec::new();

            for (i, (mesh_idx, material_idx, world_transform)) in mesh_instances.iter().enumerate() {
                // Game Camera uniform
                let camera_uniform = renderer::CameraUniform::new(
                    game_cam.view,
                    game_cam.proj,
                    game_cam.position,
                    (self.game_viewport_texture.size.0, self.game_viewport_texture.size.1),
                    0.1,
                    100.0,
                );

                let camera_buffer = gpu_context.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("Game Camera Buffer {}", i)),
                    contents: bytemuck::cast_slice(&[camera_uniform]),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

                let model_uniform = renderer::ModelUniform::new(*world_transform);
                let model_buffer = gpu_context.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("Game Model Buffer {}", i)),
                    contents: bytemuck::cast_slice(&[model_uniform]),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

                let camera_bind_group = gpu_context.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(&format!("Game Camera Bind Group {}", i)),
                    layout: self.deferred_renderer.camera_bind_group_layout(),
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: camera_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: model_buffer.as_entire_binding(),
                        },
                    ],
                });

                let model_matrix = world_transform.to_cols_array_2d();
                game_mesh_render_data.push((camera_buffer, model_buffer, camera_bind_group, *mesh_idx, *material_idx, model_matrix));
            }

            // Build Game View render meshes
            let num_gltf_meshes = self.deferred_renderer.geometry_buffer
                .as_ref()
                .map(|g| g.mesh_infos.len())
                .unwrap_or(0);

            let game_render_meshes: Vec<renderer::MeshRenderData> = game_mesh_render_data
                .iter()
                .map(|(_, _, camera_bind_group, mesh_idx, material_idx, model_matrix)| {
                    let mesh_data = &mesh_assets.meshes[*mesh_idx];
                    // Standalone material handling (same as Scene View)
                    let material = if *material_idx < material_assets.materials.len() {
                        &material_assets.materials[*material_idx]
                    } else {
                        &material_assets.materials[0]
                    };
                    let geometry_mesh_idx = if *mesh_idx < num_gltf_meshes {
                        Some(*mesh_idx)
                    } else {
                        None
                    };

                    renderer::MeshRenderData {
                        vertex_buffer: &mesh_data.vertex_buffer,
                        index_buffer: &mesh_data.index_buffer,
                        index_count: mesh_data.num_indices,
                        camera_bind_group,
                        material_bind_group: material.deferred_bind_group.as_ref()
                            .unwrap_or(&material.material_bind_group),
                        geometry_mesh_idx,
                        material_index: *material_idx as u32,
                        model_matrix: *model_matrix,
                    }
                })
                .collect();

            // Update Lighting for Game View
            let game_sun_direction = glam::Vec3::new(-0.5, -1.0, -0.3).normalize();
            let game_sun_color = glam::Vec3::new(1.0, 1.0, 1.0);  // white light
            self.deferred_renderer.update_lighting(
                &self.queue,
                game_cam.view,
                game_cam.proj,
                game_cam.position,
                game_sun_direction,
                game_sun_color,
                3.0,
                1.0, 1000.0, 100.0, 0.05,
                0, // No debug mode for game view
            );

            // Sync debug UI screen-space effect settings to renderer (Game View)
            // Note: SSAO replaced by GTAO
            self.deferred_renderer.settings.enable_gtao = debug_ui.ssao_enabled;
            // TODO: Add SSR, Contact Shadows, Volumetric, SSS toggles to DebugUi

            // Render to game_viewport_texture
            self.deferred_renderer.render_vbuffer(
                &self.device,
                &mut encoder,
                self.game_viewport_texture.render_target(),
                &game_render_meshes,
                &self.queue,
                game_cam.view,
                game_cam.proj,
                game_sun_direction,
                game_sun_color,
            );

            // V-Buffer pipeline complete (debug logs removed)
        }

        // ============ Phase 18: Hair Rendering ============
        // Hair is rendered after deferred lighting as a forward pass with alpha blending
        {
            // Get elapsed time for hair animation
            let elapsed_time = world.get_resource::<ecs_resources::Time>()
                .map(|t| t.elapsed_seconds as f32)
                .unwrap_or(0.0);

            // Clone Arc'd device for this scope
            let device = gpu_context.device.clone();
            let _ = gpu_context;  // Release immutable borrow

            // Get HairRendererRes mutably
            if let Some(mut hair_res) = world.get_resource_mut::<ecs_resources::HairRendererRes>() {
                // Update time for hair animation
                hair_res.renderer.update_time(&self.queue, elapsed_time);

                // Update Card shader uniforms (camera, transform, light)
                let card_camera = hair::HairCameraUniform {
                    view: view.to_cols_array_2d(),
                    proj: proj.to_cols_array_2d(),
                    view_proj: (proj * view).to_cols_array_2d(),
                    camera_pos: [camera_pos.x, camera_pos.y, camera_pos.z],
                    _pad: 0.0,
                };
                hair_res.renderer.update_camera(&self.queue, card_camera);

                let card_transform = hair::HairModelTransform::default();
                hair_res.renderer.update_transform(&self.queue, card_transform);

                let card_light = hair::HairLightParams {
                    sun_direction: [sun_direction.x, sun_direction.y, sun_direction.z],
                    _pad0: 0.0,
                    sun_color: [1.0, 1.0, 1.0],
                    sun_intensity: 4.0,
                    ambient_color: [0.2, 0.2, 0.2],  // increased ambient
                    ambient_intensity: 1.0,
                };
                hair_res.renderer.update_light(&self.queue, card_light);

                // Create camera buffer for strand rendering (view, proj, view_proj, camera_pos)
                let view_proj = proj * view;
                #[repr(C)]
                #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
                struct HairCameraUniform {
                    view_proj: [[f32; 4]; 4],    // 64 bytes
                    view: [[f32; 4]; 4],         // 64 bytes
                    proj: [[f32; 4]; 4],         // 64 bytes
                    camera_pos: [f32; 3],        // 12 bytes
                    _pad: f32,                   // 4 bytes = total 208 bytes
                }
                let hair_camera = HairCameraUniform {
                    view_proj: view_proj.to_cols_array_2d(),
                    view: view.to_cols_array_2d(),
                    proj: proj.to_cols_array_2d(),
                    camera_pos: [camera_pos.x, camera_pos.y, camera_pos.z],
                    _pad: 0.0,
                };
                let hair_camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Hair Camera Buffer"),
                    contents: bytemuck::cast_slice(&[hair_camera]),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

                // Create strand bind group with camera buffer
                hair_res.renderer.create_strand_bind_group(&device, &hair_camera_buffer);

                // 1. Flyaway generation (compute pass)
                {
                    let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                        label: Some("Hair Flyaway Compute Pass"),
                        timestamp_writes: None,
                    });
                    hair_res.renderer.dispatch_flyaway_generation(&mut compute_pass);
                }

                // 2. Strand rendering (forward pass with alpha blending)
                {
                    let mut hair_render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Hair Render Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: self.viewport_texture.render_target(),
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,  // Keep existing content (deferred output)
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: self.viewport_texture.depth_target(),
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Load,  // Keep depth from deferred pass
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                    // Render strands (flyaway strands generated by compute shader)
                    hair_res.renderer.render_strands(&mut hair_render_pass);

                    // Render cards if any (currently none in test)
                    hair_res.renderer.render_cards(&mut hair_render_pass);
                }

                // Hair rendering complete (debug logs removed)
            }
        }

        // ============ Particle Rendering ============
        {
            let view_proj = proj * view;

            // Update all particle emitters
            let dt = world.get_resource::<ecs_resources::Time>()
                .map(|t| t.delta_seconds)
                .unwrap_or(1.0 / 60.0);
            let mut emitter_query = world.query::<(&ecs_components::Transform, &mut particles::ParticleEmitter)>();
            for (transform, mut emitter) in emitter_query.iter_mut(world) {
                emitter.update(dt, transform.translation);
            }

            // Collect emitters for rendering
            let mut emitter_query_ref = world.query::<&particles::ParticleEmitter>();
            let emitters: Vec<&particles::ParticleEmitter> = emitter_query_ref
                .iter(world)
                .collect();

            if !emitters.is_empty() {
                // Create particle camera uniform
                #[repr(C)]
                #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
                struct ParticleCameraUniform {
                    view_proj: [[f32; 4]; 4],
                    view: [[f32; 4]; 4],
                    camera_pos: [f32; 3],
                    _padding: f32,
                }

                let particle_camera = ParticleCameraUniform {
                    view_proj: view_proj.to_cols_array_2d(),
                    view: view.to_cols_array_2d(),
                    camera_pos: camera_pos.into(),
                    _padding: 0.0,
                };

                let particle_camera_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Particle Camera Buffer"),
                    contents: bytemuck::cast_slice(&[particle_camera]),
                    usage: wgpu::BufferUsages::UNIFORM,
                });

                let particle_camera_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Particle Camera Bind Group"),
                    layout: &self.deferred_renderer.resources.camera_bind_group_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: particle_camera_buffer.as_entire_binding(),
                    }],
                });

                // Render particles
                {
                    let mut particle_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Particle Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: self.viewport_texture.render_target(),
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: self.viewport_texture.depth_target(),
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                    let emitter_refs: Vec<&particles::ParticleEmitter> = emitters.to_vec();
                    self.particle_renderer.render(
                        &mut particle_pass,
                        &self.queue,
                        &particle_camera_bind_group,
                        &emitter_refs,
                    );
                }

                // Effect Renderer camera update (for Flipbook, VAT)
                self.effect_renderer.update_camera(
                    &self.queue,
                    view_proj.to_cols_array_2d(),
                    view.to_cols_array_2d(),
                    camera_pos.into(),
                );

                // ============ Effect Rendering (Flipbook, VAT, GPU Particle) ============
                if let Some(effect_render_data) = world.get_resource::<effects::EffectRenderData>() {
                    if effect_render_data.has_data() {
                        let mut effect_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Effect Render Pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: self.viewport_texture.render_target(),
                                depth_slice: None,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Load,
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                                view: self.viewport_texture.depth_target(),
                                depth_ops: Some(wgpu::Operations {
                                    load: wgpu::LoadOp::Load,
                                    store: wgpu::StoreOp::Store,
                                }),
                                stencil_ops: None,
                            }),
                            timestamp_writes: None,
                            occlusion_query_set: None,
                        });

                        self.effect_renderer.render_all(
                            &mut effect_pass,
                            &self.queue,
                            &particle_camera_bind_group,
                            effect_render_data,
                        );
                    }
                }

                // ============ Magic Circle Rendering (SDF) ============
                if let Some(mc_render_data) = world.get_resource::<magic::MagicCircleRenderData>() {
                    if mc_render_data.has_data() {
                        let mut mc_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Magic Circle Render Pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: self.viewport_texture.render_target(),
                                depth_slice: None,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Load,
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                                view: self.viewport_texture.depth_target(),
                                depth_ops: Some(wgpu::Operations {
                                    load: wgpu::LoadOp::Load,
                                    store: wgpu::StoreOp::Store,
                                }),
                                stencil_ops: None,
                            }),
                            timestamp_writes: None,
                            occlusion_query_set: None,
                        });

                        self.magic_circle_renderer.render(
                            &mut mc_pass,
                            &self.queue,
                            &self.device,
                            &particle_camera_bind_group,
                            mc_render_data,
                        );
                    }
                }
            }
        }

        // ============ Debug Draw Rendering ============
        {
            let view_proj = proj * view;

            // Pre-query data for debug visualization (avoid borrow conflicts)
            let selection_transforms: Vec<ecs_components::Transform> =
                if editor_debug_viz.show_selection_bounds {
                    if let Some(ref sv) = scene_viewer {
                        sv.selection.entities.iter()
                            .filter_map(|e| world.get::<ecs_components::Transform>(*e).cloned())
                            .collect()
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };

            let lights_data: Vec<(ecs_components::Transform, ecs_components::Light)> =
                if editor_debug_viz.show_lights {
                    world.query::<(&ecs_components::Transform, &ecs_components::Light)>()
                        .iter(world)
                        .map(|(t, l)| (t.clone(), l.clone()))
                        .collect()
                } else {
                    Vec::new()
                };

            let colliders_data: Vec<(ecs_components::Transform, physics::ColliderShape)> =
                if editor_debug_viz.show_colliders {
                    world.query::<(&ecs_components::Transform, &physics::ColliderComponent)>()
                        .iter(world)
                        .map(|(t, c)| (t.clone(), c.shape.clone()))
                        .collect()
                } else {
                    Vec::new()
                };

            // Get primitives from DebugDrawBuffer and render
            if let Some(mut debug_buffer) = world.get_resource_mut::<debug::DebugDrawBuffer>() {
                // ============ Editor Debug Visualization ============
                // Selection bounds visualization (always in Edit mode)
                if editor_debug_viz.show_selection_bounds && !selection_transforms.is_empty() {
                    editor::debug_viz::draw_selection_bounds(&selection_transforms, &mut debug_buffer);
                }

                // Light range visualization
                if editor_debug_viz.show_lights {
                    editor::debug_viz::draw_lights_debug(&lights_data, &mut debug_buffer);
                }

                // Collider visualization
                if editor_debug_viz.show_colliders {
                    editor::debug_viz::draw_colliders_debug(&colliders_data, &mut debug_buffer);
                }

                // Buffer update
                self.debug_draw_renderer.update(&self.queue, &debug_buffer, view_proj);

                // Render pass
                {
                    let mut debug_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Debug Draw Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: self.viewport_texture.render_target(),
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: self.viewport_texture.depth_target(),
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                    self.debug_draw_renderer.render(&mut debug_pass);
                }

                // Clear one-time primitives at frame end
                debug_buffer.clear_frame();
            }
        }

        // ============ Game UI Rendering ============
        {
            // Hot reload check
            let reload_events = ui_hot_reloader.check_and_reload(game_ui);
            for event in reload_events {
                match event {
                    ui::ReloadEvent::Reloaded { ref path } => {
                        log::info!("[UI] Hot reloaded: {:?}", path);
                    }
                    ui::ReloadEvent::Error { ref path, ref error } => {
                        log::info!("[UI] Reload error {:?}: {}", path, error);
                    }
                }
            }

            // UI system update
            let delta_seconds = world.get_resource::<ecs_resources::Time>()
                .map(|t| t.delta_seconds)
                .unwrap_or(0.016);

            // Set screen size
            game_ui.set_screen_size(self.size.width as f32, self.size.height as f32);

            // Data binding update - read actual game data from ECS
            {
                // Read health info from Player + Health components
                let mut player_query = world.query::<(&ecs_components::Player, &ecs_components::Health)>();
                if let Some((_, health)) = player_query.iter(world).next() {
                    game_ui.set_binding_value("player.health", ui::BindingValue::Number(health.current as f64));
                    game_ui.set_binding_value("player.max_health", ui::BindingValue::Number(health.maximum as f64));
                    game_ui.set_binding_value("player.health_percent", ui::BindingValue::Number((health.percentage() * 100.0) as f64));
                } else {
                    // Default values if no player
                    game_ui.set_binding_value("player.health", ui::BindingValue::Number(100.0));
                    game_ui.set_binding_value("player.max_health", ui::BindingValue::Number(100.0));
                    game_ui.set_binding_value("player.health_percent", ui::BindingValue::Number(100.0));
                }
                // Gold has no component yet - use default
                game_ui.set_binding_value("player.gold", ui::BindingValue::Number(0.0));
            }

            // UI update (animation, binding, input field cursor)
            game_ui.update(delta_seconds);
            game_ui.update_input_cursor_blink(delta_seconds);
            game_ui.calculate_layout();

            // ============ Lua UI API Integration ============
            if let Some(engine) = world.get_non_send_resource::<scripting::ScriptEngine>() {
                // 1. Widget registry sync (Rust -> Lua)
                if let Some(ref root) = game_ui.root {
                    let widget_info = collect_widget_info_for_lua(root);
                    if let Err(e) = scripting::sync_widget_registry(engine.lua(), &widget_info) {
                        log::warn!("[UI] Failed to sync widget registry: {}", e);
                    }
                }

                // 2. UI state sync (Rust -> Lua)
                let ui_state = scripting::UiState {
                    mouse_over_ui: game_ui.hovered_widget().is_some(),
                    hovered_widget: game_ui.hovered_widget().cloned(),
                    focused_widget: game_ui.focused_widget().cloned(),
                    is_dragging: game_ui.drag_state().is_some(),
                };
                if let Err(e) = scripting::sync_ui_state(engine.lua(), &ui_state) {
                    log::warn!("[UI] Failed to sync UI state: {}", e);
                }

                // 3. UI command processing (Lua -> Rust)
                match scripting::process_ui_commands(engine.lua()) {
                    Ok(commands) => {
                        for cmd in commands {
                            crate::app::game_ui_commands::process_ui_command(game_ui, &cmd);
                        }
                    }
                    Err(e) => {
                        log::warn!("[UI] Failed to process UI commands: {}", e);
                    }
                }

                // 4. UI event dispatch (Rust -> Lua)
                let events = game_ui.poll_events();
                for event in events {
                    let (event_type, widget_id, data, source_id) = match event {
                        ui::UiEvent::Click { widget_id } => ("click", widget_id, None, None),
                        ui::UiEvent::Hover { widget_id } => ("hover", widget_id, None, None),
                        ui::UiEvent::HoverEnd { widget_id } => ("hover_end", widget_id, None, None),
                        ui::UiEvent::Focus { widget_id } => ("focus", widget_id, None, None),
                        ui::UiEvent::Blur { widget_id } => ("blur", widget_id, None, None),
                        ui::UiEvent::ValueChanged { widget_id, value } => ("value_changed", widget_id, Some(value), None),
                        ui::UiEvent::Drop { source_widget_id, target_widget_id, data } => {
                            ("drop", target_widget_id, data, Some(source_widget_id))
                        }
                        _ => continue,
                    };
                    if let Err(e) = scripting::dispatch_ui_event(
                        engine.lua(),
                        event_type,
                        &widget_id,
                        data.as_deref(),
                        source_id.as_deref(),
                    ) {
                        log::warn!("[UI] Failed to dispatch event {}: {}", event_type, e);
                    }
                }
            }

            // UI rendering (drag ghost + tooltip included) - Play mode only
            // magic_builder is Some only in play mode
            // Surface가 있을 때만 swapchain에 Game UI 렌더링
            if let Some(ref tv) = texture_view {
                if magic_builder.is_some() {
                    if let Some(ref root) = game_ui.root {
                        let drag_info = game_ui.get_drag_info();
                        let tooltip_info = game_ui.get_tooltip_info();
                        self.ui_renderer.render_with_overlays(&self.device, &mut encoder, tv, &self.queue, root, drag_info.as_ref(), tooltip_info);
                    }
                }

                // Magic Builder overlay rendering (Play mode only)
                if let Some(builder) = magic_builder {
                    if builder.visible {
                        let screen_w = self.size.width as f32;
                        let screen_h = self.size.height as f32;
                        builder.calculate_layout(screen_w, screen_h);
                        self.ui_renderer.render(&self.device, &mut encoder, tv, &self.queue, builder.root());
                    }
                }
            }
        }

        // ============ Editor UI State Updates ============
        {
            // Update debug UI stats
            let (delta_seconds, elapsed_seconds) = world.get_resource::<ecs_resources::Time>()
                .map(|t| (t.delta_seconds, t.elapsed_seconds))
                .unwrap_or((0.016, 0.0));
            debug_ui.update_stats(delta_seconds);
            debug_ui.elapsed_time = elapsed_seconds;

            // Update camera info in debug UI
            debug_ui.camera_pos = camera_pos;
            if let Some(ref sv) = scene_viewer {
                debug_ui.camera_yaw = sv.camera.yaw();
                debug_ui.camera_pitch = sv.camera.pitch();
            }

            // Update entity list (every 60 frames)
            unsafe {
                if FRAME_COUNT.is_multiple_of(60) || debug_ui.entities.is_empty() {
                    debug_ui.entities = debug::ui::collect_entity_info(world);
                }
            }

            // ============ Dock Layout UI (Unreal/Unity style layout) ============

            // Selected entity for Inspector
            let _selected_entity: Option<bevy_ecs::entity::Entity> = scene_viewer
                .as_ref()
                .and_then(|sv| sv.selection.entities.first().copied());

            // Inspector action tracking
            let inspector_action = editor::InspectorAction::None;

            // Hierarchy action tracking
            let hierarchy_action = editor::HierarchyAction::None;
            let asset_browser_action = editor::AssetBrowserAction::None;
            let hierarchy_state = &mut self.hierarchy_state;
            let ai_panel_state = &mut self.ai_panel_state;
            let asset_browser_state = &mut self.asset_browser_state;
            let inspector_state = &mut self.inspector_state;
            let ui_editor_state = &mut self.ui_editor_state;
            let animation_timeline_state = &mut self.animation_timeline_state;
            let magic_system_editor_state = &mut self.magic_system_editor_state;

            // ============ Scene Viewer rendering (Grid + Gizmo) ============
            let show_grid = true; // TODO: Make configurable via UI
            if let Some(ref mut viewer) = scene_viewer {
                viewer.render_overlay(
                    &self.device,
                    &self.queue,
                    &mut encoder,
                    self.viewport_texture.render_target(),
                    self.viewport_texture.depth_target(),
                    show_grid,
                );
            }

            // Suppress unused variable warnings
            let _ = hierarchy_state;
            let _ = ai_panel_state;
            let _ = asset_browser_state;
            let _ = inspector_state;
            let _ = ui_editor_state;
            let _ = animation_timeline_state;
            let _ = magic_system_editor_state;

            // Inspector action handling
            match inspector_action {
                editor::InspectorAction::RenameEntity(entity, new_name) => {
                    if let Some(mut name) = world.get_mut::<ecs_components::NodeName>(entity) {
                        name.0 = new_name;
                    }
                }
                editor::InspectorAction::TransformChanged { entity, position, rotation, scale } => {
                    // Update Transform component
                    if let Some(mut transform) = world.get_mut::<ecs_components::Transform>(entity) {
                        transform.translation = position;
                        transform.rotation = rotation;
                        transform.scale = scale;
                    }

                    // Update Gizmo
                    if let Some(ref mut sv) = scene_viewer {
                        sv.update_gizmo_from_selection(world);
                    }
                }
                editor::InspectorAction::CameraChanged { entity, fov, near, far } => {
                    if let Some(mut camera) = world.get_mut::<ecs_components::Camera>(entity) {
                        camera.fov = fov;
                        camera.near = near;
                        camera.far = far;
                    }
                }
                editor::InspectorAction::LightChanged { entity, color, intensity, range, spot_angle, cast_shadows } => {
                    if let Some(mut light) = world.get_mut::<ecs_components::Light>(entity) {
                        light.intensity = intensity;
                        light.color = color;
                        light.range = range;
                        light.spot_angle = spot_angle;
                        light.cast_shadows = cast_shadows;
                    }
                }
                editor::InspectorAction::BoxColliderChanged { entity, half_extents, offset } => {
                    if let Some(mut collider) = world.get_mut::<ecs_components::BoxCollider>(entity) {
                        collider.half_extents = half_extents;
                        collider.offset = offset;
                    }
                }
                editor::InspectorAction::SphereColliderChanged { entity, radius, offset } => {
                    if let Some(mut collider) = world.get_mut::<ecs_components::SphereCollider>(entity) {
                        collider.radius = radius;
                        collider.offset = offset;
                    }
                }
                editor::InspectorAction::MaterialChanged { material_index, base_color, metallic, roughness, emissive_strength, normal_scale } => {
                    if let Some(mut registry) = world.get_resource_mut::<crate::material::MaterialRegistry>() {
                        if let Some(entry) = registry.get_by_index_mut(material_index) {
                            entry.def.base_color = base_color;
                            entry.def.metallic = metallic;
                            entry.def.roughness = roughness;
                            entry.def.emissive_strength = emissive_strength;
                            entry.def.normal_scale = normal_scale;
                        }
                    }
                }
                editor::InspectorAction::SaveMaterial(material_index) => {
                    if let Some(registry) = world.get_resource::<crate::material::MaterialRegistry>() {
                        if let Some(entry) = registry.get_by_index(material_index) {
                            if let Err(e) = entry.save() {
                                log::error!("[Inspector] Failed to save material: {}", e);
                            } else {
                                log::info!("[Inspector] Material saved: index {}", material_index);
                            }
                        }
                    }
                }
                editor::InspectorAction::RemoveComponent(entity, component_name) => {
                    log::info!("[Inspector] Remove component '{}' from {:?}", component_name, entity);
                    // TODO: Implement component removal
                }
                editor::InspectorAction::None => {}
            }

            // Hierarchy action handling
            match hierarchy_action {
                editor::HierarchyAction::Select(entity) => {
                    // Select entity
                    self.hierarchy_state.select(entity);
                    if let Some(ref mut sv) = scene_viewer {
                        sv.selection.entities = vec![entity];
                        sv.update_gizmo_from_selection(world);
                    }
                    log::debug!("[Hierarchy] Selected entity: {:?}", entity);
                }
                editor::HierarchyAction::Focus(entity) => {
                    // Move camera to entity
                    if let Some(transform) = world.get::<ecs_components::Transform>(entity) {
                        if let Some(ref mut sv) = scene_viewer {
                            // Use object scale as approximate size, or default to 2.0
                            let size = transform.scale.max_element().max(2.0);
                            sv.camera.focus_on(transform.translation, size);
                        }
                    }
                    log::info!("[Hierarchy] Focus on entity: {:?}", entity);
                }
                editor::HierarchyAction::CreateChild(parent) => {
                    // Create child entity
                    let child = world.spawn((
                        ecs_components::NodeName("New Entity".to_string()),
                        ecs_components::Transform::default(),
                    )).id();
                    if let Ok(mut parent_mut) = world.get_entity_mut(parent) {
                        parent_mut.add_child(child);
                    }
                    log::info!("[Hierarchy] Created child {:?} under {:?}", child, parent);
                }
                editor::HierarchyAction::Duplicate(entity) => {
                    // Duplicate entity
                    let name = world.get::<ecs_components::NodeName>(entity)
                        .map(|n| format!("{} (Copy)", n.0))
                        .unwrap_or_else(|| "Duplicated Entity".to_string());
                    let transform = world.get::<ecs_components::Transform>(entity)
                        .cloned()
                        .unwrap_or_default();
                    world.spawn((
                        ecs_components::NodeName(name),
                        transform,
                    ));
                    log::info!("[Hierarchy] Duplicated entity: {:?}", entity);
                }
                editor::HierarchyAction::Delete(entity) => {
                    // Delete entity
                    world.despawn(entity);
                    self.hierarchy_state.selected.remove(&entity);
                    if let Some(ref mut sv) = scene_viewer {
                        sv.selection.entities.retain(|&e| e != entity);
                    }
                    log::info!("[Hierarchy] Deleted entity: {:?}", entity);
                }
                editor::HierarchyAction::Reparent(entity, new_parent) => {
                    // Remove from current parent
                    if let Some(current_parent) = world.get::<bevy_hierarchy::Parent>(entity).map(|p| p.get()) {
                        if let Ok(mut parent_mut) = world.get_entity_mut(current_parent) {
                            parent_mut.remove_children(&[entity]);
                        }
                    }
                    // Add to new parent
                    if let Some(new_parent_entity) = new_parent {
                        if let Ok(mut parent_mut) = world.get_entity_mut(new_parent_entity) {
                            parent_mut.add_child(entity);
                        }
                    }
                    log::info!("[Hierarchy] Reparented {:?} to {:?}", entity, new_parent);
                }
                editor::HierarchyAction::ToggleVisibility(entity) => {
                    // Toggle Hidden component
                    if world.get::<ecs_components::Hidden>(entity).is_some() {
                        if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
                            entity_mut.remove::<ecs_components::Hidden>();
                        }
                    } else {
                        if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
                            entity_mut.insert(ecs_components::Hidden);
                        }
                    }
                    log::debug!("[Hierarchy] Toggled visibility for {:?}", entity);
                }
                editor::HierarchyAction::TogglePickable(entity) => {
                    // Toggle NotPickable component
                    if world.get::<ecs_components::NotPickable>(entity).is_some() {
                        if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
                            entity_mut.remove::<ecs_components::NotPickable>();
                        }
                    } else {
                        if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
                            entity_mut.insert(ecs_components::NotPickable);
                        }
                    }
                    log::debug!("[Hierarchy] Toggled pickable for {:?}", entity);
                }
                editor::HierarchyAction::CreateEmpty => {
                    let entity = world.spawn((
                        ecs_components::NodeName("Empty".to_string()),
                        ecs_components::Transform::default(),
                        ecs_components::GlobalTransform::default(),
                    )).id();
                    self.hierarchy_state.select(entity);
                    log::info!("[Hierarchy] Created empty entity: {:?}", entity);
                }
                editor::HierarchyAction::Create3DObject(obj_type) => {
                    let name = obj_type.clone();
                    let entity = world.spawn((
                        ecs_components::NodeName(name),
                        ecs_components::Transform::default(),
                        ecs_components::GlobalTransform::default(),
                        // TODO: Add actual mesh based on obj_type (Cube, Sphere, etc.)
                    )).id();
                    self.hierarchy_state.select(entity);
                    log::info!("[Hierarchy] Created 3D object '{}': {:?}", obj_type, entity);
                }
                editor::HierarchyAction::CreateLight(light_type) => {
                    let lt = match light_type.as_str() {
                        "Point" => ecs_components::LightType::Point,
                        "Directional" | "Sun" => ecs_components::LightType::Sun,
                        "Spot" => ecs_components::LightType::Spot,
                        _ => ecs_components::LightType::Point,
                    };
                    let entity = world.spawn((
                        ecs_components::NodeName(format!("{} Light", light_type)),
                        ecs_components::Transform::default(),
                        ecs_components::GlobalTransform::default(),
                        ecs_components::Light {
                            light_type: lt,
                            color: glam::Vec3::ONE,
                            intensity: 1.0,
                            range: 10.0,
                            spot_angle: 45.0f32.to_radians(),
                            cast_shadows: true,
                        },
                    )).id();
                    self.hierarchy_state.select(entity);
                    log::info!("[Hierarchy] Created light '{}': {:?}", light_type, entity);
                }
                editor::HierarchyAction::CreateCamera => {
                    let entity = world.spawn((
                        ecs_components::NodeName("Camera".to_string()),
                        ecs_components::Transform::default(),
                        ecs_components::GlobalTransform::default(),
                        ecs_components::Camera {
                            fov: 60.0f32.to_radians(),
                            near: 0.1,
                            far: 1000.0,
                            is_active: false,
                        },
                    )).id();
                    self.hierarchy_state.select(entity);
                    log::info!("[Hierarchy] Created camera: {:?}", entity);
                }
                editor::HierarchyAction::None => {}
            }

            // ========== Menu action handling ==========
            // TODO: Implement menu action handling via skope_ui
            // Menu actions (CreateEmpty, Create3DObject, CreateLight, etc.) will be triggered
            // from menus and handled here when reimplemented

            // ========== Drag and drop handling ==========
            // TODO: Implement drag-and-drop asset spawning
            // Currently disabled - will be reimplemented with skope_ui drag/drop
            if false {
                let asset_path = String::new();
                let screen_pos = glam::Vec2::ZERO;
                log::info!("[Drop] Processing dropped asset: {} at {:?}", asset_path, screen_pos);

                // Convert screen coordinates to world coordinates
                // TODO: Reimplement with skope_ui viewport info
                let spawn_position = if let Some(ref sv) = scene_viewer {
                    // Spawn 5m in front of camera
                    let cam = &sv.camera;
                    let _ = screen_pos; // Unused for now
                    cam.position + cam.forward() * 5.0
                } else {
                    // Spawn at origin if no camera
                    glam::Vec3::ZERO
                };

                // Load GLTF model and register for rendering
                let asset_path_obj = std::path::Path::new(&asset_path);

                // Check if skinned mesh
                let is_skinned = assets::has_skinned_meshes(asset_path_obj);
                log::info!("[Drop] Asset type: {} (skinned: {})", asset_path, is_skinned);

                if is_skinned {
                    // ========== Skinned mesh loading path ==========
                    log::info!("[Drop] Loading skinned model: {}", asset_path);

                    // Get required resources
                    let skinned_pipeline = world.get_resource::<ecs_resources::SkinnedPipelineRes>();
                    let render_pipeline = world.get_resource::<ecs_resources::RenderPipelineRes>();
                    let uniform_buffer = world.get_resource::<ecs_resources::UniformBuffer>();

                    if let (Some(skinned_pipe), Some(render_pipe), Some(uniform_buf)) =
                        (skinned_pipeline, render_pipeline, uniform_buffer)
                    {
                        // Extract layout references (avoid borrow conflicts)
                        let texture_layout = &render_pipe.texture_bind_group_layout as *const _;
                        let material_layout = &render_pipe.material_bind_group_layout as *const _;
                        let skinned_layout = &skinned_pipe.skinned_uniform_bind_group_layout as *const _;
                        let uniform_buf_ref = &uniform_buf.buffer as *const _;

                        // Create SkinnedLoadContext
                        let ctx = assets::SkinnedLoadContext {
                            device: &self.device,
                            queue: &self.queue,
                            texture_bind_group_layout: unsafe { &*texture_layout },
                            material_bind_group_layout: unsafe { &*material_layout },
                            skinned_uniform_layout: unsafe { &*skinned_layout },
                            uniform_buffer: unsafe { &*uniform_buf_ref },
                        };

                        // Get resources
                        let mut skinned_mesh_assets = world.remove_resource::<ecs_resources::SkinnedMeshAssets>()
                            .unwrap_or_default();
                        let mut skin_assets = world.remove_resource::<ecs_resources::SkinAssets>()
                            .unwrap_or_default();
                        let mut skinned_model_registry = world.remove_resource::<ecs_resources::SkinnedModelRegistry>()
                            .unwrap_or_default();

                        // Load skinned model
                        match assets::load_skinned_model(
                            asset_path_obj,
                            &ctx,
                            &mut skinned_mesh_assets,
                            &mut skin_assets,
                            &mut skinned_model_registry,
                        ) {
                            Ok(model_name) => {
                                log::info!("[Drop] Skinned model '{}' loaded successfully", model_name);

                                // Restore resources
                                world.insert_resource(skinned_mesh_assets);
                                world.insert_resource(skin_assets);
                                world.insert_resource(skinned_model_registry);

                                // Spawn skinned model
                                if let Some(root_entity) = assets::spawn_skinned_model(
                                    world,
                                    &model_name,
                                    spawn_position,
                                    0.01,  // default scale (glTF models usually small)
                                    &ctx,
                                ) {
                                    log::info!("[Drop] Spawned skinned model '{}' at {:?}", model_name, spawn_position);

                                    // Select
                                    self.hierarchy_state.selected.clear();
                                    self.hierarchy_state.selected.insert(root_entity);
                                    if let Some(ref mut sv) = scene_viewer {
                                        sv.selection.entities = vec![root_entity];
                                        sv.update_gizmo_from_selection(world);
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!("[Drop] Failed to load skinned model: {}", e);
                                world.insert_resource(skinned_mesh_assets);
                                world.insert_resource(skin_assets);
                                world.insert_resource(skinned_model_registry);
                            }
                        }
                    } else {
                        log::warn!("[Drop] Required pipelines not initialized for skinned mesh loading");
                    }
                } else {
                    // ========== Static mesh loading path (existing code) ==========
                    // 1. Create GPU buffers and register in MeshAssets
                    let mut mesh_assets = world.remove_resource::<ecs_resources::MeshAssets>()
                        .unwrap_or_default();
                    let mut material_assets = world.remove_resource::<ecs_resources::MaterialAssets>()
                        .unwrap_or_default();

                    match assets::load_gltf_to_assets(
                        asset_path_obj,
                        &self.device,
                        &self.queue,
                        &mut mesh_assets,
                        &mut material_assets,
                    ) {
                        Ok(mesh_count) => {
                            log::info!("[Drop] Loaded {} new meshes to GPU", mesh_count);

                            // 2. Reload GLTF model to create ECS entities
                            if let Ok(model) = gltf_loader::load_gltf(&asset_path) {
                                // Calculate mesh_start_index:
                                // - If new meshes registered: calculate from end
                                // - If only existing meshes: find existing index by name
                                let mesh_start_index = if mesh_count > 0 {
                                    // Newly registered meshes - calculate from end
                                    mesh_assets.meshes.len() - mesh_count
                                } else if !model.meshes.is_empty() {
                                    // Already registered meshes - find by name
                                    let file_stem = asset_path_obj.file_stem()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or("unknown");
                                    // Single mesh: "filename", multiple meshes: "filename_0"
                                    let first_mesh_name = if model.meshes.len() == 1 {
                                        file_stem.to_string()
                                    } else {
                                        format!("{}_0", file_stem)
                                    };
                                    mesh_assets.get_index(&first_mesh_name).unwrap_or(0)
                                } else {
                                    0
                                };

                                log::info!("[Drop] mesh_start_index = {} (mesh_count = {}, model.meshes.len = {})",
                                    mesh_start_index, mesh_count, model.meshes.len());

                                world.insert_resource(mesh_assets);
                                world.insert_resource(material_assets);

                                // Create entities via gltf_to_ecs (with MeshInstance, MaterialHandle)
                                // mesh_start_index offset for correct GPU buffer reference
                                let root_entities = assets::spawn_gltf_model_with_offset(
                                    world,
                                    &model,
                                    mesh_start_index,
                                );

                                // 3. Apply spawn position to root entities
                                for &root_entity in &root_entities {
                                    if let Some(mut transform) = world.get_mut::<ecs_components::Transform>(root_entity) {
                                        transform.translation = spawn_position;
                                    }
                                }

                                log::info!("[Drop] Spawned {} root entities at {:?} (mesh offset: {})",
                                    root_entities.len(), spawn_position, mesh_start_index);

                                // 4. Select first root entity
                                if let Some(&first_root) = root_entities.first() {
                                    self.hierarchy_state.selected.clear();
                                    self.hierarchy_state.selected.insert(first_root);
                                    if let Some(ref mut sv) = scene_viewer {
                                        sv.selection.entities = vec![first_root];
                                        sv.update_gizmo_from_selection(world);
                                    }
                                }
                            } else {
                                world.insert_resource(mesh_assets);
                                world.insert_resource(material_assets);
                                log::warn!("[Drop] Failed to reload GLTF for entity spawn: {}", asset_path);
                            }
                        }
                        Err(e) => {
                            world.insert_resource(mesh_assets);
                            world.insert_resource(material_assets);
                            log::warn!("[Drop] Failed to load GLTF to assets: {} - {:?}", asset_path, e);
                        }
                    }
                }
            }

            // ========== Asset Browser action handling ==========
            match asset_browser_action {
                editor::AssetBrowserAction::OpenFile(path) => {
                    // Open .ui.ron files in UI Editor
                    if path.to_string_lossy().ends_with(".ui.ron") {
                        self.ui_editor_windows.open(path);
                    } else {
                        log::info!("[AssetBrowser] Open file: {:?}", path);
                        // TODO: handle other file types
                    }
                }
                editor::AssetBrowserAction::CreateUiLayout => {
                    // Create new UI file in current directory
                    let current_dir = self.asset_browser_state.current_dir.clone();
                    if let Some(path) = self.ui_editor_windows.create_new_in_dir(&current_dir) {
                        log::info!("[AssetBrowser] Created UI layout: {:?}", path);
                    }
                }
                editor::AssetBrowserAction::CreateFolder => {
                    // Create new folder
                    let current_dir = &self.asset_browser_state.current_dir;
                    let new_folder = current_dir.join("New Folder");
                    if !new_folder.exists() {
                        if let Err(e) = std::fs::create_dir(&new_folder) {
                            log::error!("[AssetBrowser] Failed to create folder: {}", e);
                        } else {
                            log::info!("[AssetBrowser] Created folder: {:?}", new_folder);
                        }
                    }
                }
                editor::AssetBrowserAction::NavigateTo(_) => {
                    // Already handled in AssetBrowserState
                }
                editor::AssetBrowserAction::LoadScene(path) => {
                    // Load scene
                    let path_str = path.to_string_lossy().to_string();
                    log::info!("[AssetBrowser] Loading scene: {}", path_str);

                    match skope_data::Scene::from_file(&path_str) {
                        Ok(scene) => {
                            // Delete existing scene entities (except camera)
                            let to_despawn: Vec<bevy_ecs::entity::Entity> = {
                                let mut query = world.query::<(
                                    bevy_ecs::entity::Entity,
                                    Option<&ecs_components::NodeName>,
                                )>();
                                query.iter(world)
                                    .filter(|(_, name)| {
                                        name.as_ref().map(|n| n.0 != "Camera").unwrap_or(true)
                                    })
                                    .map(|(e, _)| e)
                                    .collect()
                            };

                            for entity in to_despawn {
                                world.despawn(entity);
                            }

                            // Spawn new scene
                            let spawned = scene.spawn_all(world);
                            skope_data::process_pending_colliders(world);

                            log::info!("[AssetBrowser] Loaded scene: {} ({} entities)", path_str, spawned.len());
                        }
                        Err(e) => {
                            log::error!("[AssetBrowser] Failed to load scene '{}': {}", path_str, e);
                        }
                    }
                }
                editor::AssetBrowserAction::SpawnAsset { asset_path, asset_type } => {
                    // Spawn entity from asset
                    use editor::AssetType;
                    let name = asset_path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("Asset")
                        .to_string();

                    match asset_type {
                        AssetType::Mesh => {
                            // TODO: Load actual mesh from asset_path
                            let entity = world.spawn((
                                ecs_components::NodeName(name),
                                ecs_components::Transform::default(),
                                ecs_components::GlobalTransform::default(),
                            )).id();
                            log::info!("[AssetBrowser] Spawned mesh from {:?}: {:?}", asset_path, entity);
                        }
                        AssetType::Prefab => {
                            // TODO: Load prefab from asset_path
                            log::info!("[AssetBrowser] Prefab spawn not yet implemented: {:?}", asset_path);
                        }
                        _ => {
                            log::warn!("[AssetBrowser] Cannot spawn asset type {:?} into scene", asset_type);
                        }
                    }
                }
                editor::AssetBrowserAction::ApplyToEntity { asset_path, asset_type } => {
                    // Apply asset to selected entity (e.g., material)
                    use editor::AssetType;
                    if let Some(ref sv) = scene_viewer {
                        if let Some(entity) = sv.selection.entities.first() {
                            match asset_type {
                                AssetType::Material => {
                                    // TODO: Apply material to entity
                                    log::info!("[AssetBrowser] Apply material {:?} to {:?}", asset_path, entity);
                                }
                                _ => {
                                    log::warn!("[AssetBrowser] Cannot apply asset type {:?} to entity", asset_type);
                                }
                            }
                        }
                    }
                }
                editor::AssetBrowserAction::None => {}
            }

            // Suppress unused variable warnings
            let _ = debug_ui;
            let _ = game_ui;
            let _ = ui_hot_reloader;

            // Handle console actions
            if let Some(action) = debug_ui.take_action() {
                match action {
                    debug::ui::ConsoleAction::ReloadScene => {
                        // Phase 3: scene reload implementation
                        let default_level = format!("{}/start.skope", paths::game::LEVELS);
                        let level_path = std::env::var("SKOPE_LEVEL")
                            .unwrap_or(default_level);

                        // 1. Collect existing scene entities (except camera)
                        let to_despawn: Vec<bevy_ecs::entity::Entity> = {
                            let mut query = world.query::<bevy_ecs::entity::Entity>();
                            query.iter(world)
                                .filter(|e| {
                                    // Keep entities with camera
                                    world.get::<ecs_components::Camera>(*e).is_none()
                                })
                                .collect()
                        };

                        // 2. Remove entities
                        let despawn_count = to_despawn.len();
                        for entity in to_despawn {
                            world.despawn(entity);
                        }

                        // 3. Load new scene
                        match skope_data::Scene::from_file(&level_path) {
                            Ok(scene) => {
                                let spawned = scene.spawn_all(world);
                                skope_data::process_pending_colliders(world);

                                debug_ui.log(
                                    debug::ui::LogLevel::Info,
                                    &format!("Reloaded scene: removed {} entities, spawned {}", despawn_count, spawned.len()),
                                    debug_ui.elapsed_time
                                );

                                // Refresh entity list
                                debug_ui.entities = debug::ui::collect_entity_info(world);
                            }
                            Err(e) => {
                                debug_ui.log(
                                    debug::ui::LogLevel::Error,
                                    &format!("Failed to reload scene: {}", e),
                                    debug_ui.elapsed_time
                                );
                            }
                        }
                    }
                    debug::ui::ConsoleAction::ExecuteLua(code) => {
                        if let Some(engine) = world.get_non_send_resource::<scripting::ScriptEngine>() {
                            match engine.exec(&code) {
                                Ok(result) => {
                                    if !result.is_empty() {
                                        debug_ui.log(debug::ui::LogLevel::Info, &result, debug_ui.elapsed_time);
                                    } else {
                                        debug_ui.log(debug::ui::LogLevel::Info, "OK", debug_ui.elapsed_time);
                                    }
                                }
                                Err(e) => {
                                    debug_ui.log(debug::ui::LogLevel::Error, &format!("Lua error: {}", e), debug_ui.elapsed_time);
                                }
                            }
                        } else {
                            debug_ui.log(debug::ui::LogLevel::Error, "Lua engine not available", debug_ui.elapsed_time);
                        }
                    }
                    debug::ui::ConsoleAction::SpawnEntity(name) => {
                        // Try to get prefab data first (clone to avoid borrow conflict)
                        let prefab_data = world
                            .get_resource::<prefab::PrefabRegistry>()
                            .and_then(|registry| registry.get(&name).cloned());

                        if let Some(data) = prefab_data {
                            // Spawn from prefab
                            let entity = prefab::spawn_prefab_entity(world, &data.root, glam::Vec3::ZERO);
                            debug_ui.log(
                                debug::ui::LogLevel::Info,
                                &format!("Spawned prefab '{}' (ID: {})", name, entity.to_bits() & 0xFFFF),
                                debug_ui.elapsed_time
                            );
                        } else {
                            // Spawn basic entity with transform
                            let entity = world.spawn((
                                ecs_components::Transform::from_translation(glam::Vec3::ZERO),
                                ecs_components::NodeName(name.clone()),
                            )).id();
                            debug_ui.log(
                                debug::ui::LogLevel::Info,
                                &format!("Spawned entity '{}' (ID: {})", name, entity.to_bits() & 0xFFFF),
                                debug_ui.elapsed_time
                            );
                        }
                        // Refresh entity list
                        debug_ui.entities = debug::ui::collect_entity_info(world);
                    }
                    debug::ui::ConsoleAction::SpawnParticle(effect_type) => {
                        // Spawn particle emitter entity in front of camera
                        // Get forward direction from view matrix (third column negated)
                        let forward = -glam::Vec3::new(view.col(2).x, view.col(2).y, view.col(2).z);
                        let spawn_pos = camera_pos + forward * 3.0; // 3m in front of camera
                        let emitter = match effect_type.as_str() {
                            "fire" => particles::ParticleEmitter::fire(),
                            "smoke" => particles::ParticleEmitter::smoke(),
                            "explosion" => particles::ParticleEmitter::explosion(),
                            "sparkle" => particles::ParticleEmitter::sparkle(),
                            _ => particles::ParticleEmitter::fire(),
                        };
                        let entity = world.spawn((
                            ecs_components::Transform::from_translation(spawn_pos),
                            ecs_components::NodeName(format!("Particle_{}", effect_type)),
                            emitter,
                        )).id();
                        debug_ui.log(
                            debug::ui::LogLevel::Info,
                            &format!("Spawned {} particles at {:?} (ID: {})", effect_type, spawn_pos, entity.to_bits() & 0xFFFF),
                            debug_ui.elapsed_time
                        );
                        debug_ui.entities = debug::ui::collect_entity_info(world);
                    }
                }
            }
        }

        // Surface가 있을 때만 swapchain clear + UI render + present
        if let (Some(output), Some(ref tv)) = (output, &texture_view) {
            // ============ Swapchain Clear (for debugging - ensures full screen coverage) ============
            {
                let _ = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Swapchain Clear Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: tv,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.1, g: 0.1, b: 0.1, a: 1.0 }),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
            }

            // ============ skope_ui Render ============
            let (ui_elapsed, ui_delta) = world.get_resource::<ecs_resources::Time>()
                .map(|t| (t.elapsed_seconds, t.delta_seconds as f32))
                .unwrap_or((0.0, delta_time));
            self.slate_ui_render(&mut encoder, tv, ui_elapsed, ui_delta);

            // 메인 encoder 제출
            self.queue.submit(std::iter::once(encoder.finish()));

            output.present();
            log::trace!("[Render] Frame complete");
        } else {
            // Headless 모드: viewport texture만 렌더링, present 없음
            self.queue.submit(std::iter::once(encoder.finish()));
            log::trace!("[Render] Viewport-only frame complete");
        }

        Ok(())
    }

}

// ============ UI Lua API Helper Functions ============

/// Convert UI widget tree to info for Lua sync
fn collect_widget_info_for_lua(root: &ui::Widget) -> Vec<(String, scripting::WidgetInfo)> {
    let mut result = Vec::new();
    collect_widget_info_recursive(root, &mut result);
    result
}

fn collect_widget_info_recursive(widget: &ui::Widget, result: &mut Vec<(String, scripting::WidgetInfo)>) {
    if let Some(ref id) = widget.id {
        let info = scripting::WidgetInfo {
            visible: widget.visible,
            x: widget.computed_rect.x,
            y: widget.computed_rect.y,
            width: widget.computed_rect.width,
            height: widget.computed_rect.height,
            text: match &widget.widget_type {
                ui::WidgetType::Text { content, .. } => Some(content.clone()),
                ui::WidgetType::Button { text, .. } => text.clone(),
                _ => None,
            },
            progress: match &widget.widget_type {
                ui::WidgetType::ProgressBar { value, max_value, .. } => Some((*value, *max_value)),
                _ => None,
            },
            input_value: match &widget.widget_type {
                ui::WidgetType::InputField { value, .. } => Some(value.clone()),
                _ => None,
            },
        };
        result.push((id.clone(), info));
    }

    // Recursively process child widgets
    for child in &widget.children {
        collect_widget_info_recursive(child, result);
    }
}

// UI command processing moved to game_ui_commands.rs
