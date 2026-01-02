// SKOPE Engine - Post-Processing Pipeline
// 전체 포스트 프로세싱 통합

use super::*;

/// 포스트 프로세싱 설정
#[derive(Clone, Debug)]
pub struct PostProcessConfig {
    pub bloom_enabled: bool,
    pub tonemapping_enabled: bool,
    pub color_grading_enabled: bool,
    pub taa_enabled: bool,
    pub dof_enabled: bool,
    pub motion_blur_enabled: bool,
    pub ssao_enabled: bool,
    pub film_effects_enabled: bool,
}

impl Default for PostProcessConfig {
    fn default() -> Self {
        Self {
            bloom_enabled: true,  // Ping-Pong 패턴으로 MIP 충돌 해결됨
            tonemapping_enabled: true,
            color_grading_enabled: true,
            taa_enabled: true,
            dof_enabled: false,  // 기본 비활성화
            motion_blur_enabled: false,  // 기본 비활성화
            ssao_enabled: false,  // 기본 비활성화
            film_effects_enabled: true,
        }
    }
}

impl PostProcessConfig {
    /// 최소 설정 (성능 우선)
    pub fn minimal() -> Self {
        Self {
            bloom_enabled: true,
            tonemapping_enabled: true,
            color_grading_enabled: true,
            taa_enabled: true,
            dof_enabled: false,
            motion_blur_enabled: false,
            ssao_enabled: false,
            film_effects_enabled: false,
        }
    }

    /// 최대 설정 (품질 우선)
    pub fn maximum() -> Self {
        Self {
            bloom_enabled: true,
            tonemapping_enabled: true,
            color_grading_enabled: true,
            taa_enabled: true,
            dof_enabled: true,
            motion_blur_enabled: true,
            ssao_enabled: true,
            film_effects_enabled: true,
        }
    }
}

/// 전체 포스트 프로세싱 파이프라인
pub struct PostProcessPipeline {
    // 개별 이펙트 파이프라인
    pub bloom: BloomPipeline,
    pub tonemapping: TonemapPipeline,
    pub color_grading: ColorGradingPipeline,
    pub taa: TAAPipeline,
    pub dof: DOFPipeline,
    pub motion_blur: MotionBlurPipeline,
    pub ssao: SSAOPipeline,
    pub film_effects: FilmEffectsPipeline,

    // 설정
    pub config: PostProcessConfig,

    // 중간 버퍼
    pub hdr_buffer: wgpu::Texture,
    pub hdr_view: wgpu::TextureView,

    pub screen_size: (u32, u32),
}

