//! Material Variant Key — UE5.7 Material Instance 변형 시스템
//!
//! GltfShadingModel + AlphaMode + double_sided 조합으로 변형 키 생성.
//! 셰이더는 이미 shading_model/alpha_mode를 읽으므로 추가 셰이더 변경 불필요.

use skope_gltf::intermediate::{GltfShadingModel, AlphaMode};

/// 알파 모드 키 (Hash/Eq를 위해 AlphaMode에서 cutoff 분리)
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum AlphaModeKey {
    Opaque,
    Mask,
    Blend,
}

impl From<AlphaMode> for AlphaModeKey {
    fn from(mode: AlphaMode) -> Self {
        match mode {
            AlphaMode::Opaque => AlphaModeKey::Opaque,
            AlphaMode::Mask { .. } => AlphaModeKey::Mask,
            AlphaMode::Blend => AlphaModeKey::Blend,
        }
    }
}

impl AlphaModeKey {
    fn suffix(&self) -> &'static str {
        match self {
            AlphaModeKey::Opaque => "Opaque",
            AlphaModeKey::Mask => "Mask",
            AlphaModeKey::Blend => "Blend",
        }
    }
}

/// 머티리얼 변형 키
///
/// 동일한 셰이딩 모델 + 알파 모드 + 양면 조합은 동일한 파이프라인 변형을 사용.
/// UE5.7의 Material Instance 변형 패턴 참고.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct MaterialVariantKey {
    pub shading_model: GltfShadingModel,
    pub alpha_mode: AlphaModeKey,
    pub double_sided: bool,
}

impl MaterialVariantKey {
    /// 새 변형 키 생성
    pub fn new(shading_model: GltfShadingModel, alpha_mode: AlphaMode, double_sided: bool) -> Self {
        Self {
            shading_model,
            alpha_mode: AlphaModeKey::from(alpha_mode),
            double_sided,
        }
    }

    /// "MI_MetallicRoughness_Opaque_DS" 형태의 식별자
    pub fn identifier(&self) -> String {
        let model_name = match self.shading_model {
            GltfShadingModel::MetallicRoughness => "MetallicRoughness",
            GltfShadingModel::Unlit => "Unlit",
            GltfShadingModel::ClearCoat => "ClearCoat",
            GltfShadingModel::Sheen => "Sheen",
            GltfShadingModel::Transmission => "Transmission",
            GltfShadingModel::SpecularGlossiness => "SpecGloss",
            GltfShadingModel::Volume => "Volume",
        };

        let ds = if self.double_sided { "_DS" } else { "" };
        format!("MI_{}_{}{}", model_name, self.alpha_mode.suffix(), ds)
    }

    /// 전체 변형 열거
    #[allow(dead_code)]
    pub fn all_variants() -> Vec<Self> {
        let models = [
            GltfShadingModel::MetallicRoughness,
            GltfShadingModel::Unlit,
            GltfShadingModel::ClearCoat,
            GltfShadingModel::Sheen,
            GltfShadingModel::Transmission,
            GltfShadingModel::SpecularGlossiness,
            GltfShadingModel::Volume,
        ];
        let alpha_modes = [AlphaModeKey::Opaque, AlphaModeKey::Mask, AlphaModeKey::Blend];
        let double_sided = [false, true];

        let mut variants = Vec::new();
        for &model in &models {
            for &alpha in &alpha_modes {
                for &ds in &double_sided {
                    variants.push(Self {
                        shading_model: model,
                        alpha_mode: alpha,
                        double_sided: ds,
                    });
                }
            }
        }
        variants
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variant_identifier() {
        let key = MaterialVariantKey::new(
            GltfShadingModel::MetallicRoughness,
            AlphaMode::Opaque,
            false,
        );
        assert_eq!(key.identifier(), "MI_MetallicRoughness_Opaque");

        let key_ds = MaterialVariantKey::new(
            GltfShadingModel::Unlit,
            AlphaMode::Blend,
            true,
        );
        assert_eq!(key_ds.identifier(), "MI_Unlit_Blend_DS");
    }

    #[test]
    fn test_all_variants_count() {
        let variants = MaterialVariantKey::all_variants();
        // 7 models * 3 alpha modes * 2 double_sided = 42
        assert_eq!(variants.len(), 42);
    }
}
