// SKOPE Engine - Post-Processing Presets
// 스타일 프리셋

use super::*;

/// 포스트 프로세싱 프리셋
#[derive(Clone, Debug)]
pub struct PostProcessPreset {
    pub bloom: BloomParams,
    pub tonemap: TonemapParams,
    pub color_grading: ColorGradingParams,
    pub taa: TAAParams,
    pub dof: Option<DOFParams>,
    pub motion_blur: Option<MotionBlurParams>,
    pub ssao: Option<SSAOParams>,
    pub film: FilmEffectsParams,
}

impl Default for PostProcessPreset {
    fn default() -> Self {
        Self::skope_default()
    }
}

impl PostProcessPreset {
    /// SKOPE 기본 스타일
    pub fn skope_default() -> Self {
        Self {
            bloom: BloomParams {
                threshold: 0.9,
                intensity: 0.3,
                character_bloom_suppress: 0.5,
                ..Default::default()
            },
            tonemap: TonemapParams {
                operator: TonemapOperator::ACES as u32,
                saturation_preserve: 0.3,
                ..Default::default()
            },
            color_grading: ColorGradingParams {
                saturation: 1.1,
                shadow_tint: [0.0, 0.0, 0.1],
                shadow_tint_strength: 0.15,
                highlight_tint: [1.0, 0.95, 0.9],
                highlight_tint_strength: 0.1,
                ..Default::default()
            },
            taa: TAAParams::default(),
            dof: None,
            motion_blur: None,
            ssao: None,
            film: FilmEffectsParams {
                grain_intensity: 0.05,
                vignette_intensity: 0.2,
                ..Default::default()
            },
        }
    }

    /// 낮 야외 씬
    pub fn skope_daylight() -> Self {
        Self {
            bloom: BloomParams {
                threshold: 1.0,
                intensity: 0.2,
                ..Default::default()
            },
            tonemap: TonemapParams {
                exposure: 1.2,
                ..Default::default()
            },
            color_grading: ColorGradingParams {
                brightness: 1.05,
                saturation: 1.15,
                temperature: 6800.0,  // 약간 따뜻하게
                ..Default::default()
            },
            ..Self::skope_default()
        }
    }

    /// 밤 씬
    pub fn skope_night() -> Self {
        Self {
            bloom: BloomParams {
                threshold: 0.7,
                intensity: 0.4,
                tint: [0.9, 0.95, 1.0],  // 차가운 블룸
                ..Default::default()
            },
            tonemap: TonemapParams {
                exposure: 0.8,
                ..Default::default()
            },
            color_grading: ColorGradingParams {
                brightness: 0.9,
                saturation: 0.95,
                temperature: 5500.0,  // 차갑게
                shadow_tint: [0.0, 0.05, 0.15],
                shadow_tint_strength: 0.25,
                ..Default::default()
            },
            film: FilmEffectsParams {
                grain_intensity: 0.08,
                vignette_intensity: 0.35,
                ..Default::default()
            },
            ..Self::skope_default()
        }
    }

    /// 감성적인 씬 (컷씬, 대화)
    pub fn skope_emotional() -> Self {
        Self {
            bloom: BloomParams {
                threshold: 0.8,
                intensity: 0.35,
                ..Default::default()
            },
            dof: Some(DOFParams {
                focus_distance: 2.0,
                focus_range: 1.0,
                max_blur_size: 6.0,
                ..Default::default()
            }),
            color_grading: ColorGradingParams {
                contrast: 1.1,
                saturation: 1.05,
                lift: [0.02, 0.01, 0.03],  // 그림자에 보라 기운
                ..Default::default()
            },
            film: FilmEffectsParams {
                grain_intensity: 0.06,
                vignette_intensity: 0.3,
                ..Default::default()
            },
            ..Self::skope_default()
        }
    }

