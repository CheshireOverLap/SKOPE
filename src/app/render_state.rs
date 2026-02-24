//! RenderState — RT 전용 GPU 렌더링 리소스
//!
//! State에서 GPU 관련 리소스를 분리하여 RT로 move.
//! wgpu 28.0의 Send+Sync 덕분에 unsafe 없이 스레드 간 이동 가능.

use std::collections::HashMap;
use std::sync::Arc;

use crate::renderer;
use crate::debug;
use skope_effects as effects;
use skope_magic as magic;

use super::state::render::PerViewBufferPool;
use super::scene_data::SceneRenderData;
use crate::editor::gizmo::GizmoMode;

/// RT 전용 오버레이 렌더러 (Grid + Gizmo)
pub struct OverlayRenderers {
    pub grid: crate::editor::scene_viewer::GridRenderer,
    pub move_gizmo: crate::editor::gizmo::MoveGizmo,
    pub rotate_gizmo: crate::editor::gizmo::RotateGizmo,
    pub scale_gizmo: crate::editor::gizmo::ScaleGizmo,
}

/// RT 전용 — 모든 GPU 렌더링 리소스
///
/// wgpu 28.0: Device, Queue, Buffer, Texture, Pipeline 등 모두 Send+Sync.
/// State에서 `extract_from()`으로 소비하여 RT로 move.
#[allow(dead_code)]
pub struct RenderState {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub config: wgpu::SurfaceConfiguration,
    pub depth_texture: wgpu::TextureView,
    /// Deferred renderer (61+ 필드, 전부 Send+Sync)
    pub deferred_renderer: renderer::Renderer,
    /// Debug draw renderer
    pub debug_draw_renderer: debug::DebugDrawRenderer,
    /// 통합 이펙트 렌더러 (Flipbook + VAT + GPU Particle)
    pub effect_renderer: effects::EffectRenderer,
    /// 마법진 렌더러 (SDF 기반)
    pub magic_circle_renderer: magic::MagicCircleRenderer,
    /// 텍스처 배열 관리자 (material_eval용)
    pub texture_array_manager: renderer::texture_array::TextureArrayManager,
    /// Scene 뷰포트 텍스처
    pub viewport_texture: renderer::ViewportTexture,
    /// Game 뷰포트 텍스처
    pub game_viewport_texture: renderer::ViewportTexture,
    /// GPU Scene persistent entity→InstanceId mapping
    pub gpu_scene_mapping: HashMap<u64, renderer::InstanceId>,
    /// Persistent GPU buffer pool for Scene View
    pub scene_buffer_pool: Option<PerViewBufferPool>,
    /// Persistent GPU buffer pool for Game View
    pub game_buffer_pool: Option<PerViewBufferPool>,
    /// 오버레이 렌더러 (Grid + Gizmo)
    pub overlay: Option<OverlayRenderers>,
}

impl RenderState {
    /// State에서 GPU 리소스를 move (State는 이후 사용 불가)
    ///
    /// GT에서 State를 생성한 후, GPU 리소스만 추출하여 RT로 전달.
    /// 에디터 UI 상태, 아이콘 매니저 등 GT 전용 데이터는 별도 보관.
    pub fn extract_from(state: super::State) -> (Self, RenderStateRemainder) {
        // 오버레이 렌더러 생성 (RenderState 소유)
        let format = state.config.format;
        let depth_format = wgpu::TextureFormat::Depth32Float;
        let overlay = OverlayRenderers {
            grid: crate::editor::scene_viewer::GridRenderer::new(&state.device, format, depth_format),
            move_gizmo: crate::editor::gizmo::MoveGizmo::new(&state.device, format, depth_format),
            rotate_gizmo: crate::editor::gizmo::RotateGizmo::new(&state.device, format, depth_format),
            scale_gizmo: crate::editor::gizmo::ScaleGizmo::new(&state.device, format, depth_format),
        };

        let render_state = Self {
            device: state.device.clone(),
            queue: state.queue.clone(),
            config: state.config,
            depth_texture: state.depth_texture,
            deferred_renderer: state.deferred_renderer,
            debug_draw_renderer: state.debug_draw_renderer,
            effect_renderer: state.effect_renderer,
            magic_circle_renderer: state.magic_circle_renderer,
            texture_array_manager: state.texture_array_manager,
            viewport_texture: state.viewport_texture,
            game_viewport_texture: state.game_viewport_texture,
            gpu_scene_mapping: state.gpu_scene_mapping,
            scene_buffer_pool: state.scene_buffer_pool,
            game_buffer_pool: state.game_buffer_pool,
            overlay: Some(overlay),
        };

        let remainder = RenderStateRemainder {
            device: state.device,
            queue: state.queue,
            editor_ui_state: state.editor_ui_state,
            icon_manager: state.icon_manager,
            #[cfg(debug_assertions)]
            shader_hot_reload: state.shader_hot_reload,
            #[cfg(debug_assertions)]
            material_hot_reload: state.material_hot_reload,
        };

        (render_state, remainder)
    }

