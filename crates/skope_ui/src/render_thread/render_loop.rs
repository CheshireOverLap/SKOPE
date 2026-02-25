//! RT 메인 루프 + SurfaceManager
//!
//! 렌더 스레드의 메인 이벤트 루프. 커맨드를 수신하여 처리.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::Instant;
use winit::window::WindowId;

use crate::render::{RSlateRenderer, SlateRenderResources};
use super::{DrawWindowsData, FrameCompleteSignal, RenderCommand, RenderConfig, RenderThreadInitData};
use super::profiling::{RtProfilingStats, RtProfilingAccumulator};

/// Surface 상태 관리 (RT 전용)
struct SurfaceState {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    /// per-window UI renderer (Send+Sync — wgpu 리소스만 보유)
    renderer: RSlateRenderer,
}

/// RT의 Surface 매니저
///
/// 모든 윈도우의 Surface를 관리. GT에서 `AddSurface`로 전달받아 등록.
/// Device 참조는 소유하지 않으며, 각 메서드에 `&wgpu::Device`로 전달받음.
struct SurfaceManager {
    surfaces: HashMap<WindowId, SurfaceState>,
    /// Feature 4: 마지막으로 확인한 리소스 버전 (캐시 무효화 추적)
    last_resource_version: u64,
}

impl SurfaceManager {
    fn new() -> Self {
        Self {
            surfaces: HashMap::new(),
            last_resource_version: 0,
        }
    }

    fn add(
        &mut self,
        window_id: WindowId,
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
        device: &wgpu::Device,
        shared: &SlateRenderResources,
    ) {
        surface.configure(device, &config);
        let renderer = RSlateRenderer::new_viewport(
            device,
            shared,
            config.width,
            config.height,
        );
        let w = config.width;
        let h = config.height;
        self.surfaces.insert(window_id, SurfaceState { surface, config, renderer });
        log::info!("[SurfaceManager] Added surface for {:?} ({}x{})", window_id, w, h);
    }

    fn remove(&mut self, window_id: WindowId) {
        if self.surfaces.remove(&window_id).is_some() {
            log::info!("[SurfaceManager] Removed surface for {:?}", window_id);
        }
    }

    fn resize(
        &mut self,
        window_id: WindowId,
        width: u32,
        height: u32,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        if let Some(state) = self.surfaces.get_mut(&window_id) {
            if width > 0 && height > 0 {
                state.config.width = width;
                state.config.height = height;
                state.surface.configure(device, &state.config);
                state.renderer.resize(queue, width, height);
                log::debug!(
                    "[SurfaceManager] Resized {:?} to {}x{}",
                    window_id, width, height
                );
            }
        }
    }

    /// PresentMode 변경 (Feature 5: Config RT 전파)
    fn update_present_mode(&mut self, mode: wgpu::PresentMode, device: &wgpu::Device) {
        for state in self.surfaces.values_mut() {
            state.config.present_mode = mode;
            state.surface.configure(device, &state.config);
        }
        log::info!("[SurfaceManager] PresentMode updated to {:?}", mode);
    }