    /// 액션 씬
    pub fn skope_action() -> Self {
        Self {
            bloom: BloomParams {
                threshold: 0.85,
                intensity: 0.4,
                ..Default::default()
            },
            motion_blur: Some(MotionBlurParams {
                intensity: 0.4,
                exclude_characters: 1,
                ..Default::default()
            }),
            color_grading: ColorGradingParams {
                contrast: 1.15,
                saturation: 1.2,
                ..Default::default()
            },
            taa: TAAParams {
                sharpness: 0.3,  // 더 선명하게
                ..Default::default()
            },
            ..Self::skope_default()
        }
    }

    /// 보스전 씬
    pub fn skope_boss_fight() -> Self {
        Self {
            bloom: BloomParams {
                threshold: 0.75,
                intensity: 0.5,
                tint: [1.0, 0.9, 0.85],  // 따뜻한/위험한 느낌
                ..Default::default()
            },
            motion_blur: Some(MotionBlurParams {
                intensity: 0.5,
                max_blur_length: 25.0,
                exclude_characters: 1,
                ..Default::default()
            }),
            color_grading: ColorGradingParams {
                contrast: 1.2,
                saturation: 1.15,
                shadow_tint: [0.1, 0.0, 0.0],  // 붉은 그림자
                shadow_tint_strength: 0.2,
                ..Default::default()
            },
            film: FilmEffectsParams {
                grain_intensity: 0.07,
                vignette_intensity: 0.4,
                chromatic_intensity: 0.1,  // 약간의 색수차
                ..Default::default()
            },
            ..Self::skope_default()
        }
    }

    /// 평화로운 씬
    pub fn skope_peaceful() -> Self {
        Self {
            bloom: BloomParams {
                threshold: 0.95,
                intensity: 0.25,
                ..Default::default()
            },
            color_grading: ColorGradingParams {
                saturation: 1.08,
                temperature: 7000.0,  // 따뜻하게
                contrast: 0.95,  // 부드러운 대비
                highlight_tint: [1.0, 0.98, 0.92],
                highlight_tint_strength: 0.15,
                ..Default::default()
            },
            dof: Some(DOFParams {
                focus_distance: 5.0,
                focus_range: 3.0,
                max_blur_size: 4.0,
                bokeh_intensity: 0.4,
                ..Default::default()
            }),
            film: FilmEffectsParams {
                grain_intensity: 0.03,
                vignette_intensity: 0.15,
                ..Default::default()
            },
            ..Self::skope_default()
        }
    }

    /// 포토 모드 (최대 품질)
    pub fn photo_mode() -> Self {
        Self {
            bloom: BloomParams {
                threshold: 0.85,
                intensity: 0.35,
                downsample_passes: 8,
                ..Default::default()
            },
            dof: Some(DOFParams::cinematic()),
            ssao: Some(SSAOParams::high_quality()),
            taa: TAAParams::high_quality(),
            film: FilmEffectsParams::cinematic(),
            ..Self::skope_default()
        }
    }
}

