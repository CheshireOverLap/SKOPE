//! EngineHandler - SlateAppHandler 구현
//!
//! SlateApp의 핸들러로 동작하며, 엔진 로직(ECS, 물리, Lua, 3D 렌더링 등)을 담당
//! 기존 App의 기능을 SlateAppHandler 인터페이스로 래핑

use std::sync::Arc;
use glam::Vec2;
use winit::window::Window;
use winit::event::{ElementState, MouseButton};
use skope_ecs::prelude::*;

use skope_ui::application::{SlateAppHandler, FloatingWindowRequest, RedockRequest};
use skope_ui::render_thread::ViewportTextureInfo;
use skope_ui::framework::{InputPipeline, TooltipManager, UICommandList, PopupLayer, NotificationManager, WidgetReflector, AccessibilityProvider};
use skope_ui::docking::{TabId, NodeId, NodeRect, DockPosition, DragEndNotification, DragOperationRequest, TabRole};
use skope_ui::widget::Widget;

use crate::app::{State, SharedEditorContext, CommandQueue, create_shared_context};
use crate::app::slate_ui::EditorUiState;
use crate::app::commands::EditorCommand;
use skope_lighting;
use crate::debug;
use crate::editor;
use crate::ecs_components;
use crate::ecs_resources;
use crate::paths;
use crate::scripting;
use crate::shaders;
use crate::audio;
use crate::physics;
use crate::material;
use crate::particles;
use crate::prefab;
use crate::skope_data;

/// 엔진 초기화 단계
enum InitPhase {
    /// GPU 초기화 대기 중
    WaitingForGpu,
    // TODO: Phase 5에서 Splash/SplashComplete 단계 추가
    /// 엔진 정상 실행 중
    Running,
}

/// 엔진 핸들러 - SlateApp의 콜백으로 동작
#[allow(dead_code)]
pub struct EngineHandler {
    // === 초기화 단계 ===
    init_phase: InitPhase,

    // === GPU 리소스 (on_gpu_initialized에서 설정) ===
    device: Option<Arc<wgpu::Device>>,
    queue: Option<Arc<wgpu::Queue>>,
    window: Option<Arc<Window>>,

    // === 엔진 상태 ===
    pub state: Option<State>,
    pub world: World,
    pub schedule: Schedule,

    // === RT 이동용 (Step 7) ===
    /// RenderState — extract_scene_renderer()에서 꺼내서 RT로 전송
    render_state: Option<crate::app::RenderState>,
    /// RenderState 추출 후 GT에 남는 데이터
    remainder: Option<crate::app::render_state::RenderStateRemainder>,
    /// 매 프레임 수집된 씬 렌더 데이터 (drain_scene_render_data에서 꺼냄)
    pending_scene_data: Option<crate::app::SceneRenderData>,

    // === 에디터 ===
    pub scene_viewer: Option<editor::scene_viewer::SceneViewer>,
    pub editor_mode: editor::EditorMode,
    pub command_stack: editor::command::CommandStack,
    pub editor_debug_viz: editor::debug_viz::EditorDebugViz,
    pub clipboard: editor::clipboard::Clipboard,
    pub editor_context: SharedEditorContext,
    pub command_queue: CommandQueue,

    // === UI ===
    pub editor_ui_state: EditorUiState,
    pub debug_ui: debug::DebugUi,
    pub magic_builder: crate::game::MagicCircleBuilderState,

    // === 기타 상태 ===
    pub shader_manager: Option<shaders::ShaderManager>,
    pub scale_factor: f32,
    pub last_mouse_pos: (f32, f32),
    pub cursor_captured: bool,
    pub current_scene_path: Option<std::path::PathBuf>,
    pub scene_dirty: bool,

    // === Live Link ===
    #[cfg(feature = "live_link")]
    pub live_link: Option<editor::live_link::LiveLink>,

    // === 프레임 제어 ===
    frames_to_skip: u32,
    needs_center_window: bool,

    // === 더블클릭 감지 ===
    last_click_time: Option<std::time::Instant>,
    last_click_position: (f64, f64),

    // === Input Preprocessor ===
    input_pipeline: InputPipeline,

    // === Tooltip ===
    tooltip_manager: TooltipManager,
    command_list: UICommandList,
    popup_layer: PopupLayer,
    notification_manager: NotificationManager,
    widget_reflector: WidgetReflector,
    accessibility_provider: AccessibilityProvider,
}

impl EngineHandler {
    pub fn new(
        world: World,
        schedule: Schedule,
        debug_ui: debug::DebugUi,
    ) -> Self {
        Self {
            init_phase: InitPhase::WaitingForGpu,
            device: None,
            queue: None,
            window: None,
            state: None,
            world,
            schedule,
            render_state: None,
            remainder: None,
            pending_scene_data: None,
            scene_viewer: None,
            editor_mode: editor::EditorMode::default(),
            command_stack: editor::command::CommandStack::new(),
            editor_debug_viz: editor::debug_viz::EditorDebugViz::default(),
            clipboard: editor::clipboard::Clipboard::new(),
            editor_context: create_shared_context(),
            command_queue: CommandQueue::new(),
            editor_ui_state: EditorUiState::new(),
            debug_ui,
            magic_builder: crate::game::MagicCircleBuilderState::new(),
            shader_manager: None,
            scale_factor: 1.0,
            last_mouse_pos: (0.0, 0.0),
            cursor_captured: false,
            current_scene_path: None,
            scene_dirty: false,
            #[cfg(feature = "live_link")]
            live_link: None,
            frames_to_skip: 0,
            needs_center_window: false,
            last_click_time: None,
            last_click_position: (0.0, 0.0),
            input_pipeline: InputPipeline::new(),
            tooltip_manager: TooltipManager::new(),
            command_list: UICommandList::new(),
            popup_layer: PopupLayer::new(),
            notification_manager: NotificationManager::new(),
            widget_reflector: WidgetReflector::new(),
            accessibility_provider: AccessibilityProvider::new(),
        }
    }
}

impl SlateAppHandler for EngineHandler {
    fn root_widget(&mut self) -> &mut dyn Widget {
        &mut self.editor_ui_state.dock_panel
    }

