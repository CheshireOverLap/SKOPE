//! Dear ImGui Backend for SKOPE Editor
//!
//! wgpu + winit 기반의 ImGui 렌더링 백엔드
//! 도킹 + Multi-Viewport 지원
//! FreeType 기반 고품질 폰트 렌더링

use std::sync::Arc;
use std::path::Path;
use wgpu;
use winit::window::Window;

use dear_imgui_rs::{self as imgui, Context, ConfigFlags, StyleColor, Direction, FontSource, FontConfig};
use dear_imgui_wgpu::{WgpuRenderer, WgpuInitInfo, multi_viewport as wgpu_mv};
use dear_imgui_winit::{WinitPlatform, HiDpiMode, multi_viewport};
use winit::window::WindowId;

/// 기본 폰트 크기 (논리적 픽셀)
const BASE_FONT_SIZE: f32 = 15.0;

/// ImGui 백엔드 상태
pub struct ImGuiBackend {
    /// ImGui 컨텍스트
    pub context: Context,
    /// winit 플랫폼 통합
    pub platform: WinitPlatform,
    /// wgpu 렌더러
    pub renderer: WgpuRenderer,
    /// 프레임 카운터 (Multi-Viewport 안정화용)
    frame_count: u32,
}

impl ImGuiBackend {
    /// 새 ImGui 백엔드 생성
    ///
    /// Multi-Viewport 지원을 위해 Instance와 Adapter를 전달받습니다.
    /// Instance는 소유권을 가져가며, 보조 윈도우 Surface 생성에 사용됩니다.
    pub fn new(
        window: &Window,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        surface_format: wgpu::TextureFormat,
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // ImGui 컨텍스트 생성
        let mut context = Context::create();

        // 도킹 및 키보드 네비게이션 활성화
        {
            let io = context.io_mut();
            let mut flags = io.config_flags();
            flags.insert(ConfigFlags::DOCKING_ENABLE);
            flags.insert(ConfigFlags::NAV_ENABLE_KEYBOARD);
            io.set_config_flags(flags);
        }

        // Multi-Viewport 활성화 (dear-imgui-rs 권장 방식)
        // VIEWPORTS_ENABLE + DOCKING_ENABLE 자동 설정
        context.enable_multi_viewport();
        log::info!("[ImGui] Multi-Viewport enabled via enable_multi_viewport()");

        // FreeType 기반 고품질 폰트 설정
        let scale_factor = window.scale_factor() as f32;
        Self::setup_fonts_freetype(&mut context, scale_factor)?;
        log::info!("[ImGui] FreeType font setup complete (scale={})", scale_factor);

        // 스타일 설정 (Unreal Engine 스타일)
        Self::setup_style(&mut context);

        // winit 플랫폼 초기화
        let mut platform = WinitPlatform::new(&mut context);
        platform.attach_window(window, HiDpiMode::Default, &mut context);

        // Multi-Viewport 플랫폼 지원 초기화 (winit 콜백 등록)
        multi_viewport::init_multi_viewport_support(&mut context, window);
        log::info!("[ImGui] Multi-Viewport platform support initialized");

        // wgpu 렌더러 초기화 (Multi-Viewport를 위해 Instance, Adapter 전달)
        log::info!("[ImGui] Using provided Instance and Adapter for Multi-Viewport");

        let init_info = WgpuInitInfo::new(
            device.as_ref().clone(),
            queue.as_ref().clone(),
            surface_format,
        )
        .with_instance(instance)
        .with_adapter(adapter);

        let renderer = WgpuRenderer::new(init_info, &mut context)?;

        // Note: wgpu_mv::enable()는 ImGuiBackend가 최종 위치에 저장된 후 호출해야 함
        // renderer 포인터가 저장되기 때문에 이동하면 무효화됨
        // enable_multi_viewport_callbacks()를 별도로 호출할 것
        log::info!("[ImGui] Multi-Viewport renderer created (callbacks not yet enabled)");

        Ok(Self {
            context,
            platform,
            renderer,
            frame_count: 0,
        })
    }

