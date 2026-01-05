// SKOPE Engine - Post-Processing Pipeline
// 전체 포스트 프로세싱 통합

use super::*;

/// Post Processing에 필요한 G-Buffer 입력
#[derive(Clone, Copy)]
pub struct GBufferInputs<'a> {
    /// 깊이 텍스처 (DOF, SSAO용)
    pub depth_view: &'a wgpu::TextureView,
    /// 월드 노말 텍스처 (SSAO용)
    pub normal_view: &'a wgpu::TextureView,
    /// 속도 텍스처 (Motion Blur용)
    pub velocity_view: &'a wgpu::TextureView,
}

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
            bloom_enabled: false,  // DEBUG: 임시 비활성화
            tonemapping_enabled: true,
            color_grading_enabled: true,
            taa_enabled: true,
            dof_enabled: false,
            motion_blur_enabled: false,
            ssao_enabled: false,
            film_effects_enabled: false,  // DEBUG: 임시 비활성화
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
    /// Post Processing 파이프라인 실행 (기본 - G-Buffer 없음)
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
        self.execute_internal(device, encoder, hdr_input, shading_model, None)
    }

    /// Post Processing 파이프라인 실행 (G-Buffer 포함)
    ///
    /// hdr_input: Material Eval의 HDR 출력 (Rgba16Float)
    /// shading_model: 캐릭터 억제용 텍스처 (Bloom용, Motion Blur 캐릭터 제외용)
    /// gbuffer: G-Buffer 텍스처들 (DOF, SSAO, Motion Blur용)
    ///
    /// Returns: 최종 LDR 출력 텍스처 뷰
    pub fn execute_with_gbuffer<'a>(
        &'a self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        hdr_input: &wgpu::TextureView,
        shading_model: &wgpu::TextureView,
        gbuffer: GBufferInputs<'_>,
    ) -> &'a wgpu::TextureView {
        self.execute_internal(device, encoder, hdr_input, shading_model, Some(gbuffer))
    }

    /// 내부 실행 로직
    fn execute_internal<'a>(
        &'a self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        hdr_input: &wgpu::TextureView,
        shading_model: &wgpu::TextureView,
        gbuffer: Option<GBufferInputs<'_>>,
    ) -> &'a wgpu::TextureView {
        // 현재 HDR 버퍼 추적 (체이닝용)
        let mut current_hdr = hdr_input;

        // 1. SSAO (G-Buffer 필요)
        // Note: SSAO 결과는 ssao.output_view에 저장됨
        // 실제 AO 적용은 렌더러의 라이팅 패스에서 수행하는 것이 이상적
        // 여기서는 AO 텍스처만 계산
        if self.config.ssao_enabled {
            if let Some(ref gb) = gbuffer {
                self.ssao.execute(device, encoder, gb.depth_view, gb.normal_view);
            }
        }

        // 2. DOF (depth 필요)
        if self.config.dof_enabled {
            if let Some(ref gb) = gbuffer {
                self.dof.execute(device, encoder, current_hdr, gb.depth_view);
                current_hdr = &self.dof.output_view;
            }
        }

        // 3. Motion Blur (velocity 필요)
        if self.config.motion_blur_enabled {
            if let Some(ref gb) = gbuffer {
                self.motion_blur.execute(
                    device,
                    encoder,
                    current_hdr,
                    gb.velocity_view,
                    shading_model,
                );
                current_hdr = &self.motion_blur.output_view;
            }
        }

        // 4. Bloom (HDR → Bloom texture)
        if self.config.bloom_enabled {
            self.bloom.execute(device, encoder, current_hdr, shading_model);
        }

        // 5. Tonemapping (HDR + Bloom → LDR)
        if self.config.tonemapping_enabled {
            self.tonemapping.execute(
                device,
                encoder,
                current_hdr,
                &self.bloom.output_view,
            );
        }

        // 6. Film Effects (Vignette, Grain)
        if self.config.film_effects_enabled {
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

    /// SSAO 출력 뷰 가져오기 (라이팅에서 사용 가능)
    pub fn get_ssao_output(&self) -> &wgpu::TextureView {
        &self.ssao.output_view
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