    /// 모든 윈도우 UI 렌더링 (DrawWindowsData 처리)
    fn draw_all(
        &mut self,
        data: &DrawWindowsData,
        shared: &mut SlateRenderResources,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        // Feature 4: 리소스 버전 변경 시 모든 렌더러 캐시 무효화
        // (UE5 WindowElementListsPool.Empty() 패턴)
        let current_rv = shared.resource_version();
        if current_rv != self.last_resource_version {
            log::info!(
                "[RT] Resource version changed {} → {} — invalidating renderer caches",
                self.last_resource_version, current_rv
            );
            for state in self.surfaces.values_mut() {
                state.renderer.invalidate_cache();
            }
            self.last_resource_version = current_rv;
        }

        for window_data in &data.windows {
            let Some(state) = self.surfaces.get_mut(&window_data.window_id) else {
                log::warn!("[SurfaceManager] No surface for {:?}, skipping", window_data.window_id);
                continue;
            };

            // 리사이즈 처리
            if let Some((w, h)) = window_data.surface_resize {
                if w > 0 && h > 0 && (w != state.config.width || h != state.config.height) {
                    state.config.width = w;
                    state.config.height = h;
                    state.surface.configure(device, &state.config);
                    state.renderer.resize(queue, w, h);
                }
            }

            // Surface output 획득
            let output = match state.surface.get_current_texture() {
                Ok(output) => output,
                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                    state.surface.configure(device, &state.config);
                    match state.surface.get_current_texture() {
                        Ok(output) => output,
                        Err(e) => {
                            log::warn!("[RT] Surface retry failed for {:?}: {:?}", window_data.window_id, e);
                            continue;
                        }
                    }
                }
                Err(e) => {
                    log::warn!("[RT] Surface error for {:?}: {:?}", window_data.window_id, e);
                    continue;
                }
            };

            let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("RT Draw Encoder"),
            });

            // 배경 클리어
            {
                let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("RT Clear Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: window_data.clear_color[0],
                                g: window_data.clear_color[1],
                                b: window_data.clear_color[2],
                                a: window_data.clear_color[3],
                            }),
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

            // 텍스처 사전 로드 (lazy load — DrawElementList에서 참조하는 이미지)
            shared.ensure_textures_loaded(device, queue, &window_data.draw_elements);

            // DrawElementList → tessellate → render
            state.renderer.render_elements_with_shared(
                shared,
                device,
                queue,
                &mut encoder,
                &view,
                &window_data.draw_elements,
            );

            queue.submit(std::iter::once(encoder.finish()));
            output.present();
        }
    }
}

