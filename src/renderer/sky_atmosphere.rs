// SKOPE Engine — Sky Atmosphere System (Bruneton Model)
//
// Physically-based atmospheric scattering using precomputed LUTs:
// 1. Transmittance LUT (256x64) — optical depth lookup
// 2. Multi-Scatter LUT (32x32) — higher-order scattering contribution
// 3. Sky View LUT (192x108) — panoramic sky radiance
// 4. Aerial Perspective — per-pixel atmospheric fog
//
// LUTs 1-2 are computed once (or when atmosphere params change).
// LUT 3 is recomputed when sun direction changes.
// Aerial perspective runs every frame.
//
// Reference: Bruneton 2017, Hillaire 2020, UE5 SkyAtmosphereRendering.cpp

use bytemuck::{Pod, Zeroable};

// ---------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------

/// Atmosphere parameters matching the WGSL AtmosphereParams struct.
/// Earth-like defaults.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct AtmosphereParams {
    pub ground_radius: f32,     // km
    pub atmosphere_radius: f32, // km
    pub _pad0: [f32; 2],

    pub rayleigh_scatter: [f32; 3],    // 1/km
    pub rayleigh_density_h: f32,       // km

    pub mie_scatter: [f32; 3],         // 1/km
    pub mie_density_h: f32,            // km

    pub mie_absorption: [f32; 3],      // 1/km
    pub mie_phase_g: f32,

    pub ozone_absorption: [f32; 3],    // 1/km
    pub ozone_center_h: f32,           // km

    pub ozone_width: f32,              // km
    pub sun_intensity: f32,
    pub _pad1: [f32; 2],
}

impl Default for AtmosphereParams {
    fn default() -> Self {
        Self::earth()
    }
}

impl AtmosphereParams {
    /// Earth-like atmosphere parameters
    pub fn earth() -> Self {
        Self {
            ground_radius: 6360.0,
            atmosphere_radius: 6460.0,
            _pad0: [0.0; 2],
            // Rayleigh scattering at sea level (wavelength-dependent)
            rayleigh_scatter: [5.802e-3, 13.558e-3, 33.1e-3],
            rayleigh_density_h: 8.0,
            // Mie scattering at sea level
            mie_scatter: [3.996e-3, 3.996e-3, 3.996e-3],
            mie_density_h: 1.2,
            // Mie absorption
            mie_absorption: [4.4e-3, 4.4e-3, 4.4e-3],
            mie_phase_g: 0.8,
            // Ozone absorption
            ozone_absorption: [0.65e-3, 1.881e-3, 0.085e-3],
            ozone_center_h: 25.0,
            ozone_width: 15.0,
            sun_intensity: 20.0,
            _pad1: [0.0; 2],
        }
    }
}

/// Sky view parameters (per-frame, depends on camera + sun)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct SkyViewParams {
    pub camera_height: f32,        // km above ground
    pub sun_direction: [f32; 3],   // normalized world-space sun direction
}

/// Aerial perspective parameters (per-frame)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct AerialParams {
    pub inv_view_proj: [[f32; 4]; 4],
    pub camera_pos_ws: [f32; 3],
    pub camera_height: f32,        // km
    pub sun_direction: [f32; 3],
    pub max_distance: f32,         // km
    pub screen_width: u32,
    pub screen_height: u32,
    pub _pad: [u32; 2],
}

// ---------------------------------------------------------------------------
// LUT dimensions
// ---------------------------------------------------------------------------

const TRANSMITTANCE_LUT_WIDTH: u32 = 256;
const TRANSMITTANCE_LUT_HEIGHT: u32 = 64;
const MULTISCATTER_LUT_SIZE: u32 = 32;
const SKY_VIEW_LUT_WIDTH: u32 = 192;
const SKY_VIEW_LUT_HEIGHT: u32 = 108;

// ---------------------------------------------------------------------------
// Pipeline
// ---------------------------------------------------------------------------

#[allow(dead_code)]
pub struct SkyAtmospherePipeline {
    // Transmittance LUT (computed once)
    transmittance_pipeline: wgpu::ComputePipeline,
    transmittance_layout: wgpu::BindGroupLayout,
    pub transmittance_texture: wgpu::Texture,
    pub transmittance_view: wgpu::TextureView,

    // Multi-scatter LUT (computed once)
    multiscatter_pipeline: wgpu::ComputePipeline,
    multiscatter_layout: wgpu::BindGroupLayout,
    pub multiscatter_texture: wgpu::Texture,
    pub multiscatter_view: wgpu::TextureView,

