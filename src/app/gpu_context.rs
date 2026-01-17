//! Minimal GPU Context for Splash Screen
//!
//! 스플래시 화면 렌더링을 위한 최소 GPU 컨텍스트

use std::sync::Arc;
use winit::window::Window;

/// 스플래시 화면 렌더링을 위한 최소 GPU 컨텍스트
///
/// State의 전체 초기화 전에 빠르게 생성되어 스플래시 화면을 표시할 수 있게 함
pub struct MinimalGpuContext {
    pub surface: wgpu::Surface<'static>,
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub format: wgpu::TextureFormat,
    /// wgpu Instance (플로팅 윈도우 Surface 생성용)
    pub instance: wgpu::Instance,
}

impl MinimalGpuContext {
    /// 최소 GPU 초기화 (스플래시 렌더링 가능 상태)
    pub async fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();

        // wgpu instance 생성
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        // Surface 생성
        let surface = instance.create_surface(window.clone()).unwrap();

        // Adapter 요청 (GPU)
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        // Device와 Queue 생성
        // Required features for bindless textures (V2.1)
        let required_features = wgpu::Features::TEXTURE_BINDING_ARRAY
            | wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING;

        // Required limits for bindless textures
        let mut required_limits = wgpu::Limits::default();
        required_limits.max_sampled_textures_per_shader_stage = 4096;
        required_limits.max_storage_textures_per_shader_stage = 4096;
        // Critical: binding_array count limit (default 0, but all supported GPUs can do 500k)
        required_limits.max_binding_array_elements_per_shader_stage = 4096;
        required_limits.max_binding_array_sampler_elements_per_shader_stage = 16; // for samplers

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features,
                required_limits,
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::default(),
            })
            .await
            .unwrap();

        log::info!("[MinimalGpuContext] Device features: {:?}", device.features());

        // Surface 설정
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        log::info!("[MinimalGpuContext] GPU initialized ({}x{}, {:?})",
            size.width, size.height, surface_format);

        Self {
            surface,
            device: Arc::new(device),
            queue: Arc::new(queue),
            config,
            size,
            format: surface_format,
            instance,
        }
    }

    /// Surface 리사이즈
    #[allow(dead_code)]
    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }
}
