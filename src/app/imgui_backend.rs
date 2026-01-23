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
use dear_imgui_wgpu::{WgpuRenderer, WgpuInitInfo};
use dear_imgui_winit::{WinitPlatform, HiDpiMode};

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

        // FreeType 기반 고품질 폰트 설정
        let scale_factor = window.scale_factor() as f32;
        Self::setup_fonts_freetype(&mut context, scale_factor)?;
        log::info!("[ImGui] FreeType font setup complete (scale={})", scale_factor);

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

    /// FreeType 기반 고품질 폰트 설정
    ///
    /// HiDPI 대응:
    /// 1. 물리적으로 큰 폰트를 굽고 (font_size * scale_factor)
    /// 2. font_global_scale로 다시 줄임 (1.0 / scale_factor)
    fn setup_fonts_freetype(context: &mut Context, scale_factor: f32) -> Result<(), Box<dyn std::error::Error>> {
        // 물리적 폰트 크기 계산
        let font_size_pixels = BASE_FONT_SIZE * scale_factor;

        let mut fonts = context.fonts();

        // 1. 기본 폰트 로드 (영문 + 기호)
        // 먼저 시스템 폰트 시도, 없으면 기본 폰트 사용
        let font_loaded = Self::try_load_system_font(&mut fonts, font_size_pixels);

        if !font_loaded {
            // 시스템 폰트 실패 시 기본 폰트 사용
            fonts.add_font(&[FontSource::default_font_with_size(font_size_pixels)]);
            log::info!("[ImGui] Using default font (system fonts not found)");
        }

        // 2. 한글 폰트 병합 (있으면)
        Self::try_merge_korean_font(&mut fonts, font_size_pixels);

        // 3. font_global_scale 설정 (물리 크기 -> 논리 크기 변환)
        // 폰트를 크게 구웠으니 UI에서는 줄여서 표시
        drop(fonts); // fonts borrow 해제
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
                    // FreeType 최적화 설정 (builder 패턴)
                    let config = FontConfig::new()
                        .size_pixels(font_size)
                        .oversample_h(2)   // 가로 오버샘플링 (LCD 최적화)
                        .oversample_v(1)   // 세로는 1로 충분
                        .pixel_snap_h(true);  // 픽셀 그리드 정렬 (흐릿함 방지)

                    fonts.add_font_from_memory_ttf(
                        Box::leak(font_data.into_boxed_slice()),
                        font_size,
                        Some(&config),
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
                    // 병합 모드 설정 (builder 패턴)
                    let merge_config = FontConfig::new()
                        .size_pixels(font_size)
                        .merge_mode(true)  // 이전 폰트에 병합
                        .glyph_offset([0.0, 0.0])  // 높이 조정 없음 (필요시 조절)
                        .oversample_h(2)
                        .pixel_snap_h(true);

                    // 한글 글리프 범위 명시적 지정
                    let korean_ranges = Self::get_korean_glyph_ranges();

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

        // 탭 (UE5 스타일 호버 효과)
        // - Tab (비활성): 배경색과 거의 비슷한 어두운 색
        style.set_color(StyleColor::Tab, [0.10, 0.10, 0.10, 1.0]);
        // - TabHovered (마우스 오버): ★ 밝은 회색으로 "빛나는" 느낌
        style.set_color(StyleColor::TabHovered, [0.35, 0.35, 0.35, 1.0]);
        // - TabSelected (활성 탭): 진한 회색 배경
        style.set_color(StyleColor::TabSelected, [0.22, 0.22, 0.22, 1.0]);
        // - TabSelectedOverline: 상단 파란 줄 (UE5 특징)
        style.set_color(StyleColor::TabSelectedOverline, [0.26, 0.59, 0.98, 1.0]);
        // - TabDimmed (비포커스 윈도우의 탭)
        style.set_color(StyleColor::TabDimmed, [0.08, 0.08, 0.08, 1.0]);
        // - TabDimmedSelected (비포커스 윈도우의 활성 탭)
        style.set_color(StyleColor::TabDimmedSelected, [0.18, 0.18, 0.18, 1.0]);
        // - TabDimmedSelectedOverline
        style.set_color(StyleColor::TabDimmedSelectedOverline, [0.15, 0.40, 0.75, 1.0]);

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