/// 프리셋 블렌딩 (씬 전환용)
pub fn blend_presets(from: &PostProcessPreset, to: &PostProcessPreset, t: f32) -> PostProcessPreset {
    let t = t.clamp(0.0, 1.0);

    PostProcessPreset {
        bloom: BloomParams {
            threshold: lerp(from.bloom.threshold, to.bloom.threshold, t),
            soft_threshold: lerp(from.bloom.soft_threshold, to.bloom.soft_threshold, t),
            intensity: lerp(from.bloom.intensity, to.bloom.intensity, t),
            downsample_passes: if t < 0.5 { from.bloom.downsample_passes } else { to.bloom.downsample_passes },
            tint: [
                lerp(from.bloom.tint[0], to.bloom.tint[0], t),
                lerp(from.bloom.tint[1], to.bloom.tint[1], t),
                lerp(from.bloom.tint[2], to.bloom.tint[2], t),
            ],
            upsample_blend: lerp(from.bloom.upsample_blend, to.bloom.upsample_blend, t),
            character_bloom_suppress: lerp(from.bloom.character_bloom_suppress, to.bloom.character_bloom_suppress, t),
            _pad: [0.0; 3],
        },
        tonemap: TonemapParams {
            operator: if t < 0.5 { from.tonemap.operator } else { to.tonemap.operator },
            exposure: lerp(from.tonemap.exposure, to.tonemap.exposure, t),
            white_point: lerp(from.tonemap.white_point, to.tonemap.white_point, t),
            saturation_preserve: lerp(from.tonemap.saturation_preserve, to.tonemap.saturation_preserve, t),
            gamma: lerp(from.tonemap.gamma, to.tonemap.gamma, t),
            _pad: [0.0; 3],
        },
        color_grading: blend_color_grading(&from.color_grading, &to.color_grading, t),
        taa: TAAParams {
            history_weight: lerp(from.taa.history_weight, to.taa.history_weight, t),
            clamp_mode: if t < 0.5 { from.taa.clamp_mode } else { to.taa.clamp_mode },
            sharpness: lerp(from.taa.sharpness, to.taa.sharpness, t),
            motion_scale: lerp(from.taa.motion_scale, to.taa.motion_scale, t),
            jitter_enabled: if t < 0.5 { from.taa.jitter_enabled } else { to.taa.jitter_enabled },
            jitter_sequence: if t < 0.5 { from.taa.jitter_sequence } else { to.taa.jitter_sequence },
            current_jitter: from.taa.current_jitter,
        },
        dof: blend_optional_params(&from.dof, &to.dof, t, blend_dof),
        motion_blur: blend_optional_params(&from.motion_blur, &to.motion_blur, t, blend_motion_blur),
        ssao: blend_optional_params(&from.ssao, &to.ssao, t, blend_ssao),
        film: blend_film_effects(&from.film, &to.film, t),
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn blend_color_grading(from: &ColorGradingParams, to: &ColorGradingParams, t: f32) -> ColorGradingParams {
    ColorGradingParams {
        brightness: lerp(from.brightness, to.brightness, t),
        contrast: lerp(from.contrast, to.contrast, t),
        saturation: lerp(from.saturation, to.saturation, t),
        hue_shift: lerp(from.hue_shift, to.hue_shift, t),
        lift: [
            lerp(from.lift[0], to.lift[0], t),
            lerp(from.lift[1], to.lift[1], t),
            lerp(from.lift[2], to.lift[2], t),
        ],
        _pad0: 0.0,
        gamma: [
            lerp(from.gamma[0], to.gamma[0], t),
            lerp(from.gamma[1], to.gamma[1], t),
            lerp(from.gamma[2], to.gamma[2], t),
        ],
        _pad1: 0.0,
        gain: [
            lerp(from.gain[0], to.gain[0], t),
            lerp(from.gain[1], to.gain[1], t),
            lerp(from.gain[2], to.gain[2], t),
        ],
        _pad2: 0.0,
        shadow_tint: [
            lerp(from.shadow_tint[0], to.shadow_tint[0], t),
            lerp(from.shadow_tint[1], to.shadow_tint[1], t),
            lerp(from.shadow_tint[2], to.shadow_tint[2], t),
        ],
        shadow_tint_strength: lerp(from.shadow_tint_strength, to.shadow_tint_strength, t),
        highlight_tint: [
            lerp(from.highlight_tint[0], to.highlight_tint[0], t),
            lerp(from.highlight_tint[1], to.highlight_tint[1], t),
            lerp(from.highlight_tint[2], to.highlight_tint[2], t),
        ],
        highlight_tint_strength: lerp(from.highlight_tint_strength, to.highlight_tint_strength, t),
        temperature: lerp(from.temperature, to.temperature, t),
        tint: lerp(from.tint, to.tint, t),
        lut_intensity: lerp(from.lut_intensity, to.lut_intensity, t),
        lut_size: if t < 0.5 { from.lut_size } else { to.lut_size },
    }
}

fn blend_film_effects(from: &FilmEffectsParams, to: &FilmEffectsParams, t: f32) -> FilmEffectsParams {
    FilmEffectsParams {
        grain_intensity: lerp(from.grain_intensity, to.grain_intensity, t),
        grain_size: lerp(from.grain_size, to.grain_size, t),
        grain_colored: lerp(from.grain_colored, to.grain_colored, t),
        grain_luminance_linked: lerp(from.grain_luminance_linked, to.grain_luminance_linked, t),
        vignette_intensity: lerp(from.vignette_intensity, to.vignette_intensity, t),
        vignette_roundness: lerp(from.vignette_roundness, to.vignette_roundness, t),
        vignette_smoothness: lerp(from.vignette_smoothness, to.vignette_smoothness, t),
        _pad0: 0.0,
        vignette_color: [
            lerp(from.vignette_color[0], to.vignette_color[0], t),
            lerp(from.vignette_color[1], to.vignette_color[1], t),
            lerp(from.vignette_color[2], to.vignette_color[2], t),
        ],
        _pad1: 0.0,
        chromatic_intensity: lerp(from.chromatic_intensity, to.chromatic_intensity, t),
        time: from.time,
        _pad2: [0.0; 2],
    }
}

fn blend_dof(from: &DOFParams, to: &DOFParams, t: f32) -> DOFParams {
    DOFParams {
        focus_distance: lerp(from.focus_distance, to.focus_distance, t),
        focus_range: lerp(from.focus_range, to.focus_range, t),
        max_blur_size: lerp(from.max_blur_size, to.max_blur_size, t),
        bokeh_intensity: lerp(from.bokeh_intensity, to.bokeh_intensity, t),
        aperture_shape: if t < 0.5 { from.aperture_shape } else { to.aperture_shape },
        aperture_blades: if t < 0.5 { from.aperture_blades } else { to.aperture_blades },
        near_plane: from.near_plane,
        far_plane: from.far_plane,
    }
}

fn blend_motion_blur(from: &MotionBlurParams, to: &MotionBlurParams, t: f32) -> MotionBlurParams {
    MotionBlurParams {
        intensity: lerp(from.intensity, to.intensity, t),
        max_blur_length: lerp(from.max_blur_length, to.max_blur_length, t),
        sample_count: if t < 0.5 { from.sample_count } else { to.sample_count },
        exclude_characters: if t < 0.5 { from.exclude_characters } else { to.exclude_characters },
        camera_blur_scale: lerp(from.camera_blur_scale, to.camera_blur_scale, t),
        object_blur_scale: lerp(from.object_blur_scale, to.object_blur_scale, t),
        _pad: [0.0; 2],
    }
}

fn blend_ssao(from: &SSAOParams, to: &SSAOParams, t: f32) -> SSAOParams {
    SSAOParams {
        radius: lerp(from.radius, to.radius, t),
        intensity: lerp(from.intensity, to.intensity, t),
        bias: lerp(from.bias, to.bias, t),
        sample_count: if t < 0.5 { from.sample_count } else { to.sample_count },
        blur_passes: if t < 0.5 { from.blur_passes } else { to.blur_passes },
        blur_sharpness: lerp(from.blur_sharpness, to.blur_sharpness, t),
        near_plane: from.near_plane,
        far_plane: from.far_plane,
    }
}

fn blend_optional_params<T: Clone>(
    from: &Option<T>,
    to: &Option<T>,
    t: f32,
    blend_fn: fn(&T, &T, f32) -> T,
) -> Option<T> {
    match (from, to) {
        (Some(f), Some(t_val)) => Some(blend_fn(f, t_val, t)),
        (Some(f), None) => {
            if t < 0.5 { Some(f.clone()) } else { None }
        }
        (None, Some(t_val)) => {
            if t >= 0.5 { Some(t_val.clone()) } else { None }
        }
        (None, None) => None,
    }
}
