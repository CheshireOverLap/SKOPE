//! Dear ImGui Backend for SKOPE Editor
//!
//! wgpu + winit 기반의 ImGui 렌더링 백엔드
//! 도킹 + Multi-Viewport 지원

use std::sync::Arc;
use wgpu;
use winit::window::Window;

use dear_imgui_rs::{self as imgui, Context, ConfigFlags, StyleColor, Direction, FontSource, FontConfig};
use dear_imgui_wgpu::{WgpuRenderer, WgpuInitInfo};
use dear_imgui_winit::{WinitPlatform, HiDpiMode};

/// ImGui 백엔드 상태
pub struct ImGuiBackend {
    /// ImGui 컨텍스트
    pub context: Context,
    /// winit 플랫폼 통합
    pub platform: WinitPlatform,
    /// wgpu 렌더러
    pub renderer: WgpuRenderer,
}

impl ImGuiBackend {
    /// 새 ImGui 백엔드 생성
    pub fn new(
        window: &Window,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        surface_format: wgpu::TextureFormat,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // ImGui 컨텍스트 생성
        let mut context = Context::create();

        // 도킹 및 키보드 네비게이션 활성화
        {
            let io = context.io_mut();
            let mut flags = io.config_flags();
            flags.insert(ConfigFlags::DOCKING_ENABLE);
            flags.insert(ConfigFlags::NAV_ENABLE_KEYBOARD);

            // Multi-Viewport 비활성화 (Windows 이벤트 루프 문제 원인 가능성 테스트)
            // TODO: 문제 해결 후 다시 활성화
            // flags.insert(ConfigFlags::VIEWPORTS_ENABLE);

            io.set_config_flags(flags);
        }

        // NOTE: 폰트 설정은 dear-imgui-rs 0.7의 assertion 이슈로 인해 스킵
        // Self::setup_fonts(&mut context)?;
        log::info!("[ImGui] Using default font (skipped custom font setup)");

        // 스타일 설정 (Unreal Engine 스타일)
        Self::setup_style(&mut context);

        // winit 플랫폼 초기화
        let mut platform = WinitPlatform::new(&mut context);
        platform.attach_window(window, HiDpiMode::Default, &mut context);

        // wgpu 렌더러 초기화
        let init_info = WgpuInitInfo::new(
            device.as_ref().clone(),
            queue.as_ref().clone(),
            surface_format,
        );
        let renderer = WgpuRenderer::new(init_info, &mut context)?;

        Ok(Self {
            context,
            platform,
            renderer,
        })
    }

    /// 폰트 설정 (기본 폰트만 - CJK 폰트는 추후 지원)
    fn setup_fonts(context: &mut Context) -> Result<(), Box<dyn std::error::Error>> {
        // NOTE: dear-imgui 1.92+ 동적 폰트 시스템에서 TTC 로딩에 문제가 있어
        // 일단 기본 폰트만 사용. CJK 지원은 추후 개선 필요.

        let mut font_atlas = context.fonts();

        // 기본 폰트 (ProggyClean) - 더 큰 사이즈로
        font_atlas.add_font(&[FontSource::DefaultFontData {
            size_pixels: Some(15.0),
            config: None,
        }]);

        log::info!("[ImGui] Font setup complete (default font only)");
        Ok(())
    }

