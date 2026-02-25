//! 렌더 스레드 커맨드 & 초기화 데이터

use std::sync::Arc;
use std::sync::{Condvar, Mutex};
use winit::window::WindowId;

use super::DrawWindowsData;

/// RT 초기화 시 move되는 데이터 (모두 Send -- unsafe 불필요)
///
/// wgpu 28.0: Device, Queue, Surface 모두 Send+Sync.
/// Instance/Adapter는 GT에 유지 (Surface 생성 + on_gpu_initialized 콜백).
pub struct RenderThreadInitData {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub format: wgpu::TextureFormat,
    // RenderState는 Step 4에서 추가
    // pub render_state: RenderState,
    /// 공유 렌더링 리소스 (폰트 아틀라스 + 파이프라인) — RT로 move
    pub shared_resources: Option<crate::render::SlateRenderResources>,
    /// 메인 윈도우 Surface — GT에서 생성 후 RT로 move (Send+Sync)
    pub main_surface: Option<(WindowId, wgpu::Surface<'static>, wgpu::SurfaceConfiguration)>,
}

/// Viewport 텍스처 정보 (GT → RT, owned, Send)
pub struct ViewportTextureInfo {
    pub name: String,
    pub view: wgpu::TextureView,
    pub size: (u32, u32),
}

/// RT 렌더링 설정 (GT → RT 전파)
///
/// UE5: FConsoleRenderThreadPropagation 패턴
#[derive(Debug, Clone, Copy)]
pub struct RenderConfig {
    pub present_mode: wgpu::PresentMode,
    pub debug_wireframe: bool,
    pub debug_draw_stats: bool,
    pub profiling_enabled: bool,
    /// 일회성 텍스처 무효화 트리거 (핫 리로드 등)
    pub force_texture_invalidate: bool,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            present_mode: wgpu::PresentMode::AutoVsync,
            debug_wireframe: false,
            debug_draw_stats: false,
            profiling_enabled: false,
            force_texture_invalidate: false,
        }
    }
}

/// GT -> RT 렌더링 커맨드
pub enum RenderCommand {
    /// 3D 렌더 상태 초기화 (한 번만 — on_gpu_initialized 후)
    InitSceneRenderer(Box<dyn super::SceneRenderer>),

    /// 3D 씬 렌더링 (매 프레임)
    RenderScene(Box<dyn std::any::Any + Send>),

    /// UI 드로우 + Present (모든 윈도우)
    DrawWindows(Box<DrawWindowsData>),

    /// Viewport 텍스처 등록/업데이트 (GT에서 pre_render 후 전송)
    RegisterViewportTextures(Vec<ViewportTextureInfo>),

    /// Surface 생명주기 -- GT에서 생성하여 move (wgpu 28 Send+Sync -- unsafe 불필요)
    AddSurface {
        window_id: WindowId,
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
    },
    /// Surface 제거 (윈도우 닫힘)
    RemoveSurface {
        window_id: WindowId,
    },
    /// Surface 리사이즈
    ResizeSurface {
        window_id: WindowId,
        width: u32,
        height: u32,
    },

    /// RT 설정 업데이트 (UE5 FConsoleRenderThreadPropagation)
    UpdateConfig(RenderConfig),

    /// 동기화 펜스 (UE5 FRenderCommandFence 패턴)
    SignalFence {
        fence_id: u64,
        signal: Arc<(Mutex<bool>, Condvar)>,
    },

    /// 종료
    Shutdown,
}
