// SKOPE Engine - Resolution Management for TSR
//
// Manages internal vs output resolution for Temporal Super Resolution.
// Lower internal resolution = faster rendering, TSR reconstructs to output resolution.

/// Upscale quality modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UpscaleMode {
    /// 1:1 rendering (standard TAA, no upscaling)
    #[default]
    Native,
    /// 67% internal resolution (1.5x upscale)
    Quality,
    /// 58% internal resolution (sqrt(3)x upscale)
    #[allow(dead_code)]
    Balanced,
    /// 50% internal resolution (2x upscale)
    #[allow(dead_code)]
    Performance,
    /// 33% internal resolution (3x upscale)
    #[allow(dead_code)]
    Ultra,
}

impl UpscaleMode {
    /// Returns the internal resolution scale factor (0.0-1.0)
    pub fn scale_factor(&self) -> f32 {
        match self {
            UpscaleMode::Native => 1.0,
            UpscaleMode::Quality => 1.0 / 1.5,         // ~0.667
            UpscaleMode::Balanced => 1.0 / 1.732,      // ~0.577 (1/sqrt(3))
            UpscaleMode::Performance => 0.5,            // 1/2
            UpscaleMode::Ultra => 1.0 / 3.0,           // ~0.333
        }
    }

    /// Returns the upscale ratio (output / internal)
    #[allow(dead_code)]
    pub fn upscale_ratio(&self) -> f32 {
        match self {
            UpscaleMode::Native => 1.0,
            UpscaleMode::Quality => 1.5,
            UpscaleMode::Balanced => 1.732,
            UpscaleMode::Performance => 2.0,
            UpscaleMode::Ultra => 3.0,
        }
    }
}

/// Resolution configuration for TSR pipeline
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ResolutionConfig {
    pub internal_width: u32,
    pub internal_height: u32,
    pub output_width: u32,
    pub output_height: u32,
    pub scale_factor: f32,
    pub mode: UpscaleMode,
}

impl ResolutionConfig {
    /// Create a new resolution config from output dimensions and upscale mode.
    /// Internal resolution is computed from the scale factor, clamped to minimum 1.
    pub fn new(output_width: u32, output_height: u32, mode: UpscaleMode) -> Self {
        let scale = mode.scale_factor();
        let internal_width = ((output_width as f32 * scale).round() as u32).max(1);
        let internal_height = ((output_height as f32 * scale).round() as u32).max(1);

        Self {
            internal_width,
            internal_height,
            output_width,
            output_height,
            scale_factor: scale,
            mode,
        }
    }

    /// Returns true if upscaling is active (internal != output resolution)
    #[allow(dead_code)]
    pub fn is_upscaling(&self) -> bool {
        self.mode != UpscaleMode::Native
    }

    /// Returns the pixel count ratio (internal / output)
    #[allow(dead_code)]
    pub fn pixel_ratio(&self) -> f32 {
        let internal_pixels = self.internal_width as f64 * self.internal_height as f64;
        let output_pixels = self.output_width as f64 * self.output_height as f64;
        (internal_pixels / output_pixels) as f32
    }
}

impl Default for ResolutionConfig {
    fn default() -> Self {
        Self::new(1920, 1080, UpscaleMode::Native)
    }
}