    /// 3D 씬 렌더링 (RT에서 실행)
    ///
    /// State::render()에서 이식. SceneRenderData의 값 기반 데이터로 렌더링.
    pub fn render_scene(&mut self, data: &SceneRenderData) -> Result<(), wgpu::SurfaceError> {
        // ============ Viewport Texture resize ============
        if let Some((vp_w, vp_h)) = data.viewport_size {
            let current_tex_size = self.viewport_texture.size;
            if vp_w > 0 && vp_h > 0 && (current_tex_size.0 != vp_w || current_tex_size.1 != vp_h) {
                log::info!("[RT Viewport] Resizing texture: {}x{} -> {}x{}",
                    current_tex_size.0, current_tex_size.1, vp_w, vp_h);

                self.viewport_texture.resize(&self.device, (vp_w, vp_h));
                self.game_viewport_texture.resize(&self.device, (vp_w, vp_h));
                self.deferred_renderer.resize(&self.device, vp_w, vp_h);
            }
        }

        // ============ Camera setup ============
        let scene_camera = &data.scene_camera;
        let (view, proj, camera_pos) = (scene_camera.view, scene_camera.proj, scene_camera.position);

        let (_vp_w, _vp_h) = self.viewport_texture.size;

        // ============ Frustum Culling: already done on GT — use mesh_instances directly ============

        // ============ Lighting ============
        let extracted_lighting = &data.extracted_lighting;
        let sun_direction = extracted_lighting.sun_direction;
        let sun_color = extracted_lighting.sun_color;

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("RT Render Encoder"),
        });

        {
            let debug_mode = data.debug_view_mode;

            self.deferred_renderer.update_lighting_with_env(
                &self.queue,
                view,
                proj,
                camera_pos,
                sun_direction,
                sun_color,
                extracted_lighting.sun_intensity,
                &data.environment,
                data.debug_params.intensity_scale,
                data.debug_params.d_ggx_max,
                data.debug_params.specular_max,
                data.debug_params.roughness_min,
                debug_mode,
            );

            self.deferred_renderer.update_blit_params(
                &self.queue,
                debug_mode,
                true,
                self.deferred_renderer.settings.exposure,
            );

            // Update light buffers (from SceneRenderData)
            if let (Some(ref light_buf), Some(ref count_buf)) =
                (&data.light_buffer, &data.light_count_buffer)
            {
                self.deferred_renderer.update_light_buffers(&self.device, light_buf, count_buf);
            }

            // Clustered lighting (refactored — no LightManager needed)
            if let Some(ref light_buf) = data.light_buffer {
                self.deferred_renderer.update_clustered_lighting_from_data(
                    &self.device,
                    &self.queue,
                    light_buf,
                    &data.gpu_lights_for_culling,
                    view,
                    proj,
                    Some((
                        &self.texture_array_manager.albedo_array.view,
                        &self.texture_array_manager.normal_array.view,
                        &self.texture_array_manager.metallic_roughness_array.view,
                    )),
                );
            }

            // Prepare mesh render data using persistent buffer pool
            let layout = self.deferred_renderer.camera_bind_group_layout();
            if self.scene_buffer_pool.is_none() {
                self.scene_buffer_pool = Some(PerViewBufferPool::new(&self.device, layout));
            }
            let pool = self.scene_buffer_pool.as_mut().unwrap();
            pool.ensure_capacity(&self.device, layout, data.mesh_instances.len());

            // Write camera uniform
            let camera_uniform = renderer::CameraUniform::new(
                view,
                proj,
                camera_pos,
                (data.window_size.0, data.window_size.1),
                scene_camera.near,
                scene_camera.far,
            );
            pool.write_camera(&self.queue, &camera_uniform);

            // Write per-instance model uniforms
            for (i, inst) in data.mesh_instances.iter().enumerate() {
                let model_uniform = renderer::ModelUniform::new(inst.world_transform);
                pool.write_model(&self.queue, i, &model_uniform);
            }

            // Build MeshRenderData slice
            let render_meshes: Vec<renderer::MeshRenderData> = data.mesh_instances
                .iter()
                .enumerate()
                .map(|(i, inst)| {
                    let mesh_data = &data.mesh_assets.meshes[inst.mesh_index];
                    let material = if inst.material_index < data.material_assets.materials.len() {
                        &data.material_assets.materials[inst.material_index]
                    } else {
                        &data.material_assets.materials[0]
                    };

                    let geometry_mesh_idx = self.deferred_renderer.geometry_buffer
                        .as_ref()
                        .and_then(|geom| geom.mesh_to_geom.get(&inst.mesh_index).copied());

                    renderer::MeshRenderData {
                        vertex_buffer: &mesh_data.vertex_buffer,
                        index_buffer: &mesh_data.index_buffer,
                        index_count: mesh_data.num_indices,
                        camera_bind_group: pool.bind_group(i),
                        material_bind_group: material.deferred_bind_group.as_ref()
                            .unwrap_or(&material.material_bind_group),
                        geometry_mesh_idx,
                        material_index: inst.material_index as u32,
                        model_matrix: inst.world_transform.to_cols_array_2d(),
                    }
                })
                .collect();

            // Depth clear
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

            // GPU Scene: incremental update
            {
                self.deferred_renderer.gpu_scene_begin_frame();

                let geom_ref = self.deferred_renderer.geometry_buffer.as_ref();

                let current_entities: std::collections::HashSet<u64> = data.mesh_instances.iter()
                    .filter(|inst| geom_ref.map_or(false, |g| g.mesh_to_geom.contains_key(&inst.mesh_index)))
                    .map(|inst| inst.entity_bits)
                    .collect();

                let removed: Vec<u64> = self.gpu_scene_mapping.keys()
                    .filter(|k| !current_entities.contains(k))
                    .copied().collect();
                for key in removed {
                    if let Some(id) = self.gpu_scene_mapping.remove(&key) {
                        self.deferred_renderer.gpu_scene.remove_instance(id);
                    }
                }

                for inst in &data.mesh_instances {
                    let geom = match self.deferred_renderer.geometry_buffer.as_ref() {
                        Some(g) => g,
                        None => continue,
                    };
                    let geom_idx = match geom.mesh_to_geom.get(&inst.mesh_index) {
                        Some(&idx) => idx,
                        None => continue,
                    };

                    let key = inst.entity_bits;

                    if let Some(&id) = self.gpu_scene_mapping.get(&key) {
                        self.deferred_renderer.gpu_scene.update_transform(id, inst.world_transform);
                    } else {
                        let base_mesh_info = &geom.mesh_infos[geom_idx];

                        let pos = inst.world_transform.w_axis;
                        let scale = glam::Vec3::new(
                            inst.world_transform.x_axis.truncate().length(),
                            inst.world_transform.y_axis.truncate().length(),
                            inst.world_transform.z_axis.truncate().length(),
                        );
                        let max_scale = scale.x.max(scale.y).max(scale.z);

                        let instance_id = self.deferred_renderer.gpu_scene.add_instance(
                            &renderer::InstanceDesc {
                                world_transform: inst.world_transform,
                                bounds_center: glam::Vec3::new(pos.x, pos.y, pos.z),
                                bounds_radius: max_scale,
                                mesh_id: geom_idx as u32,
                                material_id: inst.material_index as u32,
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

            // MegaLights
            if self.deferred_renderer.settings.enable_megalights {
                if let Some(ref light_buf) = data.light_buffer {
                    let view_proj = proj * view;
                    let inv_view_proj = view_proj.inverse();
                    self.deferred_renderer.update_megalights(
                        &self.device,
                        &self.queue,
                        &mut encoder,
                        light_buf,
                        data.total_light_count,
                        inv_view_proj,
                    );
                }
            }

            // V-Buffer deferred rendering to viewport texture
            self.deferred_renderer.render_vbuffer(
                &self.device,
                &mut encoder,
                self.viewport_texture.render_target(),
                &render_meshes,
                &self.queue,
                view,
                proj,
                sun_direction,
                sun_color,
            );

            // Copy V-Buffer depth to viewport_texture depth
            self.deferred_renderer.copy_depth_to(
                &mut encoder,
                &self.viewport_texture.depth_texture,
            );
        }

        // ============ Game View rendering ============
        if let Some(game_cam) = data.game_camera.as_ref() {
            let layout = self.deferred_renderer.camera_bind_group_layout();
            if self.game_buffer_pool.is_none() {
                self.game_buffer_pool = Some(PerViewBufferPool::new(&self.device, layout));
            }
            let game_pool = self.game_buffer_pool.as_mut().unwrap();
            game_pool.ensure_capacity(&self.device, layout, data.mesh_instances.len());

            let game_camera_uniform = renderer::CameraUniform::new(
                game_cam.view,
                game_cam.proj,
                game_cam.position,
                (self.game_viewport_texture.size.0, self.game_viewport_texture.size.1),
                game_cam.near,
                game_cam.far,
            );
            game_pool.write_camera(&self.queue, &game_camera_uniform);

            for (i, inst) in data.mesh_instances.iter().enumerate() {
                let model_uniform = renderer::ModelUniform::new(inst.world_transform);
                game_pool.write_model(&self.queue, i, &model_uniform);
            }

            let game_render_meshes: Vec<renderer::MeshRenderData> = data.mesh_instances
                .iter()
                .enumerate()
                .map(|(i, inst)| {
                    let mesh_data = &data.mesh_assets.meshes[inst.mesh_index];
                    let material = if inst.material_index < data.material_assets.materials.len() {
                        &data.material_assets.materials[inst.material_index]
                    } else {
                        &data.material_assets.materials[0]
                    };
                    let geometry_mesh_idx = self.deferred_renderer.geometry_buffer
                        .as_ref()
                        .and_then(|geom| geom.mesh_to_geom.get(&inst.mesh_index).copied());

                    renderer::MeshRenderData {
                        vertex_buffer: &mesh_data.vertex_buffer,
                        index_buffer: &mesh_data.index_buffer,
                        index_count: mesh_data.num_indices,
                        camera_bind_group: game_pool.bind_group(i),
                        material_bind_group: material.deferred_bind_group.as_ref()
                            .unwrap_or(&material.material_bind_group),
                        geometry_mesh_idx,
                        material_index: inst.material_index as u32,
                        model_matrix: inst.world_transform.to_cols_array_2d(),
                    }
                })
                .collect();

            let game_sun_direction = data.extracted_lighting.sun_direction;
            let game_sun_color = data.extracted_lighting.sun_color;
            self.deferred_renderer.update_lighting(
                &self.queue,
                game_cam.view,
                game_cam.proj,
                game_cam.position,
                game_sun_direction,
                game_sun_color,
                data.extracted_lighting.sun_intensity,
                1.0, 1000.0, 100.0, 0.05,
                0,
            );

            // MegaLights for game view
            if self.deferred_renderer.settings.enable_megalights {
                if let Some(ref light_buf) = data.light_buffer {
                    let game_view_proj = game_cam.proj * game_cam.view;
                    let game_inv_view_proj = game_view_proj.inverse();
                    self.deferred_renderer.update_megalights(
                        &self.device,
                        &self.queue,
                        &mut encoder,
                        light_buf,
                        data.total_light_count,
                        game_inv_view_proj,
                    );
                }
            }

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
        }

        // ============ Particle / Effect Rendering — 일시 비활성화 (후속 Step) ============
        // Effects (Flipbook/VAT): EffectRenderData에 raw pointer — !Send
        // Magic Circles: MagicCircleRenderData Clone/Send 미구현

        // ============ Debug Draw Rendering ============
        if !data.debug_draw_primitives.is_empty() {
            let view_proj = proj * view;

            // Build temporary DebugDrawBuffer from primitives
            let mut temp_buffer = debug::DebugDrawBuffer::new();
            for prim in data.debug_draw_primitives.iter() {
                let p: debug::draw::DebugPrimitive = prim.clone();
                temp_buffer.push_primitive(p);
            }

            self.debug_draw_renderer.update(&self.queue, &temp_buffer, view_proj);

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
        }

        // ============ Scene Viewer overlay (Grid + Gizmo) ============
        if let (Some(ref overlay), Some(ref overlay_data)) = (&self.overlay, &data.overlay) {
            let view_proj = proj * view;
            let _aspect = if overlay_data.screen_size.0 > 0 && overlay_data.screen_size.1 > 0 {
                overlay_data.screen_size.0 as f32 / overlay_data.screen_size.1 as f32
            } else {
                16.0 / 9.0
            };

            // Grid
            if overlay_data.show_grid {
                overlay.grid.render_with_data(
                    &self.queue, &mut encoder,
                    self.viewport_texture.render_target(),
                    self.viewport_texture.depth_target(),
                    view_proj, data.scene_camera.position,
                );
            }

            // Gizmo (선택된 엔티티가 있을 때만)
            if overlay_data.has_selection {
                match overlay_data.gizmo_mode {
                    GizmoMode::Move => {
                        overlay.move_gizmo.render_with_state(
                            &self.queue, &mut encoder,
                            self.viewport_texture.render_target(),
                            self.viewport_texture.depth_target(),
                            view_proj,
                            overlay_data.gizmo_position,
                            overlay_data.gizmo_rotation,
                            overlay_data.gizmo_scale,
                            overlay_data.gizmo_hovered_axis,
                        );
                    }
                    GizmoMode::Rotate => {
                        overlay.rotate_gizmo.render_with_state(
                            &self.queue, &mut encoder,
                            self.viewport_texture.render_target(),
                            self.viewport_texture.depth_target(),
                            view_proj,
                            overlay_data.gizmo_position,
                            overlay_data.gizmo_rotation,
                            overlay_data.gizmo_scale,
                            overlay_data.gizmo_hovered_axis,
                        );
                    }
                    GizmoMode::Scale => {
                        overlay.scale_gizmo.render_with_state(
                            &self.queue, &mut encoder,
                            self.viewport_texture.render_target(),
                            self.viewport_texture.depth_target(),
                            view_proj,
                            overlay_data.gizmo_position,
                            overlay_data.gizmo_rotation,
                            overlay_data.gizmo_scale,
                            overlay_data.gizmo_hovered_axis,
                        );
                    }
                    GizmoMode::Select => {} // Gizmo 표시 안함
                }
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));

        Ok(())
    }
}

/// SceneRenderer trait 구현 — RT에서 Box<dyn SceneRenderer>로 호출
impl skope_ui::render_thread::SceneRenderer for RenderState {
    fn render(&mut self, data: Box<dyn std::any::Any + Send>) {
        let data = match data.downcast::<SceneRenderData>() {
            Ok(d) => d,
            Err(_) => {
                log::error!("[RenderState] Invalid SceneRenderData type");
                return;
            }
        };
        if let Err(e) = self.render_scene(&data) {
            log::error!("[RenderState] render_scene error: {:?}", e);
        }
    }

    fn viewport_texture_infos(&self) -> Vec<skope_ui::render_thread::ViewportTextureInfo> {
        vec![
            skope_ui::render_thread::ViewportTextureInfo {
                name: "scene_viewport".to_string(),
                view: self.viewport_texture.texture.create_view(&Default::default()),
                size: self.viewport_texture.size,
            },
            skope_ui::render_thread::ViewportTextureInfo {
                name: "game_viewport".to_string(),
                view: self.game_viewport_texture.texture.create_view(&Default::default()),
                size: self.game_viewport_texture.size,
            },
        ]
    }
}

/// State에서 RenderState 추출 후 GT에 남는 데이터
///
/// GPU 리소스가 아닌, GT에서 계속 사용하는 상태.
/// 후속 Step에서 hot reload, editor_ui_state 등 활용 예정.
#[allow(dead_code)]
pub struct RenderStateRemainder {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub editor_ui_state: Option<super::slate_ui::EditorUiState>,
    #[allow(dead_code)]
    pub icon_manager: crate::editor::IconManager,
    #[cfg(debug_assertions)]
    pub shader_hot_reload: Option<crate::shaders::ShaderHotReload>,
    #[cfg(debug_assertions)]
    pub material_hot_reload: Option<crate::material::MaterialHotReload>,
}