    // Sky view LUT (per-frame when sun moves)
    sky_view_pipeline: wgpu::ComputePipeline,
    sky_view_layout: wgpu::BindGroupLayout,
    pub sky_view_texture: wgpu::Texture,
    pub sky_view_view: wgpu::TextureView,

    // Aerial perspective (per-frame)
    aerial_pipeline: wgpu::ComputePipeline,
    aerial_layout: wgpu::BindGroupLayout,
    pub aerial_output: wgpu::Texture,
    pub aerial_output_view: wgpu::TextureView,

    // Shared
    atm_params_buffer: wgpu::Buffer,
    sky_params_buffer: wgpu::Buffer,
    aerial_params_buffer: wgpu::Buffer,
    lut_sampler: wgpu::Sampler,

    // State
    pub params: AtmosphereParams,
    luts_dirty: bool,
    screen_width: u32,
    screen_height: u32,
}

impl SkyAtmospherePipeline {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let lut_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Sky Atmosphere LUT Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let atm_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Sky Atmosphere Params"),
            size: std::mem::size_of::<AtmosphereParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sky_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Sky View Params"),
            size: std::mem::size_of::<SkyViewParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let aerial_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Aerial Perspective Params"),
            size: std::mem::size_of::<AerialParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // --- Textures ---
        let transmittance_texture = create_lut_texture(
            device, TRANSMITTANCE_LUT_WIDTH, TRANSMITTANCE_LUT_HEIGHT, "Sky Transmittance LUT",
        );
        let transmittance_view = transmittance_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let multiscatter_texture = create_lut_texture(
            device, MULTISCATTER_LUT_SIZE, MULTISCATTER_LUT_SIZE, "Sky Multi-Scatter LUT",
        );
        let multiscatter_view = multiscatter_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sky_view_texture = create_lut_texture(
            device, SKY_VIEW_LUT_WIDTH, SKY_VIEW_LUT_HEIGHT, "Sky View LUT",
        );
        let sky_view_view = sky_view_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let aerial_output = create_lut_texture(device, width, height, "Aerial Perspective Output");
        let aerial_output_view = aerial_output.create_view(&wgpu::TextureViewDescriptor::default());

