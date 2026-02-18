// SKOPE Engine - Histogram-based Auto Exposure
// Reference: Modern real-time rendering (Frostbite, UE5)
//
// Features:
// - 256-bin luminance histogram
// - Weighted average luminance calculation
// - Smooth temporal adaptation
// - Min/max exposure limits
// - Low/high percentile clamping (ignore extremes)

use bytemuck::{Pod, Zeroable};

/// Auto Exposure 파라미터
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct AutoExposureParams {
    /// 최소 노출 (EV)
    pub min_exposure: f32,
    /// 최대 노출 (EV)
    pub max_exposure: f32,
    /// 적응 속도 (높을수록 빠름)
    pub adaptation_speed: f32,
    /// 노출 보정 (EV, 최종 결과에 더함)
    pub exposure_compensation: f32,

    /// 낮은 퍼센타일 (무시할 어두운 픽셀 비율, 0.0~1.0)
    pub low_percentile: f32,
    /// 높은 퍼센타일 (무시할 밝은 픽셀 비율, 0.0~1.0)
    pub high_percentile: f32,
    /// 키 값 (평균 회색 목표, 보통 0.18)
    pub key_value: f32,
    /// Auto Exposure 활성화
    pub enabled: u32,
}

impl Default for AutoExposureParams {
    fn default() -> Self {
        Self {
            min_exposure: 0.001,
            max_exposure: 10.0,
            adaptation_speed: 1.0,
            exposure_compensation: 0.0,
            low_percentile: 0.1,    // 하위 10% 무시
            high_percentile: 0.95,  // 상위 5% 무시
            key_value: 0.18,        // 중간 회색
            enabled: 1,
        }
    }
}

impl AutoExposureParams {
    /// 빠른 적응 (액션 게임)
    pub fn fast() -> Self {
        Self {
            adaptation_speed: 3.0,
            ..Default::default()
        }
    }

    /// 느린 적응 (시네마틱)
    pub fn slow() -> Self {
        Self {
            adaptation_speed: 0.5,
            ..Default::default()
        }
    }

    /// 실내 최적화
    pub fn indoor() -> Self {
        Self {
            min_exposure: 0.01,
            max_exposure: 5.0,
            exposure_compensation: 0.5,
            ..Default::default()
        }
    }

    /// 야외 최적화
    pub fn outdoor() -> Self {
        Self {
            min_exposure: 0.0001,
            max_exposure: 2.0,
            exposure_compensation: -0.5,
            ..Default::default()
        }
    }
}

/// Histogram 데이터 (GPU 버퍼용)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct HistogramData {
    /// 256개의 히스토그램 빈
    pub bins: [u32; 256],
}

impl Default for HistogramData {
    fn default() -> Self {
        Self { bins: [0; 256] }
    }
}

/// Auto Exposure 결과 (GPU에서 계산)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ExposureResult {
    /// 현재 노출 값
    pub current_exposure: f32,
    /// 목표 노출 값
    pub target_exposure: f32,
    /// 평균 휘도
    pub average_luminance: f32,
    /// 패딩
    pub _pad: f32,
}

impl Default for ExposureResult {
    fn default() -> Self {
        Self {
            current_exposure: 1.0,
            target_exposure: 1.0,
            average_luminance: 0.18,
            _pad: 0.0,
        }
    }
}

/// Histogram Auto Exposure 파이프라인
pub struct AutoExposurePipeline {
    /// 히스토그램 계산 파이프라인
    pub histogram_pipeline: wgpu::ComputePipeline,
    /// 평균 계산 파이프라인
    pub average_pipeline: wgpu::ComputePipeline,

    /// 히스토그램 버퍼 (256 bins)
    pub histogram_buffer: wgpu::Buffer,
    /// 노출 결과 버퍼
    pub exposure_buffer: wgpu::Buffer,
    /// 파라미터 버퍼
    pub params_buffer: wgpu::Buffer,

    /// Bind group layouts
    pub histogram_bind_group_layout: wgpu::BindGroupLayout,
    pub average_bind_group_layout: wgpu::BindGroupLayout,

    /// 스크린 사이즈 (dispatch 계산용)
    pub screen_size: (u32, u32),
}

impl AutoExposurePipeline {
    pub fn new(device: &wgpu::Device, screen_size: (u32, u32)) -> Self {
        // Histogram buffer (256 u32 bins + atomic access)
        let histogram_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Auto Exposure Histogram Buffer"),
            size: std::mem::size_of::<HistogramData>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Exposure result buffer
        let exposure_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Auto Exposure Result Buffer"),
            size: std::mem::size_of::<ExposureResult>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Params buffer
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Auto Exposure Params Buffer"),
            size: std::mem::size_of::<AutoExposureParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Histogram bind group layout
        let histogram_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Histogram Bind Group Layout"),
            entries: &[
                // HDR input texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Histogram buffer (read-write)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Params
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Average bind group layout
        let average_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Average Bind Group Layout"),
            entries: &[
                // Histogram buffer (read-only)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Exposure result buffer (read-write for temporal)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Params
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Create shaders
        let histogram_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Histogram Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/histogram_compute.wgsl").into()),
        });

        let average_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Histogram Average Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/histogram_average.wgsl").into()),
        });

        // Create pipelines
        let histogram_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Histogram Pipeline Layout"),
            bind_group_layouts: &[&histogram_bind_group_layout],
            immediate_size: 0,
        });

        let histogram_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Histogram Compute Pipeline"),
            layout: Some(&histogram_pipeline_layout),
            module: &histogram_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let average_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Average Pipeline Layout"),
            bind_group_layouts: &[&average_bind_group_layout],
            immediate_size: 0,
        });

        let average_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Histogram Average Pipeline"),
            layout: Some(&average_pipeline_layout),
            module: &average_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        Self {
            histogram_pipeline,
            average_pipeline,
            histogram_buffer,
            exposure_buffer,
            params_buffer,
            histogram_bind_group_layout,
            average_bind_group_layout,
            screen_size,
        }
    }

    /// 파라미터 업데이트
    pub fn update_params(&self, queue: &wgpu::Queue, params: &AutoExposureParams) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[*params]));
    }

    /// 히스토그램 클리어 (프레임 시작 시)
    pub fn clear_histogram(&self, queue: &wgpu::Queue) {
        let zeros = HistogramData::default();
        queue.write_buffer(&self.histogram_buffer, 0, bytemuck::cast_slice(&[zeros]));
    }

    /// Auto Exposure 실행 (Histogram + Average)
    pub fn execute(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        hdr_input: &wgpu::TextureView,
    ) {
        // Histogram pass: count luminance distribution
        let histogram_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Auto Exposure Histogram BG"),
            layout: &self.histogram_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(hdr_input) },
                wgpu::BindGroupEntry { binding: 1, resource: self.histogram_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: self.params_buffer.as_entire_binding() },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Auto Exposure Histogram Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.histogram_pipeline);
            pass.set_bind_group(0, &histogram_bg, &[]);
            pass.dispatch_workgroups(
                self.screen_size.0.div_ceil(8),
                self.screen_size.1.div_ceil(8),
                1,
            );
        }

        // Average pass: compute weighted average luminance → exposure
        let average_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Auto Exposure Average BG"),
            layout: &self.average_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.histogram_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.exposure_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: self.params_buffer.as_entire_binding() },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Auto Exposure Average Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.average_pipeline);
            pass.set_bind_group(0, &average_bg, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
    }

    /// 리사이즈
    pub fn resize(&mut self, _device: &wgpu::Device, new_size: (u32, u32)) {
        self.screen_size = new_size;
    }
}
