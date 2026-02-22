//! State render function
//!
//! Main rendering logic extracted from state.rs

#![allow(clippy::type_complexity)]

use std::collections::HashMap;
use wgpu::util::DeviceExt;
use skope_ecs::prelude::*;
use super::State;

// ---------------------------------------------------------------------------
// Persistent per-view GPU buffer pool (avoids per-frame buffer allocation)
// ---------------------------------------------------------------------------

/// Reusable GPU buffer pool for one camera view.
///
/// Instead of calling `create_buffer_init` N times per frame, this pool
/// keeps persistent buffers and updates them via `queue.write_buffer()`.
pub struct PerViewBufferPool {
    /// Single camera uniform buffer (shared by all instances in a view)
    camera_buffer: wgpu::Buffer,
    /// Per-instance model uniform buffers
    model_buffers: Vec<wgpu::Buffer>,
    /// Per-instance bind groups (binding 0 = camera, binding 1 = model)
    bind_groups: Vec<wgpu::BindGroup>,
    /// Current pool capacity (number of instance slots)
    capacity: usize,
}

impl PerViewBufferPool {
    pub fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("PerView Camera Buffer"),
            size: std::mem::size_of::<renderer::CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let initial_capacity = 64;
        let (model_buffers, bind_groups) = Self::create_slots(
            device, layout, &camera_buffer, initial_capacity,
        );

        Self {
            camera_buffer,
            model_buffers,
            bind_groups,
            capacity: initial_capacity,
        }
    }

    /// Ensure pool has at least `count` instance slots. Grows if needed.
    pub fn ensure_capacity(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        count: usize,
    ) {
        if count <= self.capacity {
            return;
        }
        // Grow to next power of 2
        let new_cap = count.next_power_of_two();
        let (model_buffers, bind_groups) = Self::create_slots(
            device, layout, &self.camera_buffer, new_cap,
        );
        self.model_buffers = model_buffers;
        self.bind_groups = bind_groups;
        self.capacity = new_cap;
    }

    /// Write camera uniform for this view (once per frame).
    pub fn write_camera(&self, queue: &wgpu::Queue, camera: &renderer::CameraUniform) {
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(camera));
    }

    /// Write model uniform for instance `i`.
    pub fn write_model(&self, queue: &wgpu::Queue, index: usize, model: &renderer::ModelUniform) {
        queue.write_buffer(&self.model_buffers[index], 0, bytemuck::bytes_of(model));
    }

    /// Get the bind group for instance `i`.
    pub fn bind_group(&self, index: usize) -> &wgpu::BindGroup {
        &self.bind_groups[index]
    }

    fn create_slots(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        camera_buffer: &wgpu::Buffer,
        count: usize,
    ) -> (Vec<wgpu::Buffer>, Vec<wgpu::BindGroup>) {
        let mut model_buffers = Vec::with_capacity(count);
        let mut bind_groups = Vec::with_capacity(count);

        for i in 0..count {
            let model_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("PerView Model Buffer {}", i)),
                size: std::mem::size_of::<renderer::ModelUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("PerView Bind Group {}", i)),
                layout,
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

            model_buffers.push(model_buffer);
            bind_groups.push(bind_group);
        }

        (model_buffers, bind_groups)
    }
}
// data_types is re-exported from mod.rs (super)
use super::CameraRenderData;