        // --- Layouts ---
        let transmittance_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Transmittance LUT Layout"),
            entries: &[
                bgl_uniform(0),
                bgl_storage_tex(1, wgpu::TextureFormat::Rgba16Float),
            ],
        });

        let multiscatter_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Multi-Scatter LUT Layout"),
            entries: &[
                bgl_uniform(0),
                bgl_float_tex(1),
                bgl_sampler(2),
                bgl_storage_tex(3, wgpu::TextureFormat::Rgba16Float),
            ],
        });

        let sky_view_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Sky View LUT Layout"),
            entries: &[
                bgl_uniform(0),     // atmosphere params
                bgl_uniform(1),     // sky view params
                bgl_float_tex(2),   // transmittance LUT
                bgl_float_tex(3),   // multiscatter LUT
                bgl_sampler(4),
                bgl_storage_tex(5, wgpu::TextureFormat::Rgba16Float),
            ],
        });

        let aerial_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Aerial Perspective Layout"),
            entries: &[
                bgl_uniform(0),     // atmosphere params
                bgl_uniform(1),     // aerial params
                bgl_float_tex(2),   // transmittance LUT
                bgl_sampler(3),
                bgl_depth_tex(4),   // scene depth
                bgl_float_tex(5),   // HDR scene color
                bgl_storage_tex(6, wgpu::TextureFormat::Rgba16Float),
            ],
        });

        // --- Shaders ---
        let transmittance_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Sky Transmittance LUT Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/sky_transmittance_lut.wgsl").into(),
            ),
        });

        let multiscatter_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Sky Multi-Scatter LUT Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/sky_multiscatter_lut.wgsl").into(),
            ),
        });

        let sky_view_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Sky View LUT Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/sky_view_lut.wgsl").into(),
            ),
        });

        let aerial_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Aerial Perspective Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/sky_aerial_perspective.wgsl").into(),
            ),
        });

        // --- Pipelines ---
        let transmittance_pipeline = create_compute_pipeline(device, "Sky Transmittance", &transmittance_shader, &transmittance_layout);
        let multiscatter_pipeline = create_compute_pipeline(device, "Sky Multi-Scatter", &multiscatter_shader, &multiscatter_layout);
        let sky_view_pipeline = create_compute_pipeline(device, "Sky View", &sky_view_shader, &sky_view_layout);
        let aerial_pipeline = create_compute_pipeline(device, "Aerial Perspective", &aerial_shader, &aerial_layout);

        Self {
            transmittance_pipeline,
            transmittance_layout,
            transmittance_texture,
            transmittance_view,
            multiscatter_pipeline,
            multiscatter_layout,
            multiscatter_texture,
            multiscatter_view,
            sky_view_pipeline,
            sky_view_layout,
            sky_view_texture,
            sky_view_view,
            aerial_pipeline,
            aerial_layout,
            aerial_output,
            aerial_output_view,
            atm_params_buffer,
            sky_params_buffer,
            aerial_params_buffer,
            lut_sampler,
            params: AtmosphereParams::earth(),
            luts_dirty: true,
            screen_width: width,
            screen_height: height,
        }
    }

    /// Update atmosphere parameters (marks LUTs for recomputation)
    #[allow(dead_code)]
    pub fn set_params(&mut self, params: AtmosphereParams) {
        self.params = params;
        self.luts_dirty = true;
    }

    /// Precompute transmittance and multi-scatter LUTs (only when dirty)
    pub fn precompute_luts(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        if !self.luts_dirty {
            return;
        }
        self.luts_dirty = false;

        queue.write_buffer(&self.atm_params_buffer, 0, bytemuck::bytes_of(&self.params));

        // Transmittance LUT
        let transmittance_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Transmittance LUT Bind Group"),
            layout: &self.transmittance_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.atm_params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.transmittance_view) },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Sky Transmittance LUT"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.transmittance_pipeline);
            pass.set_bind_group(0, &transmittance_bg, &[]);
            pass.dispatch_workgroups(
                (TRANSMITTANCE_LUT_WIDTH + 7) / 8,
                (TRANSMITTANCE_LUT_HEIGHT + 7) / 8,
                1,
            );
        }

        // Multi-scatter LUT (depends on transmittance)
        let multiscatter_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Multi-Scatter LUT Bind Group"),
            layout: &self.multiscatter_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.atm_params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.transmittance_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.lut_sampler) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&self.multiscatter_view) },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Sky Multi-Scatter LUT"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.multiscatter_pipeline);
            pass.set_bind_group(0, &multiscatter_bg, &[]);
            pass.dispatch_workgroups(
                (MULTISCATTER_LUT_SIZE + 7) / 8,
                (MULTISCATTER_LUT_SIZE + 7) / 8,
                1,
            );
        }
    }

    /// Update sky view LUT (call when sun direction or camera height changes)
    pub fn update_sky_view(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        sky_params: &SkyViewParams,
    ) {
        queue.write_buffer(&self.atm_params_buffer, 0, bytemuck::bytes_of(&self.params));
        queue.write_buffer(&self.sky_params_buffer, 0, bytemuck::bytes_of(sky_params));

        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Sky View LUT Bind Group"),
            layout: &self.sky_view_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.atm_params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.sky_params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&self.transmittance_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&self.multiscatter_view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(&self.lut_sampler) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&self.sky_view_view) },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Sky View LUT"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.sky_view_pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.dispatch_workgroups(
                (SKY_VIEW_LUT_WIDTH + 7) / 8,
                (SKY_VIEW_LUT_HEIGHT + 7) / 8,
                1,
            );
        }
    }

    /// Apply aerial perspective to the HDR scene
    pub fn apply_aerial_perspective(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        aerial_params: &AerialParams,
        depth_view: &wgpu::TextureView,
        hdr_view: &wgpu::TextureView,
    ) {
        queue.write_buffer(&self.aerial_params_buffer, 0, bytemuck::bytes_of(aerial_params));

        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Aerial Perspective Bind Group"),
            layout: &self.aerial_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.atm_params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.aerial_params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&self.transmittance_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(&self.lut_sampler) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(depth_view) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(hdr_view) },
                wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::TextureView(&self.aerial_output_view) },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Aerial Perspective"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.aerial_pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.dispatch_workgroups(
                (self.screen_width + 7) / 8,
                (self.screen_height + 7) / 8,
                1,
            );
        }
    }

    /// Resize aerial perspective output texture
    #[allow(dead_code)]
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.screen_width == width && self.screen_height == height {
            return;
        }
        self.screen_width = width;
        self.screen_height = height;
        self.aerial_output = create_lut_texture(device, width, height, "Aerial Perspective Output");
        self.aerial_output_view = self.aerial_output.create_view(&wgpu::TextureViewDescriptor::default());
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn create_lut_texture(device: &wgpu::Device, width: u32, height: u32, label: &str) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::STORAGE_BINDING,
        view_formats: &[],
    })
}