    /// Multi-Viewport 콜백 활성화
    ///
    /// 중요: ImGuiBackend가 최종 메모리 위치에 저장된 후 호출해야 합니다.
    /// wgpu_mv::enable()이 renderer의 raw pointer를 저장하기 때문에,
    /// ImGuiBackend가 이동하면 포인터가 무효화됩니다.
    pub fn enable_multi_viewport_callbacks(&mut self) {
        wgpu_mv::enable(&mut self.renderer, &mut self.context);
        log::info!("[ImGui] Multi-Viewport renderer callbacks enabled");
    }

    /// 보조 뷰포트(떼어낸 윈도우)의 배경색 설정
    pub fn set_viewport_clear_color(&mut self, r: f64, g: f64, b: f64, a: f64) {
        self.renderer.set_viewport_clear_color(wgpu::Color { r, g, b, a });
    }

    /// FreeType 기반 고품질 폰트 설정
    ///
    /// HiDPI 대응:
    /// 1. 물리적으로 큰 폰트를 굽고 (font_size * scale_factor)
    /// 2. font_global_scale로 다시 줄임 (1.0 / scale_factor)
    fn setup_fonts_freetype(context: &mut Context, scale_factor: f32) -> Result<(), Box<dyn std::error::Error>> {
        // 물리적 폰트 크기 계산
        let font_size_pixels = BASE_FONT_SIZE * scale_factor;

        let mut fonts = context.fonts();

        // 시스템 폰트 로드 시도
        let font_loaded = Self::try_load_system_font(&mut fonts, font_size_pixels);

        if !font_loaded {
            fonts.add_font(&[FontSource::default_font_with_size(font_size_pixels)]);
            log::info!("[ImGui] Using default font (system fonts not found)");
        }

        // 한글 폰트 병합 - 임시 비활성화
        // FontConfig::merge_mode() 사용 시 assertion 실패 문제
        // TODO: dear-imgui-rs 업데이트 후 다시 활성화
        // Self::try_merge_korean_font(&mut fonts, font_size_pixels);
        log::info!("[ImGui] Korean font merge disabled (FontConfig compatibility issue)");

        drop(fonts);
        context.io_mut().set_font_global_scale(1.0 / scale_factor);

        Ok(())
    }

    /// 시스템 폰트 로드 시도
    fn try_load_system_font(fonts: &mut imgui::FontAtlas, font_size: f32) -> bool {
        // Windows 시스템 폰트 경로들
        let system_font_paths = [
            "C:/Windows/Fonts/segoeui.ttf",      // Segoe UI (Windows 기본)
            "C:/Windows/Fonts/consola.ttf",      // Consolas (모노스페이스)
            "C:/Windows/Fonts/arial.ttf",        // Arial
        ];

        for path in &system_font_paths {
            if Path::new(path).exists() {
                if let Ok(font_data) = std::fs::read(path) {
                    // FontConfig 없이 기본 설정으로 폰트 추가
                    // (freetype feature 비활성화 상태에서 FontConfig 사용 시 assertion 실패)
                    fonts.add_font_from_memory_ttf(
                        Box::leak(font_data.into_boxed_slice()),
                        font_size,
                        None,  // config - 기본값 사용
                        None,  // glyph_ranges
                    );

                    log::info!("[ImGui] Loaded system font: {}", path);
                    return true;
                }
            }
        }

        false
    }

