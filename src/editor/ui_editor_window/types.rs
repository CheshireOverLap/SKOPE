//! UI Editor Window Types
//!
//! Canvas resolution presets

/// Resolution presets
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CanvasResolution {
    Res1920x1080,
    Res1280x720,
    Res800x600,
    Custom(u32, u32),
}

impl CanvasResolution {
    pub fn size(&self) -> (u32, u32) {
        match self {
            CanvasResolution::Res1920x1080 => (1920, 1080),
            CanvasResolution::Res1280x720 => (1280, 720),
            CanvasResolution::Res800x600 => (800, 600),
            CanvasResolution::Custom(w, h) => (*w, *h),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            CanvasResolution::Res1920x1080 => "1920x1080",
            CanvasResolution::Res1280x720 => "1280x720",
            CanvasResolution::Res800x600 => "800x600",
            CanvasResolution::Custom(_, _) => "Custom",
        }
    }
}