    fn update(&mut self, _delta_time: f32) {
        // 프레임 업데이트 - 기존 App::handle_redraw() 로직
        // Phase 5에서 본격 구현 예정

        // 명령 큐 처리
        self.process_command_queue();

        // Running 상태에서만 엔진 로직 실행
        if !matches!(self.init_phase, InitPhase::Running) {
            return;
        }

        // 셰이더 핫리로드
        #[cfg(debug_assertions)]
        if let Some(ref mut shader_mgr) = self.shader_manager {
            let reloaded = shader_mgr.auto_reload();
            if !reloaded.is_empty() {
                log::info!("[ShaderHotReload] Auto-reloaded {} shaders", reloaded.len());
            }
        }

        // Play State 동기화
        self.sync_play_state();

        // Time 업데이트
        let should_run_gameplay = {
            let game_state = self.world.get_resource::<ecs_resources::GamePlayState>();
            game_state.map(|s| s.should_run_gameplay()).unwrap_or(false)
        };

        if let Some(time) = self.world.get_resource_mut::<ecs_resources::Time>() {
            if should_run_gameplay {
                time.update();
            } else {
                time.delta_seconds = 0.0;
            }
        }

        // Sync FrameTime for network systems (frame-rate-independent interpolation/prediction)
        if let Some(time) = self.world.get_resource::<ecs_resources::Time>() {
            let dt = time.delta_seconds;
            if let Some(ft) = self.world.get_resource_mut::<skope_net::FrameTime>() {
                ft.delta_seconds = dt;
            }
        }

        // Scene Viewer 업데이트
        if let Some(ref mut scene_viewer) = self.scene_viewer {
            static mut LAST_UPDATE: Option<std::time::Instant> = None;
            let real_dt = unsafe {
                let now = std::time::Instant::now();
                let dt = LAST_UPDATE.map(|last| (now - last).as_secs_f32()).unwrap_or(1.0 / 60.0);
                LAST_UPDATE = Some(now);
                dt.min(0.1)
            };
            scene_viewer.update(real_dt);
        }

        // Live Link
        #[cfg(feature = "live_link")]
        {
            if let Some(mut live_link) = self.live_link.take() {
                self.process_live_link_messages(&mut live_link);
                self.live_link = Some(live_link);
            }
        }

        // Lua Hot Reload
        self.check_lua_hot_reload();

        // Shader/Material Hot Reload
        #[cfg(debug_assertions)]
        self.check_hot_reloads();

        // Lua 상태 업데이트
        self.update_lua_state();

        // ECS Systems 실행
        if should_run_gameplay {
            self.schedule.run(&mut self.world);
            if let Some(game_state) = self.world.get_resource_mut::<ecs_resources::GamePlayState>() {
                game_state.clear_step();
            }
        }

        // Lua 이벤트 처리
        self.process_lua_events();

        // 디버그 토글
        self.handle_debug_toggles();

        // 메뉴 액션 처리 (dock_panel에서 소비하지 못한 메뉴 클릭)
        self.process_menu_actions();

        // Debug UI 통계 + 콘솔 커맨드 (Step 0: render()에서 이동)
        self.process_console_and_debug_ui();

        // 네트워크 상태 바 업데이트
        self.update_network_status_bar();

        // ─── RT 이동 준비: render()에서 분리된 ECS 변이 (Step 5) ───

        // Transform Propagation (Transform → GlobalTransform)
        crate::ecs_systems::transform_propagate_system(&mut self.world);

        // Animation Controller 업데이트
        {
            let delta_seconds = self.world.get_resource::<ecs_resources::Time>()
                .map(|t| t.delta_seconds)
                .unwrap_or(0.016);

            // 1. Animation duration map 생성
            let duration_map: std::collections::HashMap<(String, usize), f32> = {
                if let Some(registry) = self.world.get_resource::<ecs_resources::SkinnedModelRegistry>() {
                    registry.models.iter()
                        .flat_map(|(name, data)| {
                            data.animations.iter().enumerate()
                                .map(move |(idx, anim)| ((name.clone(), idx), anim.duration))
                        })
                        .collect()
                } else {
                    std::collections::HashMap::new()
                }
            };

            // 2. 업데이트 대상 수집
            let updates: Vec<(Entity, f32)> = {
                let query = self.world.query::<(Entity, &ecs_components::Skeleton, &ecs_components::AnimationController)>();

                query.iter(&self.world)
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

            // 3. AnimationController 업데이트
            for (entity, duration) in updates {
                if let Some(mut anim_ctrl) = self.world.get_mut::<ecs_components::AnimationController>(entity) {
                    anim_ctrl.update(delta_seconds, duration);
                }
            }
        }

        // Particle Emitter 업데이트
        {
            let dt = self.world.get_resource::<ecs_resources::Time>()
                .map(|t| t.delta_seconds)
                .unwrap_or(1.0 / 60.0);
            let mut emitter_query = self.world.query::<(
                &ecs_components::Transform,
                &mut particles::ParticleEmitter,
            )>();
            for (transform, mut emitter) in emitter_query.iter_mut(&mut self.world) {
                emitter.update(dt, transform.translation);
            }
        }
    }

    fn on_resize(&mut self, width: u32, height: u32) {
        // RenderState가 RT로 이동했으므로 state.resize() 불필요
        // 뷰포트 리사이즈는 RT의 render_scene()이 data.viewport_size로 처리
        if let Some(ref mut scene_viewer) = self.scene_viewer {
            scene_viewer.resize(width, height);
        }
        // 도킹 패널 레이아웃 업데이트
        if let Some(queue) = &self.queue {
            self.editor_ui_state.handle_resize(queue, width, height);
        }
    }

    fn on_close_requested(&mut self) -> bool {
        true
    }

    fn drain_float_requests(&mut self) -> Vec<FloatingWindowRequest> {
        self.editor_ui_state.dock_panel.drain_float_requests()
            .into_iter()
            .map(|req| FloatingWindowRequest {
                tab_id: req.tab_id,
                title: req.title,
                icon: req.icon,
                position: req.position,
                size: req.size,
                content: req.content,
                is_dragging: req.is_dragging,
                role: req.role,
            })
            .collect()
    }

    fn extract_scene_renderer(&mut self) -> Option<Box<dyn skope_ui::render_thread::SceneRenderer>> {
        self.render_state.take().map(|rs| Box::new(rs) as Box<dyn skope_ui::render_thread::SceneRenderer>)
    }

    fn drain_scene_render_data(&mut self) -> Option<Box<dyn std::any::Any + Send>> {
        self.pending_scene_data.take().map(|data| Box::new(data) as Box<dyn std::any::Any + Send>)
    }

    fn viewport_textures(&self) -> Vec<ViewportTextureInfo> {
        // RenderState가 RT로 이동했으므로 3D 뷰포트 텍스처에 직접 접근 불가.
        // RT가 RenderScene 처리 후 자동으로 viewport 텍스처를 등록함.
        Vec::new()
    }

    fn on_floating_window_closed(&mut self, tab_id: TabId) {
        log::info!("[EngineHandler] Floating window closed: {:?}", tab_id);
    }

    fn on_gpu_initialized(
        &mut self,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        _instance: &wgpu::Instance,
        _adapter: &wgpu::Adapter,
        format: wgpu::TextureFormat,
        window: Arc<Window>,
    ) {
        log::info!("[EngineHandler] GPU initialized");
        self.device = Some(device.clone());
        self.queue = Some(queue.clone());
        self.window = Some(window.clone());
        self.scale_factor = window.scale_factor() as f32;

        // DPI 스케일 적용
        self.editor_ui_state.set_dpi_scale(self.scale_factor);

        // State 생성 (headless 모드 - SlateApp이 surface 소유)
        let size = window.inner_size();
        let state = State::from_shared_gpu(
            device.clone(),
            queue.clone(),
            format,
            size.width.max(1),
            size.height.max(1),
            &mut self.world,
        );

        // RenderState 추출 → RT로 이동 (extract_scene_renderer에서 꺼냄)
        let (render_state, remainder) = crate::app::RenderState::extract_from(state);
        self.render_state = Some(render_state);
        self.remainder = Some(remainder);
        // state는 소비됨 — self.state = None 유지

        // 도킹 패널을 실제 윈도우 크기로 업데이트
        self.editor_ui_state.handle_resize(&queue, size.width.max(1), size.height.max(1));

        // SceneViewer 생성 (GT 소유 — 인터랙션 + 카메라 상태 관리)
        let viewport_size = (size.width.max(1), size.height.max(1));
        self.scene_viewer = Some(editor::scene_viewer::SceneViewer::new(
            &device, format,
            wgpu::TextureFormat::Depth32Float,
            viewport_size,
        ));

        self.init_phase = InitPhase::Running;
        log::info!("[EngineHandler] State initialized (headless), RenderState extracted for RT");

        // 레이아웃 복원 — 개발 중 임시 비활성화 (항상 초기 레이아웃으로 시작)
        // let layout_path = std::path::Path::new("engine").join("config").join("editor_layout.json");
        // if layout_path.exists() {
        //     match std::fs::read_to_string(&layout_path) {
        //         Ok(json) => {
        //             let result = self.editor_ui_state.dock_panel.restore_editor_layout(
        //                 &json,
        //                 |major_title, tab_name| {
        //                     crate::app::slate_ui::create_tab_by_name(major_title, tab_name)
        //                 },
        //             );
        //             match result {
        //                 Ok(failed) => {
        //                     if failed.is_empty() {
        //                         log::info!("[EngineHandler] Layout restored from {:?}", layout_path);
        //                     } else {
        //                         log::warn!("[EngineHandler] Layout restored with unresolved tabs: {:?}", failed);
        //                     }
        //                 }
        //                 Err(e) => log::error!("[EngineHandler] Failed to restore layout: {}", e),
        //             }
        //         }
        //         Err(e) => log::error!("[EngineHandler] Failed to read layout file: {}", e),
        //     }
        // }
    }

    fn pre_render(&mut self) {
        if !matches!(self.init_phase, InitPhase::Running) {
            return;
        }

        // delta_time
        let delta_time = self.world
            .get_resource::<ecs_resources::Time>()
            .map(|t| t.delta_seconds)
            .unwrap_or(1.0 / 60.0);

        let frame_number = self.world
            .get_resource::<ecs_resources::Time>()
            .map(|t| t.frame_count)
            .unwrap_or(0);

        // scene_viewer 분리 (borrow checker)
        let scene_viewer = self.scene_viewer.take();

        // UE 동기 리사이즈 패턴
        let viewport_size = {
            let (_x, _y, w, h) = self.editor_ui_state.get_viewport_rect();
            let (w_u32, h_u32) = (w.round() as u32, h.round() as u32);
            if w_u32 > 0 && h_u32 > 0 { Some((w_u32, h_u32)) } else { None }
        };

        // ─── Scene camera ───
        let scene_aspect = if let Some((vp_w, vp_h)) = viewport_size {
            if vp_w > 0 && vp_h > 0 { vp_w as f32 / vp_h as f32 } else { 16.0 / 9.0 }
        } else {
            16.0 / 9.0
        };

        let scene_camera = if let Some(ref sv) = scene_viewer {
            let cam = &sv.camera;
            super::data_types::CameraRenderData {
                view: cam.view_matrix(),
                proj: cam.projection_matrix(scene_aspect),
                position: cam.position,
                near: cam.settings.near,
                far: cam.settings.far,
            }
        } else {
            let pos = glam::Vec3::new(0.0, -10.0, 5.0);
            let view = glam::Mat4::look_at_rh(pos, glam::Vec3::ZERO, glam::Vec3::Z);
            let proj = glam::Mat4::perspective_rh(45.0_f32.to_radians(), scene_aspect, 0.1, 100.0);
            super::data_types::CameraRenderData { view, proj, position: pos, near: 0.1, far: 100.0 }
        };

        let game_camera = super::data_types::CameraRenderData::from_ecs_camera(&self.world, scene_aspect);

        // ─── LightManager GPU buffer update (GT에서 수행) ───
        let (light_buffer, light_count_buffer, total_light_count, gpu_lights) = {
            let gpu_ctx = self.world.get_resource::<ecs_resources::GpuContext>().unwrap();
            let device = gpu_ctx.device.clone();
            let queue = gpu_ctx.queue.clone();
            let _ = gpu_ctx;  // Release immutable borrow

            if let Some(light_manager_res) = self.world.get_resource_mut::<ecs_resources::LightManagerRes>() {
                // GT: update GPU buffers (write_buffer 등)
                light_manager_res.manager.update_gpu_buffers(&device, &queue);

                let lb = light_manager_res.manager.light_buffer().cloned();
                let lcb = light_manager_res.manager.light_count_buffer().cloned();
                let total = light_manager_res.manager.total_light_count() as u32;

                // Collect GpuLights for CPU culling on RT
                let mut gpu_lights = Vec::new();
                for light in &light_manager_res.manager.point_lights {
                    gpu_lights.push(skope_lighting::GpuLight::from_point(light));
                }
                for light in &light_manager_res.manager.spot_lights {
                    gpu_lights.push(skope_lighting::GpuLight::from_spot(light));
                }

                (lb, lcb, total, gpu_lights)
            } else {
                (None, None, 0, Vec::new())
            }
        };

        // ─── Debug visualization primitives ───
        {
            let selection_transforms: Vec<ecs_components::Transform> =
                if self.editor_debug_viz.show_selection_bounds {
                    if let Some(ref sv) = scene_viewer {
                        sv.selection.entities.iter()
                            .filter_map(|e| self.world.get::<ecs_components::Transform>(*e).cloned())
                            .collect()
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };

            let lights_data: Vec<(ecs_components::Transform, ecs_components::Light)> =
                if self.editor_debug_viz.show_lights {
                    self.world.query::<(&ecs_components::Transform, &ecs_components::Light)>()
                        .iter(&self.world)
                        .map(|(t, l)| (t.clone(), l.clone()))
                        .collect()
                } else {
                    Vec::new()
                };

            let colliders_data: Vec<(ecs_components::Transform, physics::ColliderShape)> =
                if self.editor_debug_viz.show_colliders {
                    self.world.query::<(&ecs_components::Transform, &physics::ColliderComponent)>()
                        .iter(&self.world)
                        .map(|(t, c)| (t.clone(), c.shape.clone()))
                        .collect()
                } else {
                    Vec::new()
                };

            if let Some(mut debug_buffer) = self.world.get_resource_mut::<debug::DebugDrawBuffer>() {
                if self.editor_debug_viz.show_selection_bounds && !selection_transforms.is_empty() {
                    editor::debug_viz::draw_selection_bounds(&selection_transforms, &mut debug_buffer);
                }
                if self.editor_debug_viz.show_lights {
                    editor::debug_viz::draw_lights_debug(&lights_data, &mut debug_buffer);
                }
                if self.editor_debug_viz.show_colliders {
                    editor::debug_viz::draw_colliders_debug(&colliders_data, &mut debug_buffer);
                }
            }
        }

        // ─── Overlay data (Grid + Gizmo) ───
        let overlay_data = {
            use crate::editor::gizmo::{GizmoMode, GizmoAxis};
            if let Some(ref sv) = scene_viewer {
                let (gpos, grot, gscale, ghover) = match sv.gizmo_mode {
                    GizmoMode::Move => (sv.move_gizmo.position, sv.move_gizmo.rotation,
                                        sv.move_gizmo.scale, sv.move_gizmo.hovered_axis),
                    GizmoMode::Rotate => (sv.rotate_gizmo.position, sv.rotate_gizmo.rotation,
                                          sv.rotate_gizmo.scale, sv.rotate_gizmo.hovered_axis),
                    GizmoMode::Scale => (sv.scale_gizmo.position, sv.scale_gizmo.rotation,
                                         sv.scale_gizmo.scale, sv.scale_gizmo.hovered_axis),
                    GizmoMode::Select => (glam::Vec3::ZERO, glam::Quat::IDENTITY, 1.0, GizmoAxis::None),
                };

                Some(crate::app::scene_data::OverlayRenderData {
                    show_grid: true,
                    gizmo_mode: sv.gizmo_mode,
                    gizmo_position: gpos,
                    gizmo_rotation: grot,
                    gizmo_scale: gscale,
                    gizmo_hovered_axis: ghover,
                    has_selection: !sv.selection.entities.is_empty(),
                    screen_size: viewport_size.unwrap_or((1920, 1080)),
                })
            } else {
                None
            }
        };

        // ─── Collect SceneRenderData ───
        let scene_data = self.collect_scene_render_data(
            frame_number,
            delta_time,
            scene_camera,
            game_camera,
            viewport_size,
            light_buffer,
            light_count_buffer,
            total_light_count,
            gpu_lights,
            overlay_data,
        );
        self.pending_scene_data = scene_data;

        // 복원
        self.scene_viewer = scene_viewer;

        // DebugDrawBuffer 클리어 (데이터는 이미 SceneRenderData에 복사됨)
        if let Some(debug_buffer) = self.world.get_resource_mut::<crate::debug::DebugDrawBuffer>() {
            debug_buffer.clear_frame();
        }
    }

    fn on_key_event(&mut self, key_code: winit::keyboard::KeyCode, state: ElementState) {
        use winit::keyboard::KeyCode;

        // ECS Resource에 키 입력 저장
        if let Some(keyboard) = self.world.get_resource_mut::<ecs_resources::KeyboardInput>() {
            match state {
                ElementState::Pressed => { keyboard.keys_pressed.insert(key_code); }
                ElementState::Released => { keyboard.keys_pressed.remove(&key_code); }
            }
        }

        // 수정자 키 상태
        let (ctrl_held, shift_held, alt_held) = self.get_modifier_keys();

        // === 단축키 (Pressed만) ===
        if state != ElementState::Pressed {
            // Released일 때는 씬 뷰어 카메라 키만 처리
            if self.editor_mode.is_edit() {
                if let Some(ref mut sv) = self.scene_viewer {
                    let camera_key = Self::map_camera_key(key_code);
                    sv.on_key(camera_key, false);
                }
            }
            return;
        }

        // F5: 다음 debug view | Shift+F5: 이전 debug view
        if key_code == KeyCode::F5 {
            if shift_held {
                self.debug_ui.debug_view = self.debug_ui.debug_view.prev();
            } else {
                self.debug_ui.debug_view = self.debug_ui.debug_view.next();
            }
            log::info!("[DebugView] → {}", self.debug_ui.debug_view.name());
        }

        // F6: Debug view 끄기 (None으로 복귀)
        if key_code == KeyCode::F6 {
            self.debug_ui.debug_view = skope_debug_ui::DebugView::None;
            log::info!("[DebugView] → {}", self.debug_ui.debug_view.name());
        }

        // F4: 디버그 시각화 토글
        if key_code == KeyCode::F4 && self.editor_mode.is_edit() {
            self.editor_debug_viz.toggle_all();
            log::info!("[Editor] Debug viz toggled");
        }

        // F8: Game input capture 해제
        if key_code == KeyCode::F8 {
            if let Ok(mut ctx) = self.editor_context.write() {
                if ctx.is_game_input_captured() {
                    ctx.release_game_input();
                    log::info!("[Game View] Input capture released (F8)");
                }
            }
        }

        // Ctrl+키 단축키 (Edit 모드)
        if ctrl_held && self.editor_mode.is_edit() {
            self.handle_ctrl_shortcuts(key_code, shift_held);
        }

        // Delete/Backspace: 삭제
        if self.editor_mode.is_edit()
            && (key_code == KeyCode::Delete || key_code == KeyCode::Backspace)
        {
            self.handle_delete_entities();
        }

        // Ctrl+P: 부모 설정 / Alt+P: 부모 해제
        if self.editor_mode.is_edit() && key_code == KeyCode::KeyP {
            if ctrl_held && !shift_held {
                self.handle_parent_entities();
            } else if alt_held && !ctrl_held {
                self.handle_unparent_entities();
            }
        }

        // H: 숨기기 | Shift+H: Isolate | Alt+H: Unhide all
        if self.editor_mode.is_edit() && key_code == KeyCode::KeyH && !ctrl_held {
            if alt_held {
                self.handle_unhide_all();
            } else if shift_held {
                self.handle_isolate();
            } else {
                self.handle_hide_selected();
            }
        }

        // Shift+D: 복제
        if self.editor_mode.is_edit() && key_code == KeyCode::KeyD
            && shift_held && !ctrl_held && !alt_held
        {
            self.handle_duplicate();
        }

        // L: Local/World Space 전환
        if self.editor_mode.is_edit() && key_code == KeyCode::KeyL && !ctrl_held && !alt_held {
            if let Some(ref mut sv) = self.scene_viewer {
                sv.toggle_space();
            }
        }

        // G: 그리드 스냅 토글
        if self.editor_mode.is_edit() && key_code == KeyCode::KeyG && !ctrl_held && !alt_held {
            if let Some(ref mut sv) = self.scene_viewer {
                sv.toggle_snap();
            }
        }

        // Numpad 카메라 프리셋
        if self.editor_mode.is_edit() {
            if let Some(ref mut sv) = self.scene_viewer {
                match key_code {
                    KeyCode::Numpad7 => sv.camera.set_top_view(),
                    KeyCode::Numpad1 => sv.camera.set_front_view(),
                    KeyCode::Numpad3 => sv.camera.set_right_view(),
                    KeyCode::Numpad0 => sv.camera.set_perspective_view(),
                    _ => {}
                }
            }
        }

        // F: 선택된 엔티티에 포커스
        if self.editor_mode.is_edit() && key_code == KeyCode::KeyF && !ctrl_held && !alt_held {
            if let Some(ref mut sv) = self.scene_viewer {
                sv.focus_on_selection(&self.world);
            }
        }

        // WASD / Space / Shift 카메라 키
        if self.editor_mode.is_edit() {
            if let Some(ref mut sv) = self.scene_viewer {
                let camera_key = Self::map_camera_key(key_code);
                sv.on_key(camera_key, true);
            }
        }
    }

    fn on_mouse_event(&mut self, button: MouseButton, state: ElementState, position: Vec2) {
        // ECS 리소스 업데이트
        if let Some(mouse) = self.world.get_resource_mut::<ecs_resources::MouseInput>() {
            if button == MouseButton::Right {
                mouse.is_pressed = state == ElementState::Pressed;
                if !mouse.is_pressed {
                    mouse.last_pos = None;
                }
            }
        }
        self.last_mouse_pos = (position.x, position.y);

        if !self.editor_mode.is_edit() {
            return;
        }

        let is_press = state == ElementState::Pressed;
        let pos = Vec2::new(position.x * self.scale_factor, position.y * self.scale_factor);
        let (_, _, alt_held) = self.get_modifier_keys();

        match button {
            MouseButton::Left => {
                // Gizmo 드래그 + 엔티티 선택
                let mut should_sync = false;
                if let Some(ref mut sv) = self.scene_viewer {
                    let gizmo_cmd = sv.on_mouse_button(
                        editor::scene_viewer::MouseButton::Left,
                        is_press,
                        pos,
                        alt_held,
                    );
                    if let Some(cmd) = gizmo_cmd {
                        self.command_stack.push_executed(cmd);
                        self.scene_dirty = true;
                        should_sync = true;
                    }

                    // 릴리즈 시 오브젝트 선택
                    if !is_press {
                        let (ctrl_held, shift_held, _) = Self::get_modifier_keys_from_world(&self.world);
                        use editor::selection::SelectionModifier;
                        let modifier = match (shift_held, ctrl_held) {
                            (true, false) => SelectionModifier::Additive,
                            (false, true) => SelectionModifier::Toggle,
                            _ => SelectionModifier::Replace,
                        };
                        sv.try_pick(&mut self.world, pos, modifier);
                    }
                }
                if should_sync {
                    self.sync_inspector_state();
                }
            }
            MouseButton::Right => {
                // 씬 뷰어 카메라 Look/Orbit
                if let Some(ref mut sv) = self.scene_viewer {
                    sv.on_mouse_button(
                        editor::scene_viewer::MouseButton::Right,
                        is_press,
                        pos,
                        alt_held,
                    );
                }
            }
            MouseButton::Middle => {
                // 씬 뷰어 카메라 Pan
                if let Some(ref mut sv) = self.scene_viewer {
                    sv.on_mouse_button(
                        editor::scene_viewer::MouseButton::Middle,
                        is_press,
                        pos,
                        false,
                    );
                }
            }
            _ => {}
        }
    }

    fn on_cursor_moved(&mut self, position: Vec2) {
        self.last_mouse_pos = (position.x, position.y);

        // ECS 리소스 업데이트
        if let Some(mouse) = self.world.get_resource_mut::<ecs_resources::MouseInput>() {
            mouse.last_pos = Some((position.x as f64, position.y as f64));
        }

        // 씬 뷰어 마우스 이동 (Gizmo hover + 카메라 orbit/pan)
        if self.editor_mode.is_edit() {
            if let Some(ref mut sv) = self.scene_viewer {
                sv.on_mouse_move(
                    Vec2::new(position.x * self.scale_factor, position.y * self.scale_factor),
                    &mut self.world,
                );
            }
        }
    }

    fn on_mouse_wheel(&mut self, delta: f32) {
        // 씬 뷰어 스크롤 (줌)
        if self.editor_mode.is_edit() {
            if let Some(ref mut sv) = self.scene_viewer {
                sv.on_scroll(delta);
            }
        }
    }

    fn on_scale_factor_changed(&mut self, scale_factor: f64) {
        self.scale_factor = scale_factor as f32;
        self.editor_ui_state.set_dpi_scale(self.scale_factor);
        log::info!("[EngineHandler] Scale factor changed: {}", self.scale_factor);
    }

    fn drain_window_action(&mut self) -> Option<skope_ui::docking::WindowControlAction> {
        self.editor_ui_state.dock_panel.take_window_action()
    }

    fn input_pipeline(&mut self) -> Option<&mut InputPipeline> {
        Some(&mut self.input_pipeline)
    }

    fn on_key_event_for_ui(&mut self, key_code: winit::keyboard::KeyCode, state: ElementState) -> bool {
        if state != ElementState::Pressed {
            return false;
        }
        use skope_ui::event::{KeyEvent, KeyCode as SlateKeyCode, Modifiers};
        use skope_ui::core::Geometry;

        let modifiers = Self::get_modifier_keys_from_world(&self.world);
        let key_event = KeyEvent {
            key: SlateKeyCode::from(key_code),
            modifiers: Modifiers {
                shift: modifiers.0,
                ctrl: modifiers.1,
                alt: modifiers.2,
                ..Default::default()
            },
            is_pressed: true,
            is_repeat: false,
        };

        let size = glam::Vec2::new(
            self.editor_ui_state.window_size.0 as f32,
            self.editor_ui_state.window_size.1 as f32,
        );
        let geometry = Geometry::make_root(size, 1.0);

        let reply = self.editor_ui_state.dock_panel.on_key_down(&geometry, &key_event);
        reply.is_handled()
    }

    fn tooltip_manager(&mut self) -> Option<&mut TooltipManager> {
        Some(&mut self.tooltip_manager)
    }

    fn command_list(&mut self) -> Option<&mut UICommandList> {
        Some(&mut self.command_list)
    }

    fn popup_layer(&mut self) -> Option<&mut PopupLayer> {
        Some(&mut self.popup_layer)
    }

    fn notification_manager(&mut self) -> Option<&mut NotificationManager> {
        Some(&mut self.notification_manager)
    }

    fn widget_reflector(&mut self) -> Option<&mut WidgetReflector> {
        Some(&mut self.widget_reflector)
    }

    fn accessibility_provider(&mut self) -> Option<&mut AccessibilityProvider> {
        Some(&mut self.accessibility_provider)
    }

    fn tick_widgets(&mut self, delta_time: f32) {
        self.editor_ui_state.dock_panel.tick_all(delta_time);
    }

    fn set_external_dock_target(&mut self, local_pos: Vec2) {
        self.editor_ui_state.dock_panel.set_external_dock_target(local_pos);
    }

    fn clear_external_dock_target(&mut self) {
        self.editor_ui_state.dock_panel.clear_external_dock_target();
    }

    fn get_external_dock_target(&self) -> Option<NodeRect> {
        self.editor_ui_state.dock_panel.get_external_dock_target()
    }

    fn update_external_dock_hover(&mut self, local_pos: Vec2) {
        self.editor_ui_state.dock_panel.update_external_dock_hover(local_pos);
    }

    fn get_external_dock_info(&self) -> Option<(NodeId, DockPosition, Option<NodeRect>)> {
        self.editor_ui_state.dock_panel.get_external_dock_info()
    }

    fn tick_external_compass(&mut self, dt: f32) {
        self.editor_ui_state.dock_panel.tick_external_compass(dt);
    }

    fn set_external_preview_tab(&mut self, info: Option<(String, Option<String>)>) {
        self.editor_ui_state.dock_panel.set_external_preview_tab(info);
    }

    fn restore_cancelled_drag(&mut self, tab_id: TabId, title: String, icon: Option<String>, content: Box<dyn Widget>, role: TabRole) {
        self.editor_ui_state.dock_panel.restore_cancelled_drag(tab_id, title, icon, content, role);
    }

    fn clear_ghost_tab(&mut self) {
        self.editor_ui_state.dock_panel.clear_ghost_tab();
    }

    fn on_redock_request(&mut self, request: RedockRequest) {
        self.editor_ui_state.dock_panel.handle_redock(
            request.tab_id,
            request.title,
            request.icon,
            request.content,
            request.drop_position,
            request.target_stack_id,
            request.dock_position,
        );
    }

    fn drain_drag_end_notifications(&mut self) -> Vec<DragEndNotification> {
        self.editor_ui_state.dock_panel.drain_drag_end_notifications()
    }

    fn redock_tab(&mut self, tab_id: TabId, title: String, icon: Option<String>, target_stack_id: NodeId, position: DockPosition, content: Box<dyn Widget>) {
        self.editor_ui_state.dock_panel.add_tab_with_content(tab_id, title, icon, content, target_stack_id, position);
    }

    fn drain_drag_operation_request(&mut self) -> Option<DragOperationRequest> {
        self.editor_ui_state.dock_panel.drain_drag_operation_request()
    }

    fn on_shutdown(&mut self) {
        // 레이아웃 저장 — 개발 중 임시 비활성화 (항상 초기 레이아웃으로 시작)
        // let config_dir = std::path::Path::new("engine").join("config");
        // let layout_path = config_dir.join("editor_layout.json");
        // match self.editor_ui_state.dock_panel.save_editor_layout("LastSession") {
        //     Ok(json) => {
        //         let _ = std::fs::create_dir_all(&config_dir);
        //         match std::fs::write(&layout_path, &json) {
        //             Ok(_) => log::info!("[EngineHandler] Layout saved to {:?}", layout_path),
        //             Err(e) => log::error!("[EngineHandler] Failed to save layout: {}", e),
        //         }
        //     }
        //     Err(e) => log::error!("[EngineHandler] Failed to serialize layout: {}", e),
        // }
    }
}

// === Private methods (기존 App의 로직 이식) ===
impl EngineHandler {
    /// GT에서 SceneRenderData 수집 (매 프레임)
    ///
    /// ECS 쿼리로 frustum-culled 메시 인스턴스, GPU 에셋, 라이팅, 디버그 데이터를 수집.
    #[allow(clippy::too_many_arguments)]
    fn collect_scene_render_data(
        &self,
        frame_number: u64,
        delta_time: f32,
        scene_camera: super::data_types::CameraRenderData,
        game_camera: Option<super::data_types::CameraRenderData>,
        viewport_size: Option<(u32, u32)>,
        light_buffer: Option<wgpu::Buffer>,
        light_count_buffer: Option<wgpu::Buffer>,
        total_light_count: u32,
        gpu_lights_for_culling: Vec<skope_lighting::GpuLight>,
        overlay: Option<crate::app::scene_data::OverlayRenderData>,
    ) -> Option<crate::app::SceneRenderData> {
        use crate::renderer::frustum::Frustum;

        // Frustum culling
        let view_proj = scene_camera.proj * scene_camera.view;
        let frustum = Frustum::from_view_proj(view_proj);

        let mesh_instances: Vec<crate::app::MeshInstanceData> = {
            let query_with_bounds = self.world.query_filtered::<(
                Entity,
                &ecs_components::MeshInstance,
                &ecs_components::MaterialHandle,
                &ecs_components::GlobalTransform,
                &ecs_components::MeshBounds,
            ), Without<ecs_components::Hidden>>();

            let mut results: Vec<_> = query_with_bounds.iter(&self.world)
                .filter(|(_, _, _, global_transform, bounds)| {
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
                    crate::app::MeshInstanceData {
                        entity_bits: entity.to_bits(),
                        mesh_index: mesh_instance.mesh_index,
                        material_index: material_handle.material_index,
                        world_transform: global_transform.0,
                    }
                })
                .collect();

            // Entities without MeshBounds (no culling)
            let query_no_bounds = self.world.query_filtered::<(
                Entity,
                &ecs_components::MeshInstance,
                &ecs_components::MaterialHandle,
                &ecs_components::GlobalTransform,
            ), (Without<ecs_components::Hidden>, Without<ecs_components::MeshBounds>)>();

            results.extend(query_no_bounds.iter(&self.world).map(
                |(entity, mesh_instance, material_handle, global_transform)| {
                    crate::app::MeshInstanceData {
                        entity_bits: entity.to_bits(),
                        mesh_index: mesh_instance.mesh_index,
                        material_index: material_handle.material_index,
                        world_transform: global_transform.0,
                    }
                },
            ));

            results
        };

        // GPU 에셋 clone (wgpu Arc — cheap)
        let mesh_assets = self.world.get_resource::<ecs_resources::MeshAssets>()?.clone();
        let material_assets = self.world.get_resource::<ecs_resources::MaterialAssets>()?.clone();

        // Lighting data
        let extracted_lighting = self.world.get_resource::<ecs_resources::RenderExtractedData>()
            .map(|d| d.lighting.clone())
            .unwrap_or_default();
        let environment = self.world.get_resource::<ecs_resources::Environment>()
            .cloned()
            .unwrap_or_default();

        // Debug params
        let debug_view_mode = self.debug_ui.debug_view.to_shader_mode();
        let debug_params = crate::app::DebugRenderParams {
            intensity_scale: self.debug_ui.intensity_scale,
            d_ggx_max: self.debug_ui.d_ggx_max,
            specular_max: self.debug_ui.specular_max,
            roughness_min: self.debug_ui.roughness_min,
        };

        // Debug draw primitives
        let debug_draw_primitives = self.world.get_resource::<debug::DebugDrawBuffer>()
            .map(|buf| buf.all_primitives().cloned().collect::<Vec<_>>())
            .unwrap_or_default();

        // Window size (for CameraUniform aspect)
        let window_size = self.window.as_ref()
            .map(|w| {
                let size = w.inner_size();
                (size.width.max(1), size.height.max(1))
            })
            .unwrap_or((1920, 1080));

        Some(crate::app::SceneRenderData {
            frame_number,
            delta_time,
            scene_camera,
            game_camera,
            viewport_size,
            game_viewport_size: viewport_size, // same size as scene for now
            mesh_instances,
            mesh_assets,
            material_assets,
            extracted_lighting,
            environment,
            light_buffer,
            light_count_buffer,
            total_light_count,
            gpu_lights_for_culling,
            debug_view_mode,
            debug_params,
            debug_draw_primitives,
            window_size,
            overlay,
        })
    }

    /// Play State 동기화
    fn sync_play_state(&mut self) {
        let new_state = match self.editor_mode {
            editor::EditorMode::Edit => ecs_resources::PlayState::Edit,
            editor::EditorMode::Play => ecs_resources::PlayState::Playing,
        };

        let (just_started, just_stopped, player_entity) = {
            if let Some(game_state) = self.world.get_resource_mut::<ecs_resources::GamePlayState>() {
                game_state.update_state(new_state);
                (game_state.just_started_playing(), game_state.just_stopped_playing(), game_state.player_entity)
            } else {
                (false, false, None)
            }
        };

        if just_started {
            self.spawn_player();
        }
        if just_stopped {
            self.despawn_player(player_entity);
        }
    }

    /// 플레이어 스폰
    fn spawn_player(&mut self) {
        log::warn!("[Game] Player spawning not available (forward pipeline removed)");
    }

    /// 플레이어 디스폰
    fn despawn_player(&mut self, player_entity: Option<Entity>) {
        log::info!("[Game] Exiting play mode - despawning player");
        if let Some(entity) = player_entity {
            self.world.despawn(entity);
            if let Some(game_state) = self.world.get_resource_mut::<ecs_resources::GamePlayState>() {
                game_state.player_spawned = false;
                game_state.player_entity = None;
            }
        }
    }

    /// Lua 핫리로드 체크
    fn check_lua_hot_reload(&mut self) {
        let changed_scripts = self
            .world
            .get_non_send_resource_mut::<scripting::ScriptEngine>()
            .map(|engine| engine.check_hot_reload())
            .unwrap_or_default();

        if !changed_scripts.is_empty() {
            let mut reload_targets: Vec<(std::path::PathBuf, i64)> = Vec::new();
            {
                let query = self.world.query::<&scripting::LuaScript>();
                for script in query.iter(&self.world) {
                    if let Some(instance_id) = script.instance_id {
                        for changed_path in &changed_scripts {
                            let script_abs = if script.path.is_absolute() {
                                script.path.clone()
                            } else {
                                std::path::PathBuf::from(paths::game::SCRIPTS).join(&script.path)
                            };
                            if script_abs == *changed_path || script.path == *changed_path {
                                reload_targets.push((changed_path.clone(), instance_id));
                            }
                        }
                    }
                }
            }

            if let Some(engine) = self.world.get_non_send_resource_mut::<scripting::ScriptEngine>() {
                for (path, instance_id) in reload_targets {
                    if let Err(e) = engine.reload_script(&path, instance_id) {
                        log::warn!("[HotReload] Failed to reload {:?}: {}", path, e);
                    }
                }
            }
        }
    }

    /// 셰이더/머티리얼 핫리로드 체크
    ///
    /// 임시 비활성화: RenderState가 RT로 이동하여 state가 None.
    /// 후속 Step에서 RT에 reload 커맨드 전송으로 복원.
    #[cfg(debug_assertions)]
    fn check_hot_reloads(&mut self) {
        // RenderState가 RT로 이동했으므로 GT에서 직접 리로드 불가.
        // 후속 Step에서 HotReload 커맨드를 RT에 전송하도록 리팩토링 예정.
    }

    /// Lua 스크립팅 상태 업데이트
    fn update_lua_state(&mut self) {
        use winit::keyboard::KeyCode;

        if let Some(engine) = self.world.get_non_send_resource::<scripting::ScriptEngine>() {
            if let Some(time) = self.world.get_resource::<ecs_resources::Time>() {
                let _ = engine.update_time(
                    time.delta_seconds,
                    time.elapsed_seconds as f32,
                    time.frame_count,
                    if time.delta_seconds > 0.0 { 1.0 / time.delta_seconds } else { 60.0 },
                );
            }

            let (mx, my) = self.last_mouse_pos;
            let _ = engine.update_input(mx, my, 0.0, 0.0);

            if let Some(keyboard) = self.world.get_resource::<ecs_resources::KeyboardInput>() {
                let _ = engine.update_key("W", keyboard.keys_pressed.contains(&KeyCode::KeyW));
                let _ = engine.update_key("A", keyboard.keys_pressed.contains(&KeyCode::KeyA));
                let _ = engine.update_key("S", keyboard.keys_pressed.contains(&KeyCode::KeyS));
                let _ = engine.update_key("D", keyboard.keys_pressed.contains(&KeyCode::KeyD));
                let _ = engine.update_key("Up", keyboard.keys_pressed.contains(&KeyCode::ArrowUp));
                let _ = engine.update_key("Down", keyboard.keys_pressed.contains(&KeyCode::ArrowDown));
                let _ = engine.update_key("Left", keyboard.keys_pressed.contains(&KeyCode::ArrowLeft));
                let _ = engine.update_key("Right", keyboard.keys_pressed.contains(&KeyCode::ArrowRight));
                let _ = engine.update_key("Space", keyboard.keys_pressed.contains(&KeyCode::Space));
                let _ = engine.update_key("Shift", keyboard.keys_pressed.contains(&KeyCode::ShiftLeft) || keyboard.keys_pressed.contains(&KeyCode::ShiftRight));
                let _ = engine.update_key("Control", keyboard.keys_pressed.contains(&KeyCode::ControlLeft) || keyboard.keys_pressed.contains(&KeyCode::ControlRight));
                let _ = engine.update_key("E", keyboard.keys_pressed.contains(&KeyCode::KeyE));
                let _ = engine.update_key("Q", keyboard.keys_pressed.contains(&KeyCode::KeyQ));
                let _ = engine.update_key("F", keyboard.keys_pressed.contains(&KeyCode::KeyF));
                let _ = engine.update_key("R", keyboard.keys_pressed.contains(&KeyCode::KeyR));
                let _ = engine.update_key("Escape", keyboard.keys_pressed.contains(&KeyCode::Escape));
            }
        }
    }

    /// Lua 이벤트 처리
    fn process_lua_events(&mut self) {
        // Collision 이벤트
        if let Some(engine) = self.world.get_non_send_resource::<scripting::ScriptEngine>() {
            if let Some(entity_events) = self.world.get_resource::<physics::EntityCollisionEvents>() {
                let lua_events: Vec<scripting::api::LuaCollisionEvent> = entity_events.events.iter()
                    .map(|e| scripting::api::LuaCollisionEvent {
                        entity_a: e.entity_a.to_bits(),
                        entity_b: e.entity_b.to_bits(),
                        is_enter: e.event_type == physics::CollisionEventType::Started,
                    })
                    .collect();

                if !lua_events.is_empty() {
                    let _ = scripting::api::push_collision_events(engine.lua(), &lua_events);
                }
            }
        }

        // Audio 명령
        if let Some(engine) = self.world.get_non_send_resource::<scripting::ScriptEngine>() {
            if let Ok(audio_commands) = scripting::api::process_audio_commands(engine.lua()) {
                if let Some(audio_system) = self.world.get_non_send_resource_mut::<audio::AudioSystem>() {
                    for cmd in audio_commands {
                        match cmd {
                            scripting::api::AudioCommand::Play { sound, volume, looping } => {
                                let settings = audio::PlaySettings::sfx()
                                    .with_volume(volume)
                                    .with_loop(looping);
                                let _ = audio_system.play_with_settings(&sound, settings);
                            }
                            scripting::api::AudioCommand::PlayMusic { sound } => {
                                let _ = audio_system.play_music(&sound);
                            }
                            scripting::api::AudioCommand::Stop { id } => {
                                audio_system.stop(id);
                            }
                            scripting::api::AudioCommand::StopAll => {
                                audio_system.stop_all();
                            }
                            scripting::api::AudioCommand::StopMusic => {
                                audio_system.stop_music();
                            }
                            scripting::api::AudioCommand::SetMasterVolume { volume } => {
                                audio_system.set_master_volume(volume);
                            }
                            scripting::api::AudioCommand::SetMusicVolume { volume } => {
                                audio_system.set_music_volume(volume);
                            }
                            scripting::api::AudioCommand::SetSfxVolume { volume } => {
                                audio_system.set_sfx_volume(volume);
                            }
                            scripting::api::AudioCommand::Play3D { sound, position, volume, looping } => {
                                let settings = audio::SpatialSettings {
                                    volume,
                                    looping,
                                    position: [position.0, position.1, position.2],
                                    ..Default::default()
                                };
                                let _ = audio_system.play_spatial(&sound, settings);
                            }
                            scripting::api::AudioCommand::Pause { id } => {
                                audio_system.pause(id);
                            }
                            scripting::api::AudioCommand::Resume { id } => {
                                audio_system.resume(id);
                            }
                            scripting::api::AudioCommand::SetSourcePosition { id, position } => {
                                audio_system.set_source_position(id, [position.0, position.1, position.2]);
                            }
                        }
                    }
                }
            }
        }
    }

    /// 디버그 토글 처리
    fn handle_debug_toggles(&mut self) {
        use winit::keyboard::KeyCode;

        // F3: Debug UI
        {
            let keyboard = self.world.get_resource::<ecs_resources::KeyboardInput>().unwrap();
            static mut F3_WAS_PRESSED: bool = false;
            let f3_pressed = keyboard.keys_pressed.contains(&KeyCode::F3);
            unsafe {
                if f3_pressed && !F3_WAS_PRESSED {
                    self.debug_ui.toggle();
                }
                F3_WAS_PRESSED = f3_pressed;
            }
        }

        // F2: Magic Builder (Play 모드에서만)
        {
            let keyboard = self.world.get_resource::<ecs_resources::KeyboardInput>().unwrap();
            static mut F2_WAS_PRESSED: bool = false;
            let f2_pressed = keyboard.keys_pressed.contains(&KeyCode::F2);
            unsafe {
                if f2_pressed && !F2_WAS_PRESSED && self.editor_mode.is_play() {
                    self.magic_builder.toggle_visible();
                    log::info!("[Game] MagicBuilder: visible={}", self.magic_builder.visible);
                }
                F2_WAS_PRESSED = f2_pressed;
            }
        }
    }

    /// 메뉴 액션 처리 (File 메뉴, Debug 메뉴 등)
    fn process_menu_actions(&mut self) {
        use skope_debug_ui::DebugView;

        for label in self.editor_ui_state.dock_panel.drain_unhandled_menu_actions() {
            match label.as_str() {
                // File 메뉴
                "New Scene" => {
                    self.new_scene();
                }
                "Open Scene" => {
                    self.open_scene_dialog();
                }
                "Save Scene" => {
                    self.save_scene();
                }
                // Debug 메뉴 — 뷰 모드 전환
                "None (끄기)" => self.debug_ui.debug_view = DebugView::None,
                "Albedo" => self.debug_ui.debug_view = DebugView::Albedo,
                "Normal" => self.debug_ui.debug_view = DebugView::Normal,
                "Depth" => self.debug_ui.debug_view = DebugView::Depth,
                "Metallic" => self.debug_ui.debug_view = DebugView::Metallic,
                "Roughness" => self.debug_ui.debug_view = DebugView::Roughness,
                "Tangent W" => self.debug_ui.debug_view = DebugView::TangentW,
                "Bitangent" => self.debug_ui.debug_view = DebugView::Bitangent,
                "Final Normal" => self.debug_ui.debug_view = DebugView::FinalNormal,
                "Normal Map Raw" => self.debug_ui.debug_view = DebugView::NormalMapRaw,
                "NdotL" => self.debug_ui.debug_view = DebugView::NdotL,
                "Barycentric" => self.debug_ui.debug_view = DebugView::Barycentric,
                "Triangle ID" => self.debug_ui.debug_view = DebugView::TriangleId,
                "UV Coords" => self.debug_ui.debug_view = DebugView::UvCoords,
                "Motion Vectors" => self.debug_ui.debug_view = DebugView::MotionVectors,
                "Motion Vectors Magnitude" => self.debug_ui.debug_view = DebugView::MotionVectorsMagnitude,
                // Debug 메뉴 — 순회
                "Cycle Next" => self.debug_ui.debug_view = self.debug_ui.debug_view.next(),
                "Cycle Prev" => self.debug_ui.debug_view = self.debug_ui.debug_view.prev(),
                // Multiplayer 메뉴
                "Host Game (7777)" => { self.host_game(7777); }
                "Host Game (7778)" => { self.host_game(7778); }
                "Join localhost:7777" => { self.join_game("127.0.0.1:7777"); }
                "Join localhost:7778" => { self.join_game("127.0.0.1:7778"); }
                "Disconnect" => { self.disconnect_game(); }
                _ => {
                    log::debug!("[EngineHandler] Unhandled menu action: {}", label);
                    continue;
                }
            }
        }
    }

    /// 명령 큐 처리
    fn process_command_queue(&mut self) {
        let commands: Vec<_> = self.command_queue.drain().collect();

        for cmd in commands {
            match cmd {
                EditorCommand::SelectEntity(entity) => {
                    if let Ok(mut ctx) = self.editor_context.write() {
                        ctx.select_entity(entity);
                    }
                    log::debug!("[CommandQueue] SelectEntity: {:?}", entity);
                }
                EditorCommand::SetPlayMode(play_state) => {
                    self.editor_mode = play_state;
                    if let Ok(mut ctx) = self.editor_context.write() {
                        ctx.set_editor_mode(self.editor_mode);
                    }
                    log::debug!("[CommandQueue] SetPlayMode: {:?}", play_state);
                }
                EditorCommand::SaveScene => {
                    self.save_scene();
                }
                EditorCommand::LoadScene(path) => {
                    use super::scene_manager;
                    log::info!("[CommandQueue] LoadScene requested: {}", path);
                    let p = std::path::PathBuf::from(&path);
                    if scene_manager::load_scene_from_path(&mut self.world, &p) {
                        self.current_scene_path = Some(p);
                        self.scene_dirty = false;
                    }
                }
                EditorCommand::Undo => {
                    if self.command_stack.undo(&mut self.world) {
                        self.scene_dirty = true;
                    }
                    log::debug!("[CommandQueue] Undo");
                }
                EditorCommand::Redo => {
                    if self.command_stack.redo(&mut self.world) {
                        self.scene_dirty = true;
                    }
                    log::debug!("[CommandQueue] Redo");
                }
            }
        }
    }

    /// Live Link 메시지 처리
    #[cfg(feature = "live_link")]
    fn process_live_link_messages(&mut self, live_link: &mut editor::live_link::LiveLink) {
        use editor::live_link::LiveLinkMessage;

        for msg in live_link.poll_messages() {
            match msg {
                LiveLinkMessage::EntityUpdate { entity, position, rotation, scale } => {
                    let mut query = self.world.query::<(&ecs_components::NodeName, &mut ecs_components::Transform)>();
                    for (name, mut transform) in query.iter_mut(&mut self.world) {
                        if name.0 == entity {
                            transform.translation = glam::Vec3::from_array(position);
                            transform.rotation = glam::Quat::from_array(rotation);
                            transform.scale = glam::Vec3::from_array(scale);
                            break;
                        }
                    }
                }
                LiveLinkMessage::PlayRequest => {
                    self.editor_mode = editor::EditorMode::Play;
                }
                LiveLinkMessage::StopRequest | LiveLinkMessage::PauseRequest => {
                    self.editor_mode = editor::EditorMode::Edit;
                }
                LiveLinkMessage::SceneSync => {
                    let mut entities = Vec::new();
                    let mut query = self.world.query::<(
                        &ecs_components::NodeName,
                        &ecs_components::Transform,
                        Option<&ecs_components::MeshInstance>,
                        Option<&ecs_components::ScriptComponent>,
                    )>();
                    for (name, transform, mesh_opt, script_opt) in query.iter(&self.world) {
                        let mesh_str: Option<String> = mesh_opt.map(|m| format!("mesh_{}", m.mesh_index));
                        let script_str: Option<String> = script_opt.map(|s| s.script_path.clone());
                        entities.push(editor::live_link::EntityData {
                            name: name.0.clone(),
                            position: transform.translation.to_array(),
                            rotation: transform.rotation.to_array(),
                            scale: transform.scale.to_array(),
                            mesh: mesh_str,
                            script: script_str,
                        });
                    }
                    live_link.send_scene_data(entities);
                }
                LiveLinkMessage::Connected { client_name } => {
                    log::info!("[LiveLink] Client connected: {}", client_name);
                }
                _ => {}
            }
        }
    }
}

// === 입력 처리 헬퍼 메서드 ===
impl EngineHandler {
    /// 수정자 키 상태 (ctrl, shift, alt)
    fn get_modifier_keys(&self) -> (bool, bool, bool) {
        Self::get_modifier_keys_from_world(&self.world)
    }

    fn get_modifier_keys_from_world(world: &World) -> (bool, bool, bool) {
        use winit::keyboard::KeyCode;
        let keyboard = world.get_resource::<ecs_resources::KeyboardInput>().unwrap();
        let ctrl = keyboard.keys_pressed.contains(&KeyCode::ControlLeft)
            || keyboard.keys_pressed.contains(&KeyCode::ControlRight);
        let shift = keyboard.keys_pressed.contains(&KeyCode::ShiftLeft)
            || keyboard.keys_pressed.contains(&KeyCode::ShiftRight);
        let alt = keyboard.keys_pressed.contains(&KeyCode::AltLeft)
            || keyboard.keys_pressed.contains(&KeyCode::AltRight);
        (ctrl, shift, alt)
    }

    /// KeyCode → 씬 뷰어 카메라 키 매핑
    fn map_camera_key(key_code: winit::keyboard::KeyCode) -> editor::scene_viewer::Key {
        use winit::keyboard::KeyCode;
        match key_code {
            KeyCode::KeyW => editor::scene_viewer::Key::W,
            KeyCode::KeyS => editor::scene_viewer::Key::S,
            KeyCode::KeyA => editor::scene_viewer::Key::A,
            KeyCode::KeyD => editor::scene_viewer::Key::D,
            KeyCode::KeyE => editor::scene_viewer::Key::E,
            KeyCode::KeyQ => editor::scene_viewer::Key::Q,
            KeyCode::KeyR => editor::scene_viewer::Key::R,
            KeyCode::Space => editor::scene_viewer::Key::Space,
            KeyCode::ShiftLeft => editor::scene_viewer::Key::LShift,
            KeyCode::KeyF => editor::scene_viewer::Key::F,
            _ => editor::scene_viewer::Key::Other,
        }
    }

    /// Ctrl+키 단축키 처리
    fn handle_ctrl_shortcuts(&mut self, key_code: winit::keyboard::KeyCode, shift_held: bool) {
        use winit::keyboard::KeyCode;
        let mut did_undo_redo = false;

        if key_code == KeyCode::KeyZ {
            if shift_held {
                did_undo_redo = self.command_stack.redo(&mut self.world);
            } else {
                did_undo_redo = self.command_stack.undo(&mut self.world);
            }
        } else if key_code == KeyCode::KeyY {
            did_undo_redo = self.command_stack.redo(&mut self.world);
        } else if key_code == KeyCode::KeyS {
            self.save_scene();
        } else if key_code == KeyCode::KeyN {
            self.new_scene();
        } else if key_code == KeyCode::KeyO {
            self.open_scene_dialog();
        } else if key_code == KeyCode::KeyD {
            self.handle_duplicate_with_offset();
        } else if key_code == KeyCode::KeyC {
            self.handle_copy();
        } else if key_code == KeyCode::KeyV {
            self.handle_paste();
        } else if key_code == KeyCode::KeyX {
            self.handle_cut();
        }

        if did_undo_redo {
            self.scene_dirty = true;
            if let Some(ref mut sv) = self.scene_viewer {
                sv.update_gizmo_from_selection(&self.world);
            }
            self.sync_inspector_state();
        }
    }

    /// 씬 저장
    fn save_scene(&mut self) {
        use super::scene_manager;

        if let Some(ref path) = self.current_scene_path.clone() {
            if scene_manager::save_scene_to_path(&mut self.world, &path) {
                self.scene_dirty = false;
                log::info!("[Editor] Scene saved to {:?}", path);
            }
        } else {
            self.save_scene_as_dialog();
        }
    }

    /// 다른 이름으로 저장 대화상자
    fn save_scene_as_dialog(&mut self) {
        use super::scene_manager;

        let file = rfd::FileDialog::new()
            .add_filter("SKOPE Scene", &["skope"])
            .set_title("Save Scene As")
            .set_file_name("untitled.skope")
            .save_file();

        if let Some(path) = file {
            if scene_manager::save_scene_to_path(&mut self.world, &path) {
                self.current_scene_path = Some(path);
                self.scene_dirty = false;
            }
        }
    }

    /// 새 씬 생성 (미저장 확인 포함)
    fn new_scene(&mut self) {
        use super::scene_manager;

        if !scene_manager::prompt_save_if_dirty(
            self.scene_dirty,
            &self.current_scene_path,
            &mut self.world,
        ) {
            return; // 사용자가 취소
        }

        scene_manager::new_scene(&mut self.world);
        self.current_scene_path = None;
        self.scene_dirty = false;

        // Selection 초기화
        if let Some(ref mut sv) = self.scene_viewer {
            sv.selection.clear();
        }
        self.sync_hierarchy_state();
    }

    /// 씬 열기 대화상자 (미저장 확인 포함)
    fn open_scene_dialog(&mut self) {
        use super::scene_manager;

        if !scene_manager::prompt_save_if_dirty(
            self.scene_dirty,
            &self.current_scene_path,
            &mut self.world,
        ) {
            return; // 사용자가 취소
        }

        let file = rfd::FileDialog::new()
            .add_filter("SKOPE Scene", &["skope"])
            .add_filter("All Files", &["*"])
            .set_title("Open Scene")
            .pick_file();

        if let Some(path) = file {
            if scene_manager::load_scene_from_path(&mut self.world, &path) {
                self.current_scene_path = Some(path);
                self.scene_dirty = false;

                // Selection 초기화
                if let Some(ref mut sv) = self.scene_viewer {
                    sv.selection.clear();
                }
                self.sync_hierarchy_state();
            }
        }
    }

    /// Inspector 동기화
    fn sync_inspector_state(&mut self) {
        if let Some(ref sv) = self.scene_viewer {
            let selected = if sv.selection.entities.len() == 1 {
                Some(sv.selection.entities[0])
            } else {
                None
            };
            self.editor_ui_state.sync_inspector(&self.world, selected);
        }
    }

    /// Hierarchy 동기화
    fn sync_hierarchy_state(&mut self) {
        if let Some(ref sv) = self.scene_viewer {
            let selected_set: std::collections::HashSet<Entity> =
                sv.selection.entities.iter().cloned().collect();
            self.editor_ui_state.sync_hierarchy(&self.world, &selected_set);
        }
    }

    /// 엔티티 삭제
    fn handle_delete_entities(&mut self) {
        if let Some(ref mut sv) = self.scene_viewer {
            let entities: Vec<Entity> = sv.selection.entities.clone();
            if !entities.is_empty() {
                let cmd = Box::new(editor::command::DeleteCommand::new(entities.clone(), &self.world));
                self.command_stack.execute(cmd, &mut self.world);
                self.scene_dirty = true;
                sv.selection.clear();
                self.sync_hierarchy_state();
                log::info!("[Editor] Deleted {} entities", entities.len());
            }
        }
    }

    /// 부모 설정 (Ctrl+P)
    fn handle_parent_entities(&mut self) {
        if let Some(ref mut sv) = self.scene_viewer {
            let selection = &sv.selection.entities;
            if selection.len() >= 2 {
                let parent = selection[selection.len() - 1];
                let children: Vec<Entity> = selection[..selection.len() - 1].to_vec();
                for child in children {
                    if child == parent { continue; }
                    let old_parent = self.world.get::<Parent>(child).map(|p| p.get());
                    let cmd = editor::command::ReparentCommand::new(child, old_parent, Some(parent));
                    self.command_stack.execute(Box::new(cmd), &mut self.world);
                }
                self.scene_dirty = true;
                self.sync_hierarchy_state();
                log::info!("[Editor] Parented to {:?}", parent);
            }
        }
    }

    /// 부모 해제 (Alt+P)
    fn handle_unparent_entities(&mut self) {
        if let Some(ref mut sv) = self.scene_viewer {
            let mut count = 0;
            for &entity in &sv.selection.entities {
                if self.world.get::<Parent>(entity).is_some() {
                    let old_parent = self.world.get::<Parent>(entity).map(|p| p.get());
                    let cmd = editor::command::ReparentCommand::new(entity, old_parent, None);
                    self.command_stack.execute(Box::new(cmd), &mut self.world);
                    count += 1;
                }
            }
            if count > 0 {
                self.scene_dirty = true;
                self.sync_hierarchy_state();
                log::info!("[Editor] Unparented {} entities", count);
            }
        }
    }

    /// 숨기기 (H)
    fn handle_hide_selected(&mut self) {
        if let Some(ref mut sv) = self.scene_viewer {
            let entities: Vec<Entity> = sv.selection.entities.clone();
            for &entity in &entities {
                self.world.entity_mut(entity).insert(ecs_components::Hidden);
            }
            if !entities.is_empty() {
                sv.selection.clear();
                log::info!("[Editor] Hidden {} entities", entities.len());
            }
        }
    }

    /// 모든 숨김 해제 (Alt+H)
    fn handle_unhide_all(&mut self) {
        let hidden: Vec<Entity> = {
            let query = self.world.query_filtered::<Entity, With<ecs_components::Hidden>>();
            query.iter(&self.world).collect()
        };
        for &entity in &hidden {
            self.world.entity_mut(entity).remove::<ecs_components::Hidden>();
        }
        if !hidden.is_empty() {
            log::info!("[Editor] Unhidden {} entities", hidden.len());
        }
    }

    /// Isolate (Shift+H)
    fn handle_isolate(&mut self) {
        if let Some(ref sv) = self.scene_viewer {
            if sv.selection.entities.is_empty() { return; }
            let selected_set: std::collections::HashSet<Entity> =
                sv.selection.entities.iter().cloned().collect();
            let to_hide: Vec<Entity> = {
                let query = self.world.query_filtered::<Entity, With<ecs_components::MeshInstance>>();
                query.iter(&self.world).filter(|e| !selected_set.contains(e)).collect()
            };
            for &entity in &to_hide {
                self.world.entity_mut(entity).insert(ecs_components::Hidden);
            }
            if !to_hide.is_empty() {
                log::info!("[Editor] Isolated, hidden {} entities", to_hide.len());
            }
        }
    }

    /// 복제 (Shift+D)
    fn handle_duplicate(&mut self) {
        if let Some(ref mut sv) = self.scene_viewer {
            if sv.selection.entities.is_empty() { return; }
            self.clipboard.copy_from(&self.world, &sv.selection.entities);
            let center = sv.selection.center(&self.world).unwrap_or(glam::Vec3::ZERO);
            let new_entities = self.clipboard.paste_to(&mut self.world, center);
            sv.selection.set(new_entities.clone());
            sv.update_gizmo_from_selection(&self.world);
            self.scene_dirty = true;
            log::info!("[Editor] Duplicated {} entities", new_entities.len());
        }
    }

    /// 복제 with 오프셋 (Ctrl+D)
    fn handle_duplicate_with_offset(&mut self) {
        if let Some(ref mut sv) = self.scene_viewer {
            let entities: Vec<Entity> = sv.selection.entities.clone();
            let mut new_entities = Vec::new();
            for entity in &entities {
                if let Some(transform) = self.world.get::<ecs_components::Transform>(*entity) {
                    let mut new_t = transform.clone();
                    new_t.translation += glam::Vec3::new(1.0, 0.0, 1.0);
                    let mesh = self.world.get::<ecs_components::MeshInstance>(*entity).cloned();
                    let mat = self.world.get::<ecs_components::MaterialHandle>(*entity).cloned();
                    let name = self.world.get::<ecs_components::NodeName>(*entity)
                        .map(|n| ecs_components::NodeName(format!("{}_copy", n.0)));
                    let mut cmd = self.world.spawn((new_t, ecs_components::GlobalTransform::default()));
                    if let Some(m) = mesh { cmd.insert(m); }
                    if let Some(m) = mat { cmd.insert(m); }
                    if let Some(n) = name { cmd.insert(n); }
                    new_entities.push(cmd.id());
                }
            }
            if !new_entities.is_empty() {
                sv.selection.entities = new_entities.clone();
                sv.update_gizmo_from_selection(&self.world);
                self.scene_dirty = true;
                self.sync_hierarchy_state();
                log::info!("[Editor] Duplicated {} entities (Ctrl+D)", new_entities.len());
            }
        }
    }

    /// 복사 (Ctrl+C)
    fn handle_copy(&mut self) {
        if let Some(ref sv) = self.scene_viewer {
            if !sv.selection.entities.is_empty() {
                self.clipboard.copy_from(&self.world, &sv.selection.entities);
                log::info!("[Editor] Copied {} entities", self.clipboard.entities.len());
            }
        }
    }

    /// 붙여넣기 (Ctrl+V)
    fn handle_paste(&mut self) {
        if self.clipboard.is_empty() { return; }
        let paste_pos = self.scene_viewer.as_ref()
            .and_then(|sv| sv.selection.center(&self.world))
            .unwrap_or(glam::Vec3::ZERO);
        let pasted = self.clipboard.paste_to(&mut self.world, paste_pos);
        if !pasted.is_empty() {
            self.command_stack.push_executed(
                Box::new(editor::command::PasteCommand::new(pasted.clone()))
            );
            self.scene_dirty = true;
            if let Some(ref mut sv) = self.scene_viewer {
                sv.selection.entities = pasted.clone();
                sv.update_gizmo_from_selection(&self.world);
            }
            self.sync_hierarchy_state();
            log::info!("[Editor] Pasted {} entities", pasted.len());
        }
    }

    /// 잘라내기 (Ctrl+X)
    fn handle_cut(&mut self) {
        if let Some(ref mut sv) = self.scene_viewer {
            if sv.selection.entities.is_empty() { return; }
            self.clipboard.copy_from(&self.world, &sv.selection.entities);
            let count = sv.selection.entities.len();
            for entity in sv.selection.entities.drain(..) {
                if self.world.get_entity(entity).is_ok() {
                    self.world.despawn(entity);
                }
            }
            self.scene_dirty = true;
            self.sync_hierarchy_state();
            log::info!("[Editor] Cut {} entities", count);
        }
    }

    // ============ Multiplayer ============

    /// Host a multiplayer game on the specified port.
    fn host_game(&mut self, port: u16) {
        // Already hosting or connected? Disconnect first
        let is_active = self.world.get_resource::<skope_net::NetworkState>()
            .map(|s| s.peer_count > 0 || s.role == skope_net::SessionRole::Client)
            .unwrap_or(false);
        if is_active {
            self.disconnect_game();
        }

        let bind_addr = format!("0.0.0.0:{}", port);
        log::info!("[Multiplayer] Hosting on {}", bind_addr);

        let transport = skope_net::UdpTransport::new(skope_net::UdpTransportConfig {
            bind_addr,
            server_addr: None,
        });

        // Replace transport
        self.world.remove_non_send_resource::<skope_net::NetworkTransport>();
        self.world.insert_non_send_resource(skope_net::NetworkTransport::new(Box::new(transport)));

        // Ensure server role
        if let Some(ns) = self.world.get_resource_mut::<skope_net::NetworkState>() {
            ns.role = skope_net::SessionRole::Server;
        }

        // Spawn host player if no Player entity exists
        self.ensure_host_player();

        self.notification_manager.push(
            skope_ui::framework::NotificationLevel::Info,
            "Multiplayer",
            format!("Hosting on port {}", port),
        );
        log::info!("[Multiplayer] Server started on port {}", port);
    }

    /// Ensure the server host has a local player entity.
    /// Only spawns if no Player entity exists in the scene.
    fn ensure_host_player(&mut self) {
        use crate::ecs_components::{Transform, Player, Velocity};
        use crate::ecs_systems::player::PlayerController;

        // Check if any Player entity already exists
        let has_player = self.world.alive_entities().into_iter().any(|e| {
            self.world.get::<Player>(e).is_some()
        });
        if has_player {
            return;
        }

        let entity = self.world.spawn(Player::new(0))
            .insert(Transform::default())
            .insert(Velocity::default())
            .insert(PlayerController::default())
            .insert(skope_net::components::Replicated)
            .insert(skope_net::components::NetOwner(0))
            .insert(skope_net::components::NetRole::Authority)
            .id();

        if let Some(ns) = self.world.get_resource_mut::<skope_net::NetworkState>() {
            ns.peer_to_entity.insert(0, entity);
        }

        log::info!("[Multiplayer] Spawned host player entity {:?}", entity);
    }

    /// Join a remote game server.
    fn join_game(&mut self, addr: &str) {
        // Validate address format
        if addr.parse::<std::net::SocketAddr>().is_err() {
            log::error!("[Multiplayer] Invalid address: {}", addr);
            self.notification_manager.push(
                skope_ui::framework::NotificationLevel::Error,
                "Multiplayer",
                format!("Invalid address: {}", addr),
            );
            return;
        }

        // Disconnect if currently active
        let is_active = self.world.get_resource::<skope_net::NetworkState>()
            .map(|s| s.peer_count > 0 || s.role == skope_net::SessionRole::Client)
            .unwrap_or(false);
        if is_active {
            self.disconnect_game();
        }

        log::info!("[Multiplayer] Joining server at {}", addr);

        let transport = skope_net::UdpTransport::new(skope_net::UdpTransportConfig {
            bind_addr: "0.0.0.0:0".to_string(),
            server_addr: Some(addr.to_string()),
        });

        // Replace transport
        self.world.remove_non_send_resource::<skope_net::NetworkTransport>();
        self.world.insert_non_send_resource(skope_net::NetworkTransport::new(Box::new(transport)));

        // Switch to client role
        if let Some(ns) = self.world.get_resource_mut::<skope_net::NetworkState>() {
            ns.role = skope_net::SessionRole::Client;
            ns.peer_count = 1; // server is our peer
        }

        // Send Handshake to server (reliable — must not be lost)
        let handshake = skope_net::protocol::NetMessage::Handshake {
            client_name: "Player".to_string(),
        };
        if let Some(data) = handshake.to_bytes() {
            if let Some(transport) = self.world.get_non_send_resource_mut::<skope_net::NetworkTransport>() {
                if let Some(t) = transport.as_mut() {
                    t.send_reliable(0, data);
                }
            }
        }

        self.notification_manager.push(
            skope_ui::framework::NotificationLevel::Info,
            "Multiplayer",
            format!("Connecting to {}...", addr),
        );
        log::info!("[Multiplayer] Handshake sent to {}", addr);
    }

    /// Disconnect and return to solo mode.
    fn disconnect_game(&mut self) {
        let was_active = self.world.get_resource::<skope_net::NetworkState>()
            .map(|s| s.peer_count > 0 || s.role == skope_net::SessionRole::Client)
            .unwrap_or(false);

        log::info!("[Multiplayer] Disconnecting...");

        // Despawn remote player entities before resetting state
        let remote_entities: Vec<skope_ecs::prelude::Entity> = {
            let entities = self.world.alive_entities();
            entities.into_iter()
                .filter(|&e| {
                    self.world.get::<skope_net::components::NetRole>(e)
                        .map(|r| *r == skope_net::components::NetRole::SimulatedProxy)
                        .unwrap_or(false)
                })
                .collect()
        };
        for entity in &remote_entities {
            self.world.despawn(*entity);
        }

        // Remove existing transport
        self.world.remove_non_send_resource::<skope_net::NetworkTransport>();
        self.world.insert_non_send_resource(skope_net::NetworkTransport::default());

        // Reset network state to solo server
        if let Some(ns) = self.world.get_resource_mut::<skope_net::NetworkState>() {
            ns.role = skope_net::SessionRole::Server;
            ns.peer_count = 0;
            ns.connected_peers.clear();
            ns.local_peer_id = 0;
            ns.peer_to_entity.clear();
        }

        // Clean up connection tracker
        if let Some(ct) = self.world.get_resource_mut::<skope_net::ConnectionTracker>() {
            ct.connections.clear();
        }

        if was_active {
            self.notification_manager.push(
                skope_ui::framework::NotificationLevel::Info,
                "Multiplayer",
                "Disconnected",
            );
        }
        log::info!("[Multiplayer] Disconnected — solo mode");
    }

    /// Debug UI 통계 업데이트 + 콘솔 커맨드 처리
    ///
    /// Step 0: State::render()에서 게임 로직을 분리하여 GT update()에서 실행.
    /// render()는 순수 GPU 커맨드 인코딩만 남긴다.
    fn process_console_and_debug_ui(&mut self) {
        static FRAME_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let frame = FRAME_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Debug UI 통계 업데이트
        let (delta_seconds, elapsed_seconds) = self.world.get_resource::<ecs_resources::Time>()
            .map(|t| (t.delta_seconds, t.elapsed_seconds))
            .unwrap_or((0.016, 0.0));
        self.debug_ui.update_stats(delta_seconds);
        self.debug_ui.elapsed_time = elapsed_seconds;

        // 카메라 정보 업데이트
        if let Some(ref sv) = self.scene_viewer {
            self.debug_ui.camera_pos = sv.camera.position;
            self.debug_ui.camera_yaw = sv.camera.yaw();
            self.debug_ui.camera_pitch = sv.camera.pitch();
        }

        // 엔티티 목록 업데이트 (60프레임 주기)
        if frame % 60 == 0 || self.debug_ui.entities.is_empty() {
            self.debug_ui.entities = debug::ui::collect_entity_info(&mut self.world);
        }

        // 콘솔 커맨드 처리
        if let Some(action) = self.debug_ui.take_action() {
            match action {
                debug::ui::ConsoleAction::ReloadScene => {
                    let default_level = format!("{}/start.skope", paths::game::LEVELS);
                    let level_path = std::env::var("SKOPE_LEVEL")
                        .unwrap_or(default_level);

                    // 1. 기존 씬 엔티티 수집 (카메라 제외)
                    let to_despawn: Vec<Entity> = {
                        let query = self.world.query::<Entity>();
                        query.iter(&self.world)
                            .filter(|e| {
                                self.world.get::<ecs_components::Camera>(*e).is_none()
                            })
                            .collect()
                    };

                    // 2. 엔티티 제거
                    let despawn_count = to_despawn.len();
                    for entity in to_despawn {
                        self.world.despawn(entity);
                    }

                    // 3. 새 씬 로드
                    match crate::scene::load_from_file(&mut self.world, std::path::Path::new(&level_path)) {
                        Ok(()) => {
                            skope_data::process_pending_colliders(&mut self.world);
                            self.debug_ui.log(
                                debug::ui::LogLevel::Info,
                                &format!("Reloaded scene: removed {} entities", despawn_count),
                                self.debug_ui.elapsed_time,
                            );
                            self.debug_ui.entities = debug::ui::collect_entity_info(&mut self.world);
                        }
                        Err(e) => {
                            self.debug_ui.log(
                                debug::ui::LogLevel::Error,
                                &format!("Failed to reload scene: {}", e),
                                self.debug_ui.elapsed_time,
                            );
                        }
                    }
                }
                debug::ui::ConsoleAction::ExecuteLua(code) => {
                    if let Some(engine) = self.world.get_non_send_resource::<scripting::ScriptEngine>() {
                        match engine.exec(&code) {
                            Ok(result) => {
                                if !result.is_empty() {
                                    self.debug_ui.log(debug::ui::LogLevel::Info, &result, self.debug_ui.elapsed_time);
                                } else {
                                    self.debug_ui.log(debug::ui::LogLevel::Info, "OK", self.debug_ui.elapsed_time);
                                }
                            }
                            Err(e) => {
                                self.debug_ui.log(debug::ui::LogLevel::Error, &format!("Lua error: {}", e), self.debug_ui.elapsed_time);
                            }
                        }
                    } else {
                        self.debug_ui.log(debug::ui::LogLevel::Error, "Lua engine not available", self.debug_ui.elapsed_time);
                    }
                }
                debug::ui::ConsoleAction::SpawnEntity(name) => {
                    let prefab_data = self.world
                        .get_resource::<prefab::PrefabRegistry>()
                        .and_then(|registry| registry.get(&name).cloned());

                    if let Some(data) = prefab_data {
                        let entity = prefab::spawn_prefab_entity(&mut self.world, &data.root, glam::Vec3::ZERO);
                        self.debug_ui.log(
                            debug::ui::LogLevel::Info,
                            &format!("Spawned prefab '{}' (ID: {})", name, entity.to_bits() & 0xFFFF),
                            self.debug_ui.elapsed_time,
                        );
                    } else {
                        let entity = self.world.spawn((
                            ecs_components::Transform::from_translation(glam::Vec3::ZERO),
                            ecs_components::NodeName(name.clone()),
                        )).id();
                        self.debug_ui.log(
                            debug::ui::LogLevel::Info,
                            &format!("Spawned entity '{}' (ID: {})", name, entity.to_bits() & 0xFFFF),
                            self.debug_ui.elapsed_time,
                        );
                    }
                    self.debug_ui.entities = debug::ui::collect_entity_info(&mut self.world);
                }
                debug::ui::ConsoleAction::SpawnParticle(effect_type) => {
                    // 카메라 전방 3m에 파티클 스폰
                    let (camera_pos, forward) = if let Some(ref sv) = self.scene_viewer {
                        let view = sv.camera.view_matrix();
                        let fwd = -glam::Vec3::new(view.col(2).x, view.col(2).y, view.col(2).z);
                        (sv.camera.position, fwd)
                    } else {
                        (glam::Vec3::ZERO, -glam::Vec3::Z)
                    };
                    let spawn_pos = camera_pos + forward * 3.0;
                    let emitter = match effect_type.as_str() {
                        "fire" => particles::ParticleEmitter::fire(),
                        "smoke" => particles::ParticleEmitter::smoke(),
                        "explosion" => particles::ParticleEmitter::explosion(),
                        "sparkle" => particles::ParticleEmitter::sparkle(),
                        _ => particles::ParticleEmitter::fire(),
                    };
                    let entity = self.world.spawn((
                        ecs_components::Transform::from_translation(spawn_pos),
                        ecs_components::NodeName(format!("Particle_{}", effect_type)),
                        emitter,
                    )).id();
                    self.debug_ui.log(
                        debug::ui::LogLevel::Info,
                        &format!("Spawned {} particles at {:?} (ID: {})", effect_type, spawn_pos, entity.to_bits() & 0xFFFF),
                        self.debug_ui.elapsed_time,
                    );
                    self.debug_ui.entities = debug::ui::collect_entity_info(&mut self.world);
                }
            }
        }
    }

    /// Update status bar with network connection info.
    fn update_network_status_bar(&mut self) {
        let (role, peer_count, local_peer_id) = match self.world.get_resource::<skope_net::NetworkState>() {
            Some(ns) => (ns.role, ns.peer_count, ns.local_peer_id),
            None => return,
        };

        let status = match role {
            skope_net::SessionRole::Server => {
                if peer_count > 0 {
                    format!("Hosting ({} peer{})", peer_count, if peer_count == 1 { "" } else { "s" })
                } else {
                    String::new() // Solo — don't clutter status bar
                }
            }
            skope_net::SessionRole::Client => {
                if local_peer_id != 0 {
                    format!("Connected (peer #{})", local_peer_id)
                } else {
                    "Connecting...".to_string()
                }
            }
        };

        self.editor_ui_state.dock_panel.status_right_text = status;

        // Check for transport errors (e.g. bind failure)
        if let Some(transport) = self.world.get_non_send_resource_mut::<skope_net::NetworkTransport>() {
            if let Some(t) = transport.as_mut() {
                if let Some(err) = t.take_error() {
                    log::error!("[Multiplayer] Transport error: {}", err);
                    self.notification_manager.push(
                        skope_ui::framework::NotificationLevel::Error,
                        "Network Error",
                        err,
                    );
                    // Auto-disconnect on transport error
                }
            }
        }
    }
}