use crate::ecs_components;
use crate::ecs_resources;
use crate::skope_data;
use crate::physics;
use crate::renderer;
use crate::debug;
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
        mut scene_viewer: Option<&mut editor::scene_viewer::SceneViewer>,
        _command_stack: &mut editor::command::CommandStack,
        editor_debug_viz: &editor::debug_viz::EditorDebugViz,
        magic_builder: Option<&mut crate::game::MagicCircleBuilderState>,
        delta_time: f32,
        viewport_size_override: Option<(u32, u32)>,
    ) -> Result<(), wgpu::SurfaceError> {
        // Frame count for debugging
        static FRAME_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        FRAME_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

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
                // f32→u32: round instead of truncate to prevent ±1px oscillation
                (w.round() as u32, h.round() as u32)
            } else {
                // fallback: 현재 텍스처 크기 유지
                self.viewport_texture.size
            };
            let current_tex_size = self.viewport_texture.size;

            // 뷰포트 패널 크기가 변경되면 텍스처 리사이즈
            // 픽셀 스냅으로 값이 안정적이므로 정확 비교 (hysteresis 불필요)
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
                let updates: Vec<(Entity, f32)> = {
                    let query = world.query::<(Entity, &ecs_components::Skeleton, &ecs_components::AnimationController)>();

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
                near: cam.settings.near,
                far: cam.settings.far,
            }
        } else {
            // Fallback: default camera
            let pos = glam::Vec3::new(0.0, -10.0, 5.0);
            let view = glam::Mat4::look_at_rh(pos, glam::Vec3::ZERO, glam::Vec3::Z);
            let proj = glam::Mat4::perspective_rh(45.0_f32.to_radians(), scene_aspect, 0.1, 100.0);
            CameraRenderData { view, proj, position: pos, near: 0.1, far: 100.0 }
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
        let mut culling_stats = renderer::frustum::CullingStats::default();

        let mesh_instances: Vec<(Entity, usize, usize, glam::Mat4)> = {
            // First try with MeshBounds for precise culling
            let query_with_bounds = world.query_filtered::<(
                Entity,
                &ecs_components::MeshInstance,
                &ecs_components::MaterialHandle,
                &ecs_components::GlobalTransform,
                &ecs_components::MeshBounds,
            ), Without<ecs_components::Hidden>>();

            let all_bounded: Vec<_> = query_with_bounds.iter(world).collect();
            culling_stats.total_objects += all_bounded.len() as u32;

            let mut results: Vec<_> = all_bounded
                .into_iter()
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
                .map(|(entity, mesh_instance, material_handle, global_transform, _)| {
                    (
                        entity,
                        mesh_instance.mesh_index,
                        material_handle.material_index,
                        global_transform.0,
                    )
                })
                .collect();

            culling_stats.visible_objects += results.len() as u32;

            // Also include entities without MeshBounds (no culling for them)
            let query_without_bounds = world.query_filtered::<(
                Entity,
                &ecs_components::MeshInstance,
                &ecs_components::MaterialHandle,
                &ecs_components::GlobalTransform,
            ), (Without<ecs_components::Hidden>, Without<ecs_components::MeshBounds>)>();

            let additional: Vec<_> = query_without_bounds
                .iter(world)
                .map(|(entity, mesh_instance, material_handle, global_transform)| {
                    (
                        entity,
                        mesh_instance.mesh_index,
                        material_handle.material_index,
                        global_transform.0,
                    )
                })
                .collect();

            culling_stats.total_objects += additional.len() as u32;
            culling_stats.visible_objects += additional.len() as u32;
            results.extend(additional);
            results
        };

        culling_stats.culled_objects = culling_stats.total_objects - culling_stats.visible_objects;
        log::trace!(
            "[Frustum Culling] total={}, visible={}, culled={} ({:.1}% culled)",
            culling_stats.total_objects,
            culling_stats.visible_objects,
            culling_stats.culled_objects,
            culling_stats.cull_ratio() * 100.0,
        );

        // ============ Phase 17a: Update LightManager (before borrowing other resources) ============
        {
            // Clone the Arc'd device/queue for use in this scope
            let gpu_ctx = world.get_resource::<ecs_resources::GpuContext>().unwrap();
            let device = gpu_ctx.device.clone();
            let queue = gpu_ctx.queue.clone();
            let _ = gpu_ctx;  // Release immutable borrow

            if let Some(light_manager_res) = world.get_resource_mut::<ecs_resources::LightManagerRes>() {
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

        // ============ Phase 17: Deferred Rendering ============
        // Read sun lighting from ECS ExtractedLighting (no more hardcoded values)
        let extracted_lighting = world.get_resource::<ecs_resources::RenderExtractedData>()
            .map(|d| d.lighting.clone())
            .unwrap_or_default();
        let sun_direction = extracted_lighting.sun_direction;
        {
            // Read lighting settings from Environment resource
            let env = world.get_resource::<ecs_resources::Environment>()
                .cloned()
                .unwrap_or_default();

            let sun_color = extracted_lighting.sun_color;
            let sun_intensity = extracted_lighting.sun_intensity;

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
            self.deferred_renderer.update_blit_params(&self.queue, debug_mode, true, self.deferred_renderer.settings.exposure);

            // Prepare mesh render data using persistent buffer pool
            // (avoids per-frame create_buffer_init + create_bind_group)
            {
                // Debug: first frame only
                static FIRST_FRAME_LOGGED: std::sync::Once = std::sync::Once::new();
                FIRST_FRAME_LOGGED.call_once(|| {
                    for (i, (_, mesh_idx, material_idx, world_transform)) in mesh_instances.iter().enumerate() {
                        let pos = world_transform.w_axis;
                        let scale_x = world_transform.x_axis.length();
                        let scale_y = world_transform.y_axis.length();
                        let scale_z = world_transform.z_axis.length();
                        log::info!("[RENDER] Instance {}: mesh={}, mat={}, pos=({:.2},{:.2},{:.2}), scale=({:.2},{:.2},{:.2})",
                                 i, mesh_idx, material_idx, pos.x, pos.y, pos.z, scale_x, scale_y, scale_z);
                    }
                });
            }

            // Lazy-init + ensure capacity for scene buffer pool
            let layout = self.deferred_renderer.camera_bind_group_layout();
            if self.scene_buffer_pool.is_none() {
                self.scene_buffer_pool = Some(PerViewBufferPool::new(&gpu_context.device, layout));
            }
            let pool = self.scene_buffer_pool.as_mut().unwrap();
            pool.ensure_capacity(&gpu_context.device, layout, mesh_instances.len());

            // Write camera uniform once for all instances
            let camera_uniform = renderer::CameraUniform::new(
                view,
                proj,
                camera_pos,
                (self.size.width, self.size.height),
                scene_camera.near,
                scene_camera.far,
            );
            pool.write_camera(&self.queue, &camera_uniform);

            // Write per-instance model uniforms
            for (i, (_, _, _, world_transform)) in mesh_instances.iter().enumerate() {
                let model_uniform = renderer::ModelUniform::new(*world_transform);
                pool.write_model(&self.queue, i, &model_uniform);
            }

            // Build MeshRenderData slice using pool's persistent bind groups
            let render_meshes: Vec<renderer::MeshRenderData> = mesh_instances
                .iter()
                .enumerate()
                .map(|(i, (_, mesh_idx, material_idx, world_transform))| {
                    let mesh_data = &mesh_assets.meshes[*mesh_idx];
                    let material = if *material_idx < material_assets.materials.len() {
                        &material_assets.materials[*material_idx]
                    } else {
                        &material_assets.materials[0]
                    };

                    let geometry_mesh_idx = self.deferred_renderer.geometry_buffer
                        .as_ref()
                        .and_then(|geom| geom.mesh_to_geom.get(mesh_idx).copied());

                    renderer::MeshRenderData {
                        vertex_buffer: &mesh_data.vertex_buffer,
                        index_buffer: &mesh_data.index_buffer,
                        index_count: mesh_data.num_indices,
                        camera_bind_group: pool.bind_group(i),
                        material_bind_group: material.deferred_bind_group.as_ref()
                            .unwrap_or(&material.material_bind_group),
                        geometry_mesh_idx,
                        material_index: *material_idx as u32,
                        model_matrix: world_transform.to_cols_array_2d(),
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
                    multiview_mask: None,
                });
            }

            // Sync debug UI screen-space effect settings to renderer
            // Note: SSAO replaced by GTAO
            // self.deferred_renderer.settings.enable_gtao = false; // DEBUG removed
            // TODO: Add SSR, Contact Shadows, Volumetric, SSS toggles to DebugUi
            // Currently using RenderSettings defaults

            // GPU Scene: incremental update with persistent entity→InstanceId mapping
            // Only adds/removes/updates instances that changed since last frame.
            {
                self.deferred_renderer.gpu_scene_begin_frame();

                let geom_ref = self.deferred_renderer.geometry_buffer.as_ref();

                // 1. Collect current frame entity set (only entities with geometry mapping)
                let current_entities: std::collections::HashSet<u64> = mesh_instances.iter()
                    .filter(|(_, mesh_idx, _, _)| geom_ref.map_or(false, |g| g.mesh_to_geom.contains_key(mesh_idx)))
                    .map(|(entity, _, _, _)| entity.to_bits())
                    .collect();

                // 2. Remove entities that are no longer present
                let removed: Vec<u64> = self.gpu_scene_mapping.keys()
                    .filter(|k| !current_entities.contains(k))
                    .copied().collect();
                for key in removed {
                    if let Some(id) = self.gpu_scene_mapping.remove(&key) {
                        self.deferred_renderer.gpu_scene.remove_instance(id);
                    }
                }

                // 3. Add new entities or update existing transforms
                for (entity, mesh_idx, material_idx, world_transform) in mesh_instances.iter() {
                    let geom = match self.deferred_renderer.geometry_buffer.as_ref() {
                        Some(g) => g,
                        None => continue,
                    };
                    let geom_idx = match geom.mesh_to_geom.get(mesh_idx) {
                        Some(&idx) => idx,
                        None => continue,
                    };

                    let key = entity.to_bits();

                    if let Some(&id) = self.gpu_scene_mapping.get(&key) {
                        // Existing entity — update transform only
                        self.deferred_renderer.gpu_scene.update_transform(id, *world_transform);
                    } else {
                        // New entity — add instance
                        let base_mesh_info = &geom.mesh_infos[geom_idx];

                        let pos = world_transform.w_axis;
                        let scale = glam::Vec3::new(
                            world_transform.x_axis.truncate().length(),
                            world_transform.y_axis.truncate().length(),
                            world_transform.z_axis.truncate().length(),
                        );
                        let max_scale = scale.x.max(scale.y).max(scale.z);

                        let instance_id = self.deferred_renderer.gpu_scene.add_instance(
                            &renderer::InstanceDesc {
                                world_transform: *world_transform,
                                bounds_center: glam::Vec3::new(pos.x, pos.y, pos.z),
                                bounds_radius: max_scale,
                                mesh_id: geom_idx as u32,
                                material_id: *material_idx as u32,
                                flags: renderer::instance_flags::VISIBLE | renderer::instance_flags::SHADOW_CASTER | renderer::instance_flags::MOVABLE,
                                vertex_offset: base_mesh_info.vertex_offset,
                                index_offset: base_mesh_info.index_offset,
                                index_count: base_mesh_info.index_count,
                                ..Default::default()
                            },
                        );
                        self.gpu_scene_mapping.insert(key, instance_id);
                    }
                }

                self.deferred_renderer.gpu_scene_upload(&self.device, &self.queue);
            }

            // MegaLights: tile classification + RIS sampling (before render_vbuffer)
            if self.deferred_renderer.settings.enable_megalights {
                if let Some(light_manager_res) = world.get_resource::<ecs_resources::LightManagerRes>() {
                    if let Some(light_buf) = light_manager_res.manager.light_buffer() {
                        let light_count = light_manager_res.manager.total_light_count() as u32;
                        let view_proj = proj * view;
                        let inv_view_proj = view_proj.inverse();
                        self.deferred_renderer.update_megalights(
                            &self.device,
                            &self.queue,
                            &mut encoder,
                            light_buf,
                            light_count,
                            inv_view_proj,
                        );
                    }
                }
            }

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


        // ============ Game View rendering (ECS Camera) ============
        // Conditional rendering: only when camera exists (for potential game view tab)
        {
            static LOGGED_ONCE: std::sync::Once = std::sync::Once::new();
            LOGGED_ONCE.call_once(|| {
                log::info!("[RENDER] game_camera.is_some() = {}", game_camera.is_some());
            });
        }
        let should_render_game = game_camera.is_some();
        if let Some(game_cam) = should_render_game.then_some(()).and(game_camera.as_ref()) {
            // Prepare Game View mesh render data using persistent buffer pool
            let layout = self.deferred_renderer.camera_bind_group_layout();
            if self.game_buffer_pool.is_none() {
                self.game_buffer_pool = Some(PerViewBufferPool::new(&gpu_context.device, layout));
            }
            let game_pool = self.game_buffer_pool.as_mut().unwrap();
            game_pool.ensure_capacity(&gpu_context.device, layout, mesh_instances.len());

            // Write game camera uniform once (use Camera component near/far)
            let game_camera_uniform = renderer::CameraUniform::new(
                game_cam.view,
                game_cam.proj,
                game_cam.position,
                (self.game_viewport_texture.size.0, self.game_viewport_texture.size.1),
                game_cam.near,
                game_cam.far,
            );
            game_pool.write_camera(&self.queue, &game_camera_uniform);

            // Write per-instance model uniforms
            for (i, (_, _, _, world_transform)) in mesh_instances.iter().enumerate() {
                let model_uniform = renderer::ModelUniform::new(*world_transform);
                game_pool.write_model(&self.queue, i, &model_uniform);
            }

            // Build Game View render meshes
            let game_render_meshes: Vec<renderer::MeshRenderData> = mesh_instances
                .iter()
                .enumerate()
                .map(|(i, (_, mesh_idx, material_idx, world_transform))| {
                    let mesh_data = &mesh_assets.meshes[*mesh_idx];
                    let material = if *material_idx < material_assets.materials.len() {
                        &material_assets.materials[*material_idx]
                    } else {
                        &material_assets.materials[0]
                    };
                    let geometry_mesh_idx = self.deferred_renderer.geometry_buffer
                        .as_ref()
                        .and_then(|geom| geom.mesh_to_geom.get(mesh_idx).copied());

                    renderer::MeshRenderData {
                        vertex_buffer: &mesh_data.vertex_buffer,
                        index_buffer: &mesh_data.index_buffer,
                        index_count: mesh_data.num_indices,
                        camera_bind_group: game_pool.bind_group(i),
                        material_bind_group: material.deferred_bind_group.as_ref()
                            .unwrap_or(&material.material_bind_group),
                        geometry_mesh_idx,
                        material_index: *material_idx as u32,
                        model_matrix: world_transform.to_cols_array_2d(),
                    }
                })
                .collect();

            // Update Lighting for Game View (uses same ECS-extracted lighting)
            let game_sun_direction = extracted_lighting.sun_direction;
            let game_sun_color = extracted_lighting.sun_color;
            self.deferred_renderer.update_lighting(
                &self.queue,
                game_cam.view,
                game_cam.proj,
                game_cam.position,
                game_sun_direction,
                game_sun_color,
                extracted_lighting.sun_intensity,
                1.0, 1000.0, 100.0, 0.05,
                0, // No debug mode for game view
            );

            // Sync debug UI screen-space effect settings to renderer (Game View)
            // Note: SSAO replaced by GTAO
            // self.deferred_renderer.settings.enable_gtao = false; // DEBUG removed
            // TODO: Add SSR, Contact Shadows, Volumetric, SSS toggles to DebugUi

            // MegaLights: tile classification + RIS sampling (before render_vbuffer)
            if self.deferred_renderer.settings.enable_megalights {
                if let Some(light_manager_res) = world.get_resource::<ecs_resources::LightManagerRes>() {
                    if let Some(light_buf) = light_manager_res.manager.light_buffer() {
                        let light_count = light_manager_res.manager.total_light_count() as u32;
                        let game_view_proj = game_cam.proj * game_cam.view;
                        let game_inv_view_proj = game_view_proj.inverse();
                        self.deferred_renderer.update_megalights(
                            &self.device,
                            &self.queue,
                            &mut encoder,
                            light_buf,
                            light_count,
                            game_inv_view_proj,
                        );
                    }
                }
            }

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
            let emitter_query_ref = world.query::<&particles::ParticleEmitter>();
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
                            multiview_mask: None,
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
                            multiview_mask: None,
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
                        multiview_mask: None,
                    });

                    self.debug_draw_renderer.render(&mut debug_pass);
                }

                // Clear one-time primitives at frame end
                debug_buffer.clear_frame();
            }
        }

        // ============ Game UI Rendering ============
        // NOTE: skope_game_ui crate has been removed. Game UI rendering is disabled.
        // TODO: Reimplement game UI with skope_ui when ready.
        {
            // Magic Builder overlay rendering (Play mode only)
            // TODO: Reimplement MagicCircleBuilderState UI with skope_ui
            // (calculate_layout and root() methods were removed with skope_game_ui)
            let _ = magic_builder;
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
            {
                let frame = FRAME_COUNT.load(std::sync::atomic::Ordering::Relaxed);
                if frame % 60 == 0 || debug_ui.entities.is_empty() {
                    debug_ui.entities = debug::ui::collect_entity_info(world);
                }
            }

            // ============ Dock Layout UI (Unreal/Unity style layout) ============

            // Selected entity for Inspector
            let _selected_entity: Option<Entity> = scene_viewer
                .as_ref()
                .and_then(|sv| sv.selection.entities.first().copied());

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

            // (Inspector action handling removed - editor stub types removed)

            // (Hierarchy action handling removed - editor stub types removed)

            // ========== Menu action handling ==========
            // TODO: Implement menu action handling via skope_ui
            // Menu actions (CreateEmpty, Create3DObject, CreateLight, etc.) will be triggered
            // from menus and handled here when reimplemented

            // (Drag and drop handling removed - was disabled dead code using editor stub types)

            // (Asset Browser action handling removed - editor stub types removed)

            // Suppress unused variable warnings
            let _ = debug_ui;

            // Handle console actions
            if let Some(action) = debug_ui.take_action() {
                match action {
                    debug::ui::ConsoleAction::ReloadScene => {
                        // Phase 3: scene reload implementation
                        let default_level = format!("{}/start.skope", paths::game::LEVELS);
                        let level_path = std::env::var("SKOPE_LEVEL")
                            .unwrap_or(default_level);

                        // 1. Collect existing scene entities (except camera)
                        let to_despawn: Vec<Entity> = {
                            let query = world.query::<Entity>();
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
                        match crate::scene::load_from_file(world, std::path::Path::new(&level_path)) {
                            Ok(()) => {
                                skope_data::process_pending_colliders(world);

                                debug_ui.log(
                                    debug::ui::LogLevel::Info,
                                    &format!("Reloaded scene: removed {} entities", despawn_count),
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
                    multiview_mask: None,
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

// NOTE: UI Lua API helper functions (collect_widget_info_for_lua, etc.) removed
// along with skope_game_ui crate. UI command processing (game_ui_commands.rs)
// will also need updating when game UI is reimplemented with skope_ui.