/// RT 메인 루프
///
/// 커맨드 큐에서 렌더링 커맨드를 수신하여 순차 처리.
/// `Shutdown` 수신 시 루프 종료.
pub(crate) fn render_thread_main(
    init: RenderThreadInitData,
    cmd_rx: Receiver<RenderCommand>,
    frame_signal: Arc<FrameCompleteSignal>,
    profiling_stats: Arc<RtProfilingStats>,
) {
    let device: Arc<wgpu::Device> = init.device;
    let queue: Arc<wgpu::Queue> = init.queue;
    let _format = init.format;
    let mut surface_mgr = SurfaceManager::new();

    // 공유 렌더링 리소스 (폰트 아틀라스 등)
    let mut shared_resources = init.shared_resources;

    // 메인 윈도우 Surface 등록
    if let Some((window_id, surface, config)) = init.main_surface {
        if let Some(ref shared) = shared_resources {
            surface_mgr.add(window_id, surface, config, &device, shared);
        }
    }

    // 3D 씬 렌더러 (InitSceneRenderer로 설정)
    let mut scene_renderer: Option<Box<dyn super::SceneRenderer>> = None;

    let mut frame_number: u64 = 0;

    // Feature 5: RT 설정
    let mut config = RenderConfig::default();

    // Feature 3: 프로파일링 누산기
    let mut profiler = RtProfilingAccumulator::new();

    loop {
        profiler.begin_idle();
        let cmd = match cmd_rx.recv() {
            Ok(cmd) => cmd,
            Err(_) => {
                log::info!("[RenderThread] Command channel closed -- exiting");
                break;
            }
        };
        profiler.end_idle();

        let cmd_start = Instant::now();
        let cmd_name = match &cmd {
            RenderCommand::DrawWindows(_) => "DrawWindows",
            RenderCommand::RenderScene(_) => "RenderScene",
            RenderCommand::InitSceneRenderer(_) => "InitSceneRenderer",
            RenderCommand::RegisterViewportTextures(_) => "RegisterViewportTextures",
            RenderCommand::AddSurface { .. } => "AddSurface",
            RenderCommand::RemoveSurface { .. } => "RemoveSurface",
            RenderCommand::ResizeSurface { .. } => "ResizeSurface",
            RenderCommand::UpdateConfig(_) => "UpdateConfig",
            RenderCommand::SignalFence { .. } => "SignalFence",
            RenderCommand::Shutdown => "Shutdown",
        };

        match cmd {
            RenderCommand::InitSceneRenderer(renderer) => {
                log::info!("[RT] SceneRenderer initialized");
                scene_renderer = Some(renderer);
            }
            RenderCommand::RenderScene(data) => {
                if let Some(ref mut renderer) = scene_renderer {
                    renderer.render(data);
                    // 렌더링 후 viewport 텍스처를 UI shared_resources에 등록
                    if let Some(ref mut shared) = shared_resources {
                        for vt in renderer.viewport_texture_infos() {
                            let needs_update = shared.get_texture_size(&vt.name) != Some(vt.size);
                            if needs_update {
                                shared.register_external_texture(&device, &vt.name, &vt.view, vt.size);
                            }
                        }
                    }
                }
            }
            RenderCommand::RegisterViewportTextures(textures) => {
                if let Some(ref mut shared) = shared_resources {
                    for vt in &textures {
                        let needs_update = shared.get_texture_size(&vt.name) != Some(vt.size);
                        if needs_update {
                            shared.register_external_texture(&device, &vt.name, &vt.view, vt.size);
                        }
                    }
                }
            }
            RenderCommand::DrawWindows(data) => {
                // Feature 5: debug_draw_stats 로그
                if config.debug_draw_stats {
                    log::debug!("[RT] DrawWindows: {}x windows, frame #{}", data.windows.len(), data.frame_number);
                }
                if let Some(ref mut shared) = shared_resources {
                    surface_mgr.draw_all(&data, shared, &device, &queue);
                } else {
                    log::warn!("[RT] DrawWindows received but no shared_resources");
                }
            }
            RenderCommand::AddSurface {
                window_id,
                surface,
                config: surf_config,
            } => {
                if let Some(ref shared) = shared_resources {
                    surface_mgr.add(window_id, surface, surf_config, &device, shared);
                }
            }
            RenderCommand::RemoveSurface { window_id } => {
                surface_mgr.remove(window_id);
            }
            RenderCommand::ResizeSurface {
                window_id,
                width,
                height,
            } => {
                surface_mgr.resize(window_id, width, height, &device, &queue);
            }
            RenderCommand::UpdateConfig(new_config) => {
                if new_config.present_mode != config.present_mode {
                    surface_mgr.update_present_mode(new_config.present_mode, &device);
                }
                // Feature 4: force_texture_invalidate 처리
                if new_config.force_texture_invalidate {
                    if let Some(ref mut shared) = shared_resources {
                        shared.invalidate_all_textures();
                        log::info!("[RT] Texture invalidation triggered via config");
                    }
                }
                config = new_config;
                // 일회성 플래그 리셋
                config.force_texture_invalidate = false;
                log::debug!("[RT] Config updated: {:?}", config);
            }
            RenderCommand::SignalFence {
                fence_id,
                signal,
            } => {
                let (lock, cvar) = &*signal;
                let mut signaled = lock.lock().unwrap();
                *signaled = true;
                cvar.notify_one();
                log::trace!("[RenderThread] Fence {} signaled", fence_id);
            }
            RenderCommand::Shutdown => {
                log::info!("[RenderThread] Shutdown received");
                break;
            }
        }

        // Feature 3: 명령 프로파일링
        profiler.record_command(cmd_name, cmd_start.elapsed());

        // Feature 2: DrawWindows = 논리 프레임 종료 마커
        // DrawWindows 완료 시에만 frame_number 증가 + GT 깨우기
        // (모든 커맨드마다 signal하면 RT frame_number >> GT frame_number → frame_gate 데드코드화)
        if cmd_name == "DrawWindows" {
            frame_number += 1;
            frame_signal.signal(frame_number);
            // Feature 3: 프로파일링 publish
            profiler.publish(&profiling_stats, frame_number);
        }
    }
}