impl PostProcessPipeline {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen_size: (u32, u32),
    ) -> Self {
        let bloom = BloomPipeline::new(device, screen_size);
        let tonemapping = TonemapPipeline::new(device, screen_size);
        let color_grading = ColorGradingPipeline::new(device, screen_size);
        let taa = TAAPipeline::new(device, screen_size);
        let dof = DOFPipeline::new(device, screen_size);
        let motion_blur = MotionBlurPipeline::new(device, screen_size);
        let ssao = SSAOPipeline::new(device, queue, screen_size);
        let film_effects = FilmEffectsPipeline::new(device, screen_size);

        // HDR 중간 버퍼
        let hdr_buffer = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Post HDR Buffer"),
            size: wgpu::Extent3d {
                width: screen_size.0,
                height: screen_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let hdr_view = hdr_buffer.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            bloom,
            tonemapping,
            color_grading,
            taa,
            dof,
            motion_blur,
            ssao,
            film_effects,
            config: PostProcessConfig::default(),
            hdr_buffer,
            hdr_view,
            screen_size,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, new_size: (u32, u32)) {
        if self.screen_size == new_size {
            return;
        }
        self.screen_size = new_size;

        self.bloom.resize(device, new_size);
        self.tonemapping.resize(device, new_size);
        self.color_grading.resize(device, new_size);
        self.taa.resize(device, new_size);
        self.dof.resize(device, new_size);
        self.motion_blur.resize(device, new_size);
        self.ssao.resize(device, new_size);
        self.film_effects.resize(device, new_size);

        self.hdr_buffer = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Post HDR Buffer"),
            size: wgpu::Extent3d {
                width: new_size.0,
                height: new_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        self.hdr_view = self.hdr_buffer.create_view(&wgpu::TextureViewDescriptor::default());
    }

    /// TAA 지터 값 가져오기 (프로젝션 매트릭스에 적용)
    pub fn get_taa_jitter(&self) -> [f32; 2] {
        if self.config.taa_enabled {
            self.taa.get_current_jitter()
        } else {
            [0.0, 0.0]
        }
    }

    /// 프레임 증가 (TAA용)
    pub fn increment_frame(&mut self) {
        self.taa.increment_frame();
    }

    /// 모든 파라미터 업데이트
    pub fn update_all_params(&self, queue: &wgpu::Queue, preset: &PostProcessPreset) {
        self.bloom.update_params(queue, &preset.bloom);
        self.tonemapping.update_params(queue, &preset.tonemap);
        self.color_grading.update_params(queue, &preset.color_grading);
        self.taa.update_params(queue, &preset.taa);
        if let Some(ref dof_params) = preset.dof {
            self.dof.update_params(queue, dof_params);
        }
        if let Some(ref motion_blur_params) = preset.motion_blur {
            self.motion_blur.update_params(queue, motion_blur_params);
        }
        if let Some(ref ssao_params) = preset.ssao {
            self.ssao.update_params(queue, ssao_params);
        }
        self.film_effects.update_params(queue, &preset.film);
    }
}

/// 디버그 뷰 모드
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DebugView {
    #[default]
    None,
    BloomOnly,
    PreTonemap,
    LUTPreview,
    Velocity,
    DOFCoC,
    SSAOOnly,
    TAAHistory,
}

impl PostProcessPipeline {
    /// Post Processing 파이프라인 실행
    ///
    /// hdr_input: Material Eval의 HDR 출력 (Rgba16Float)
    /// shading_model: 캐릭터 억제용 텍스처 (Bloom용)
    /// frame_time: 현재 프레임 시간 (Film Grain 애니메이션용)
    ///
    /// Returns: 최종 LDR 출력 텍스처 뷰
    pub fn execute<'a>(
        &'a self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        hdr_input: &wgpu::TextureView,
        shading_model: &wgpu::TextureView,
        _frame_time: f32,
    ) -> &'a wgpu::TextureView {
        // 1. Bloom (HDR → Bloom texture)
        if self.config.bloom_enabled {
            self.bloom.execute(device, encoder, hdr_input, shading_model);
        }

        // 2. Tonemapping (HDR + Bloom → LDR)
        if self.config.tonemapping_enabled {
            self.tonemapping.execute(
                device,
                encoder,
                hdr_input,
                &self.bloom.output_view,
            );
        }

        // 3. Film Effects (Vignette, Grain)
        if self.config.film_effects_enabled {
            // Film Grain 시간 업데이트는 별도로 호출 필요
            self.film_effects.execute(
                device,
                encoder,
                &self.tonemapping.output_view,
            );
            return &self.film_effects.output_view;
        }

        // Tonemapping 출력 반환
        &self.tonemapping.output_view
    }

    /// Film Grain 시간 업데이트
    pub fn update_film_time(&self, queue: &wgpu::Queue, time: f32) {
        self.film_effects.update_time(queue, time);
    }

    /// 최종 출력 뷰 가져오기 (설정에 따라 다름)
    /// Blit 초기화 및 바인드 그룹 생성에 사용
    pub fn get_final_output_view(&self) -> &wgpu::TextureView {
        if self.config.film_effects_enabled {
            &self.film_effects.output_view
        } else {
            &self.tonemapping.output_view
        }
    }
}
