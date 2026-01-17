//! State Builder for Incremental Initialization
//!
//! 단계별 State 초기화를 위한 빌더 (스플래시 화면 응답성 유지)

use std::sync::Arc;

use super::gpu_context::MinimalGpuContext;
use crate::splash::InitStage;

/// State를 단계별로 초기화하는 빌더
///
/// 각 단계 사이에 이벤트 루프가 실행되어 윈도우 응답성을 유지함
pub struct StateBuilder {
    // GPU 컨텍스트 (MinimalGpuContext에서 가져옴) - 스플래시 렌더링에 필요
    pub surface: wgpu::Surface<'static>,
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    pub format: wgpu::TextureFormat,
    /// wgpu Instance (플로팅 윈도우 Surface 생성용)
    pub instance: wgpu::Instance,

    // 현재 단계
    current_stage: InitStage,
    // 시작 시간 (시간 기반 진행률용)
    start_time: std::time::Instant,
    // 총 로딩 시간 (초) - 95%까지만 시간 기반
    total_duration: f32,
    // 실제 초기화 시작 플래그
    init_started: bool,
}

impl StateBuilder {
    /// MinimalGpuContext로부터 StateBuilder 생성
    pub fn from_gpu_context(ctx: MinimalGpuContext) -> Self {
        Self {
            surface: ctx.surface,
            device: ctx.device,
            queue: ctx.queue,
            config: ctx.config,
            size: ctx.size,
            format: ctx.format,
            instance: ctx.instance,
            current_stage: InitStage::Renderers,
            start_time: std::time::Instant::now(),
            total_duration: 1.0, // 1초 동안 95%까지 애니메이션
            init_started: false,
        }
    }

    /// 초기화 시작 준비 완료 여부 (95% 도달)
    pub fn ready_to_init(&self) -> bool {
        !self.init_started && self.start_time.elapsed().as_secs_f32() >= self.total_duration
    }

    /// 초기화 시작 마킹
    pub fn mark_init_started(&mut self) {
        if !self.init_started {
            self.init_started = true;
            log::info!("[StateBuilder] Starting actual initialization...");
        }
    }

    /// 현재 단계 반환
    pub fn current_stage(&self) -> InitStage {
        self.current_stage
    }

    /// 현재 진행률 반환 (0.0 ~ 0.95)
    /// 95%까지만 시간 기반 애니메이션 (실제 초기화 완료 후 100% 표시는 SplashComplete에서)
    pub fn progress(&self) -> f32 {
        let elapsed = self.start_time.elapsed().as_secs_f32();
        // ease-out 곡선 적용 (처음 빠르고 끝에서 느려짐)
        let t = (elapsed / self.total_duration).min(1.0);
        // ease-out-cubic: 1 - (1 - t)^3
        let base_progress = 1.0 - (1.0 - t).powi(3);
        // 최대 95%까지만 (실제 초기화 완료 전)
        base_progress * 0.95
    }

    /// Surface 리사이즈
    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    /// 진행률에 따라 현재 단계 업데이트 (텍스트 표시용)
    pub fn advance(&mut self) {
        let progress = self.progress();
        let new_stage = if progress < 0.15 {
            InitStage::Renderers
        } else if progress < 0.30 {
            InitStage::Textures
        } else if progress < 0.50 {
            InitStage::Meshes
        } else if progress < 0.70 {
            InitStage::Scene
        } else if progress < 0.90 {
            InitStage::Characters
        } else if progress < 1.0 {
            InitStage::Finalize
        } else {
            InitStage::Complete
        };

        // 단계가 바뀔 때만 로그 출력
        if new_stage != self.current_stage {
            log::info!("[StateBuilder] Stage: {:?}", new_stage);
            self.current_stage = new_stage;
        }
    }

    /// GPU 컨텍스트 추출 (State::from_gpu_context()에 전달용)
    pub fn into_gpu_context(self) -> MinimalGpuContext {
        log::info!("[StateBuilder] Extracting GPU context for State::from_gpu_context()");
        MinimalGpuContext {
            surface: self.surface,
            device: self.device,
            queue: self.queue,
            config: self.config,
            size: self.size,
            format: self.format,
            instance: self.instance,
        }
    }
}