    /// 한글 글리프 범위 반환
    ///
    /// ImGui glyph ranges 형식: [start1, end1, start2, end2, ..., 0]
    fn get_korean_glyph_ranges() -> &'static [u16] {
        // 한글 범위 (정적 배열 - 'static lifetime 필요)
        static KOREAN_RANGES: &[u16] = &[
            // 한글 자모 (Hangul Jamo)
            0x1100, 0x11FF,
            // 한글 호환 자모 (Hangul Compatibility Jamo)
            0x3130, 0x318F,
            // 한글 음절 (Hangul Syllables) - 가~힣
            0xAC00, 0xD7A3,
            // 종료 마커
            0,
        ];
        KOREAN_RANGES
    }

    /// 한글 폰트 병합 시도
    fn try_merge_korean_font(fonts: &mut imgui::FontAtlas, font_size: f32) {
        // 한글 폰트 경로들 (우선순위 순)
        let korean_font_paths = [
            "engine/fonts/Pretendard-Medium.ttf",  // 프로젝트 내장 (권장)
            "engine/fonts/NotoSansKR-Medium.ttf",  // Noto Sans Korean
            "C:/Windows/Fonts/malgun.ttf",         // 맑은 고딕 (Windows)
        ];

        for path in &korean_font_paths {
            if Path::new(path).exists() {
                if let Ok(font_data) = std::fs::read(path) {
                    // 한글 글리프 범위 명시적 지정
                    let korean_ranges = Self::get_korean_glyph_ranges();

                    // 병합 모드 설정 - merge_mode만 사용
                    // (freetype feature 비활성화 상태에서 다른 옵션 사용 시 assertion 실패)
                    let merge_config = FontConfig::new()
                        .merge_mode(true);  // 이전 폰트에 병합

                    fonts.add_font_from_memory_ttf(
                        Box::leak(font_data.into_boxed_slice()),
                        font_size,
                        Some(&merge_config),
                        Some(korean_ranges),  // 한글 글리프 범위 지정
                    );

                    log::info!("[ImGui] Merged Korean font: {} ({} glyph ranges)",
                        path, korean_ranges.len() / 2);
                    return;
                }
            }
        }

        log::warn!("[ImGui] No Korean font found - Korean text may not display correctly");
    }

    /// 스타일 설정 (SKOPE Metal Theme)
    fn setup_style(context: &mut Context) {
        let style = context.style_mut();

        // === 텍스트 ===
        style.set_color(StyleColor::Text, [0.90, 0.90, 0.90, 1.0]);
        style.set_color(StyleColor::TextDisabled, [0.50, 0.50, 0.50, 1.0]);
        style.set_color(StyleColor::TextSelectedBg, [0.26, 0.59, 0.98, 0.35]);

        // === 배경색 (SKOPE Metal 3단계) ===
        // Deepest Dark (#121214)
        style.set_color(StyleColor::WindowBg, [0.07, 0.07, 0.08, 1.0]);
        // Panel BG (#1C1E21)
        style.set_color(StyleColor::ChildBg, [0.11, 0.12, 0.13, 1.0]);
        style.set_color(StyleColor::PopupBg, [0.08, 0.08, 0.09, 0.98]);
        style.set_color(StyleColor::MenuBarBg, [0.11, 0.12, 0.13, 1.0]);

        // === 타이틀바 (Flat Header: TitleBg == ChildBg) ===
        style.set_color(StyleColor::TitleBg, [0.11, 0.12, 0.13, 1.0]);
        style.set_color(StyleColor::TitleBgActive, [0.11, 0.12, 0.13, 1.0]);
        style.set_color(StyleColor::TitleBgCollapsed, [0.05, 0.05, 0.05, 0.5]);

        // === 탭 (VIP Tab 스타일) ===
        // 비활성 탭: Deepest Dark
        style.set_color(StyleColor::Tab, [0.07, 0.07, 0.08, 1.0]);
        // 호버: 살짝 밝게
        style.set_color(StyleColor::TabHovered, [0.25, 0.25, 0.27, 1.0]);
        // 활성 탭: Panel BG보다 약간 밝게
        style.set_color(StyleColor::TabSelected, [0.20, 0.20, 0.22, 1.0]);
        // 상단 파란 액센트 줄
        style.set_color(StyleColor::TabSelectedOverline, [0.35, 0.75, 0.95, 1.0]);
        // 비포커스 탭
        style.set_color(StyleColor::TabDimmed, [0.05, 0.05, 0.06, 1.0]);
        style.set_color(StyleColor::TabDimmedSelected, [0.16, 0.16, 0.18, 1.0]);
        style.set_color(StyleColor::TabDimmedSelectedOverline, [0.20, 0.50, 0.70, 1.0]);

        // 도킹
        style.set_color(StyleColor::DockingEmptyBg, [0.05, 0.05, 0.05, 1.0]);
        style.set_color(StyleColor::DockingPreview, [0.26, 0.59, 0.98, 0.70]);

        // 버튼
        style.set_color(StyleColor::Button, [0.16, 0.17, 0.18, 1.0]);
        style.set_color(StyleColor::ButtonHovered, [0.22, 0.23, 0.25, 1.0]);
        style.set_color(StyleColor::ButtonActive, [0.28, 0.30, 0.32, 1.0]);

        // 헤더
        style.set_color(StyleColor::Header, [0.16, 0.17, 0.18, 1.0]);
        style.set_color(StyleColor::HeaderHovered, [0.22, 0.23, 0.25, 1.0]);
        style.set_color(StyleColor::HeaderActive, [0.28, 0.30, 0.32, 1.0]);

        // 프레임
        style.set_color(StyleColor::FrameBg, [0.05, 0.05, 0.06, 1.0]);
        style.set_color(StyleColor::FrameBgHovered, [0.08, 0.08, 0.09, 1.0]);
        style.set_color(StyleColor::FrameBgActive, [0.10, 0.10, 0.11, 1.0]);

        // 스크롤바
        style.set_color(StyleColor::ScrollbarBg, [0.02, 0.02, 0.02, 0.53]);
        style.set_color(StyleColor::ScrollbarGrab, [0.31, 0.31, 0.31, 1.0]);
        style.set_color(StyleColor::ScrollbarGrabHovered, [0.41, 0.41, 0.41, 1.0]);
        style.set_color(StyleColor::ScrollbarGrabActive, [0.51, 0.51, 0.51, 1.0]);

        // 테두리
        style.set_color(StyleColor::Border, [0.18, 0.19, 0.20, 1.0]);
        style.set_color(StyleColor::BorderShadow, [0.0, 0.0, 0.0, 0.0]);

        // 구분선
        style.set_color(StyleColor::Separator, [0.18, 0.19, 0.20, 1.0]);
        style.set_color(StyleColor::SeparatorHovered, [0.26, 0.59, 0.98, 0.78]);
        style.set_color(StyleColor::SeparatorActive, [0.26, 0.59, 0.98, 1.0]);

        // 슬라이더
        style.set_color(StyleColor::SliderGrab, [0.35, 0.75, 0.95, 1.0]);
        style.set_color(StyleColor::SliderGrabActive, [0.45, 0.85, 1.0, 1.0]);

        // 체크마크/리사이즈 그립
        style.set_color(StyleColor::CheckMark, [0.35, 0.75, 0.95, 1.0]);
        style.set_color(StyleColor::ResizeGrip, [0.26, 0.59, 0.98, 0.20]);
        style.set_color(StyleColor::ResizeGripHovered, [0.26, 0.59, 0.98, 0.67]);
        style.set_color(StyleColor::ResizeGripActive, [0.26, 0.59, 0.98, 0.95]);

        // 플롯
        style.set_color(StyleColor::PlotLines, [0.61, 0.61, 0.61, 1.0]);
        style.set_color(StyleColor::PlotLinesHovered, [1.0, 0.43, 0.35, 1.0]);
        style.set_color(StyleColor::PlotHistogram, [0.90, 0.70, 0.00, 1.0]);
        style.set_color(StyleColor::PlotHistogramHovered, [1.0, 0.60, 0.00, 1.0]);

        // 테이블
        style.set_color(StyleColor::TableHeaderBg, [0.11, 0.12, 0.13, 1.0]);
        style.set_color(StyleColor::TableBorderStrong, [0.18, 0.19, 0.20, 1.0]);
        style.set_color(StyleColor::TableBorderLight, [0.13, 0.14, 0.15, 1.0]);
        style.set_color(StyleColor::TableRowBg, [0.0, 0.0, 0.0, 0.0]);
        style.set_color(StyleColor::TableRowBgAlt, [1.0, 1.0, 1.0, 0.03]);

        // 네비게이션
        style.set_color(StyleColor::NavCursor, [0.26, 0.59, 0.98, 1.0]);
        style.set_color(StyleColor::NavWindowingHighlight, [1.0, 1.0, 1.0, 0.70]);
        style.set_color(StyleColor::NavWindowingDimBg, [0.80, 0.80, 0.80, 0.20]);

        // 모달
        style.set_color(StyleColor::ModalWindowDimBg, [0.0, 0.0, 0.0, 0.60]);

        // 드래그 앤 드롭
        style.set_color(StyleColor::DragDropTarget, [0.35, 0.75, 0.95, 0.90]);

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

    /// winit 이벤트 처리 (Multi-Viewport 지원)
    ///
    /// 메인 윈도우와 보조 윈도우(Multi-Viewport)의 이벤트를 모두 처리합니다.
    /// dear-imgui-winit의 handle_event_with_multi_viewport() 사용
    pub fn handle_event_multi_viewport(
        &mut self,
        window: &Window,
        window_id: winit::window::WindowId,
        event: &winit::event::WindowEvent,
    ) -> bool {
        // WindowEvent를 Event로 래핑 (user event 타입은 () 사용)
        let full_event: winit::event::Event<()> = winit::event::Event::WindowEvent {
            window_id,
            event: event.clone(),
        };

        // Multi-Viewport 이벤트 핸들링 (메인 + 보조 윈도우)
        let consumed = multi_viewport::handle_event_with_multi_viewport(
            &mut self.platform,
            &mut self.context,
            window,
            &full_event,
        );

        // HiDPI 리사이즈 시 ImGui display_size 강제 업데이트 (메인 윈도우만)
        if window_id == window.id() {
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
        }

        consumed || self.context.io().want_capture_mouse() || self.context.io().want_capture_keyboard()
    }

    /// winit 이벤트 처리 (기존 방식 - 호환성 유지)
    #[allow(dead_code)]
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

    /// 메인 뷰포트 렌더링 (encoder에 렌더 패스 추가)
    ///
    /// 주의: Multi-Viewport를 사용할 경우, 이 함수 호출 후 encoder를 제출한 다음
    /// `render_secondary_viewports()`를 호출해야 합니다.
    /// (보조 뷰포트가 같은 uniform buffer를 사용하기 때문)
    pub fn render_main_viewport(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        window: &Window,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 렌더 준비
        self.platform.prepare_render(&mut self.context, window);

        // 드로우 데이터 가져오기
        let draw_data = self.context.render();

        // 렌더 패스 생성 및 메인 윈도우 렌더링
        {
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

            // ImGui 메인 윈도우 렌더링
            self.renderer.render_draw_data(draw_data, &mut render_pass)?;
        } // render_pass 드롭

        Ok(())
    }

    /// 보조 뷰포트(Multi-Viewport) 렌더링
    ///
    /// 주의: 반드시 메인 encoder 제출 후에 호출해야 합니다.
    /// 보조 뷰포트는 자체 encoder를 생성하여 즉시 제출합니다.
    pub fn render_secondary_viewports(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
    ) {
        // Multi-Viewport: EventLoop guard 설정 (보조 윈도우 생성에 필요)
        let _guard = multi_viewport::set_event_loop_for_frame(event_loop);

        // Multi-Viewport: 보조 윈도우 업데이트 및 렌더링
        self.context.update_platform_windows();
        self.context.render_platform_windows_default();

        // 디버그: Viewport 상태 로깅 (처음 몇 프레임만)
        if self.frame_count < 5 {
            unsafe {
                let pio = dear_imgui_rs::sys::igGetPlatformIO_Nil();
                if !pio.is_null() {
                    let viewport_count = (*pio).Viewports.Size;
                    log::info!("[ImGui-MV] Frame {}: {} viewports", self.frame_count, viewport_count);
                    for i in 0..viewport_count {
                        let vp = *(*pio).Viewports.Data.add(i as usize);
                        if !vp.is_null() {
                            let pos = (*vp).Pos;
                            let size = (*vp).Size;
                            let flags = (*vp).Flags;
                            log::info!("[ImGui-MV]   VP{}: pos=({:.0},{:.0}) size=({:.0},{:.0}) flags=0x{:x}",
                                i, pos.x, pos.y, size.x, size.y, flags);
                        }
                    }
                }
            }
        }

        self.frame_count += 1;
    }

    /// 렌더링 (하위 호환성 유지 - Multi-Viewport 없이 사용 시)
    #[allow(dead_code)]
    pub fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        window: &Window,
        event_loop: &winit::event_loop::ActiveEventLoop,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.render_main_viewport(encoder, view, window)?;
        // Note: Multi-Viewport 사용 시 이 방식은 uniform buffer 충돌 발생
        // render_secondary_viewports는 encoder 제출 후 호출 필요
        let _guard = multi_viewport::set_event_loop_for_frame(event_loop);
        self.context.update_platform_windows();
        self.context.render_platform_windows_default();
        self.frame_count += 1;
        Ok(())
    }

    /// 보조 윈도우(Multi-Viewport) 이벤트 처리
    ///
    /// 메인 윈도우가 아닌 ImGui가 생성한 보조 윈도우의 이벤트를 처리합니다.
    /// Resize/Move/Close 등의 이벤트를 해당 viewport에 전달합니다.
    pub fn handle_secondary_viewport_event(
        &mut self,
        window_id: WindowId,
        event: &winit::event::WindowEvent,
    ) {
        use winit::event::WindowEvent;

        unsafe {
            let pio = dear_imgui_rs::sys::igGetPlatformIO_Nil();
            if pio.is_null() {
                return;
            }
            let viewports = &(*pio).Viewports;
            if viewports.Data.is_null() || viewports.Size <= 0 {
                return;
            }

            // 메인 viewport(첫 번째)는 건너뛰기 - 이미 일반 이벤트 플로우에서 처리됨
            // 보조 viewport만 처리 (i=1부터 시작)
            for i in 1..viewports.Size {
                let vp = *viewports.Data.add(i as usize);
                if vp.is_null() {
                    continue;
                }

                // PlatformUserData가 없으면 초기화 안 된 viewport
                if (*vp).PlatformUserData.is_null() {
                    continue;
                }

                // PlatformHandle은 Window 포인터
                let window_ptr = (*vp).PlatformHandle as *const Window;
                if window_ptr.is_null() {
                    continue;
                }

                // 포인터 유효성 기본 검사 (0x3F800000 같은 이상한 값 방지)
                let ptr_val = window_ptr as usize;
                if ptr_val < 0x10000 || ptr_val == 0x3F800000 {
                    // 너무 낮은 주소거나 float 1.0 값이면 무효
                    continue;
                }

                // 윈도우 ID 비교
                let vp_window: &Window = &*window_ptr;
                if vp_window.id() != window_id {
                    continue;
                }

                // 핵심 이벤트만 처리 (간소화하여 안정성 확보)
                match event {
                    WindowEvent::Resized(_) => {
                        (*vp).PlatformRequestResize = true;
                    }
                    WindowEvent::Moved(_) => {
                        (*vp).PlatformRequestMove = true;
                    }
                    WindowEvent::CloseRequested => {
                        (*vp).PlatformRequestClose = true;
                    }
                    WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                        let scale = *scale_factor as f32;
                        if scale.is_finite() && scale > 0.0 && scale < 10.0 {
                            (*vp).DpiScale = scale;
                            (*vp).FramebufferScale.x = scale;
                            (*vp).FramebufferScale.y = scale;
                        }
                    }
                    _ => {
                        // 마우스/키보드 이벤트는 WinitPlatform이 처리하므로 여기서는 생략
                        // (Multi-Viewport에서 입력 이벤트는 메인 윈도우와 동일하게 라우팅됨)
                    }
                }

                break; // 해당 viewport 찾았으므로 종료
            }
        }
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

        // Multi-Viewport 활성화
        flags.insert(ConfigFlags::VIEWPORTS_ENABLE);
        io.set_config_flags(flags);
    }

    context
}