fn create_compute_pipeline(
    device: &wgpu::Device,
    label: &str,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::BindGroupLayout,
) -> wgpu::ComputePipeline {
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(&format!("{} Pipeline Layout", label)),
        bind_group_layouts: &[layout],
        immediate_size: 0,
    });
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(&format!("{} Pipeline", label)),
        layout: Some(&pipeline_layout),
        module: shader,
        entry_point: Some("main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    })
}

fn bgl_uniform(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn bgl_float_tex(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn bgl_depth_tex(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Depth,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

// ---------------------------------------------------------------------------
// CPU-side analytical transmittance approximation
// ---------------------------------------------------------------------------
// UE5's GetAtmosphereTransmittance() equivalent for CPU-side use.
// Uses the Chapman function approximation to avoid GPU readback.

/// Approximate atmospheric transmittance toward the sun at a given elevation.
///
/// This performs a simplified Rayleigh + Mie + Ozone optical depth calculation
/// using the Chapman function, producing an RGB transmittance value that
/// accounts for wavelength-dependent extinction.
///
/// At sunset/sunrise (low elevation), the path through the atmosphere is longer,
/// resulting in more blue light being scattered away → warm orange/red tones.
#[allow(dead_code)]
pub fn approximate_transmittance_toward_sun(
    params: &AtmosphereParams,
    sun_elevation_deg: f32,
) -> glam::Vec3 {
    let zenith_angle = (90.0 - sun_elevation_deg).to_radians();
    let cos_zenith = zenith_angle.cos();

    // Observer at ground level
    let h = 0.001_f32; // km above ground (sea level)
    let _r = params.ground_radius + h;

    // Chapman function approximation for optical path length
    // ch(x, theta) ≈ 1 / cos(theta) for theta < ~75°
    // For grazing angles, use the approximation from Schüler 2012
    let air_mass = if cos_zenith > 0.035 {
        // Simple secant approximation (valid for sun > ~2° above horizon)
        1.0 / cos_zenith
    } else if cos_zenith > -0.1 {
        // Kasten & Young (1989) empirical air mass formula
        let elev = sun_elevation_deg.max(0.0);
        1.0 / (elev.to_radians().sin() + 0.50572 * (elev + 6.07995).powf(-1.6364))
    } else {
        // Below horizon — effectively opaque
        return glam::Vec3::ZERO;
    };

    // Rayleigh optical depth at sea level (scale height ~8 km)
    let rayleigh_h = params.rayleigh_density_h;
    let rayleigh_depth_r = params.rayleigh_scatter[0] * rayleigh_h * air_mass;
    let rayleigh_depth_g = params.rayleigh_scatter[1] * rayleigh_h * air_mass;
    let rayleigh_depth_b = params.rayleigh_scatter[2] * rayleigh_h * air_mass;

    // Mie optical depth at sea level (scale height ~1.2 km)
    let mie_h = params.mie_density_h;
    let mie_ext_r = (params.mie_scatter[0] + params.mie_absorption[0]) * mie_h * air_mass;
    let mie_ext_g = (params.mie_scatter[1] + params.mie_absorption[1]) * mie_h * air_mass;
    let mie_ext_b = (params.mie_scatter[2] + params.mie_absorption[2]) * mie_h * air_mass;

    // Ozone optical depth (peaked around 25 km, width ~15 km)
    // Simplified: assume constant ozone layer contribution
    let ozone_path = params.ozone_width * air_mass.min(40.0); // Cap to prevent overflow
    let ozone_r = params.ozone_absorption[0] * ozone_path;
    let ozone_g = params.ozone_absorption[1] * ozone_path;
    let ozone_b = params.ozone_absorption[2] * ozone_path;

    // Total optical depth
    let tau_r = rayleigh_depth_r + mie_ext_r + ozone_r;
    let tau_g = rayleigh_depth_g + mie_ext_g + ozone_g;
    let tau_b = rayleigh_depth_b + mie_ext_b + ozone_b;

    // Transmittance = exp(-optical_depth)
    glam::Vec3::new(
        (-tau_r).exp(),
        (-tau_g).exp(),
        (-tau_b).exp(),
    )
}

fn bgl_storage_tex(binding: u32, format: wgpu::TextureFormat) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format,
            view_dimension: wgpu::TextureViewDimension::D2,
        },
        count: None,
    }
}

fn bgl_sampler(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}