    /// 스타일 설정 (Unreal Engine 스타일 다크 테마)
    fn setup_style(context: &mut Context) {
        let style = context.style_mut();

        // 배경색
        style.set_color(StyleColor::WindowBg, [0.1, 0.1, 0.1, 1.0]);
        style.set_color(StyleColor::ChildBg, [0.08, 0.08, 0.08, 1.0]);
        style.set_color(StyleColor::PopupBg, [0.12, 0.12, 0.12, 0.95]);

        // 타이틀바
        style.set_color(StyleColor::TitleBg, [0.08, 0.08, 0.08, 1.0]);
        style.set_color(StyleColor::TitleBgActive, [0.12, 0.12, 0.12, 1.0]);
        style.set_color(StyleColor::TitleBgCollapsed, [0.05, 0.05, 0.05, 0.5]);

        // 탭
        style.set_color(StyleColor::Tab, [0.12, 0.12, 0.12, 1.0]);
        style.set_color(StyleColor::TabHovered, [0.2, 0.2, 0.2, 1.0]);
        style.set_color(StyleColor::TabSelected, [0.18, 0.18, 0.18, 1.0]);
        style.set_color(StyleColor::TabSelectedOverline, [0.26, 0.59, 0.98, 1.0]);

        // 도킹
        style.set_color(StyleColor::DockingPreview, [0.26, 0.59, 0.98, 0.7]);
        style.set_color(StyleColor::DockingEmptyBg, [0.05, 0.05, 0.05, 1.0]);

        // 버튼
        style.set_color(StyleColor::Button, [0.2, 0.2, 0.2, 1.0]);
        style.set_color(StyleColor::ButtonHovered, [0.28, 0.28, 0.28, 1.0]);
        style.set_color(StyleColor::ButtonActive, [0.35, 0.35, 0.35, 1.0]);

        // 헤더
        style.set_color(StyleColor::Header, [0.2, 0.2, 0.2, 1.0]);
        style.set_color(StyleColor::HeaderHovered, [0.26, 0.59, 0.98, 0.8]);
        style.set_color(StyleColor::HeaderActive, [0.26, 0.59, 0.98, 1.0]);

        // 프레임
        style.set_color(StyleColor::FrameBg, [0.16, 0.16, 0.16, 1.0]);
        style.set_color(StyleColor::FrameBgHovered, [0.22, 0.22, 0.22, 1.0]);
        style.set_color(StyleColor::FrameBgActive, [0.28, 0.28, 0.28, 1.0]);

        // 스크롤바
        style.set_color(StyleColor::ScrollbarBg, [0.05, 0.05, 0.05, 0.5]);
        style.set_color(StyleColor::ScrollbarGrab, [0.3, 0.3, 0.3, 1.0]);
        style.set_color(StyleColor::ScrollbarGrabHovered, [0.4, 0.4, 0.4, 1.0]);
        style.set_color(StyleColor::ScrollbarGrabActive, [0.5, 0.5, 0.5, 1.0]);

        // 테두리
        style.set_color(StyleColor::Border, [0.2, 0.2, 0.2, 0.5]);
        style.set_color(StyleColor::BorderShadow, [0.0, 0.0, 0.0, 0.0]);

        // 둥근 모서리
        style.set_window_rounding(4.0);
        style.set_frame_rounding(2.0);
        style.set_tab_rounding(4.0);
        style.set_scrollbar_rounding(2.0);
        style.set_grab_rounding(2.0);

        // 패딩
        style.set_window_padding([8.0, 8.0]);
        style.set_frame_padding([4.0, 3.0]);
        style.set_item_spacing([8.0, 4.0]);
        style.set_item_inner_spacing([4.0, 4.0]);

        // 도킹 분리 버튼 위치
        style.set_window_menu_button_position(Direction::Right);
    }

    /// 프레임 시작 (이벤트 처리 후, UI 코드 전에 호출)
    pub fn begin_frame(&mut self, window: &Window, delta_time: f32) {
        // 델타 타임 설정
        self.context.io_mut().set_delta_time(delta_time);

        // winit 이벤트 처리 완료 후 프레임 시작 준비
        self.platform.prepare_frame(window, &mut self.context);

        // prepare_frame 이후 display_size 강제 업데이트 (HiDPI 대응)
        let physical_size = window.inner_size();
        let scale_factor = window.scale_factor() as f32;
        let logical_size = [
            physical_size.width as f32 / scale_factor,
            physical_size.height as f32 / scale_factor,
        ];
        self.context.io_mut().set_display_size(logical_size);
        self.context.io_mut().set_display_framebuffer_scale([scale_factor, scale_factor]);
    }

    /// 새 프레임 시작 (UI 코드 시작)
    pub fn new_frame(&mut self) -> &imgui::Ui {
        self.context.frame()
    }

    /// winit 이벤트 처리
    pub fn handle_event(&mut self, window: &Window, event: &winit::event::WindowEvent) -> bool {
        self.platform.handle_window_event(&mut self.context, window, event);

        // HiDPI 리사이즈 시 ImGui display_size 강제 업데이트
        if let winit::event::WindowEvent::Resized(physical_size) = event {
            let scale_factor = window.scale_factor() as f32;
            let logical_size = [
                physical_size.width as f32 / scale_factor,
                physical_size.height as f32 / scale_factor,
            ];
            self.context.io_mut().set_display_size(logical_size);
            self.context.io_mut().set_display_framebuffer_scale([scale_factor, scale_factor]);
            log::debug!("[ImGui] Resized: physical={}x{}, logical={:.0}x{:.0}, scale={}",
                physical_size.width, physical_size.height,
                logical_size[0], logical_size[1], scale_factor);
        }

        self.context.io().want_capture_mouse() || self.context.io().want_capture_keyboard()
    }

    /// 렌더링
    pub fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        window: &Window,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 렌더 준비
        self.platform.prepare_render(&mut self.context, window);

        // 드로우 데이터 가져오기
        let draw_data = self.context.render();

        // 렌더 패스 생성
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ImGui Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load, // 기존 내용 유지 (3D 씬 위에 그리기)
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        // ImGui 렌더링
        self.renderer.render_draw_data(draw_data, &mut render_pass)?;

        Ok(())
    }
}

/// ImGui 컨텍스트 초기화
pub fn init_imgui() -> Context {
    let mut context = Context::create();

    // 기본 설정
    {
        let io = context.io_mut();
        let mut flags = io.config_flags();
        flags.insert(ConfigFlags::DOCKING_ENABLE);
        flags.insert(ConfigFlags::NAV_ENABLE_KEYBOARD);

        // Multi-Viewport 비활성화 (Windows 이벤트 루프 문제 원인 가능성 테스트)
        // flags.insert(ConfigFlags::VIEWPORTS_ENABLE);
        io.set_config_flags(flags);
    }

    context
}
