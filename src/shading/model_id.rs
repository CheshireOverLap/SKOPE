// SKOPE Engine - Shading Model ID
// 각 머티리얼 타입을 G-Buffer에서 구분하기 위한 ID

/// Shading Model 식별자
/// G-Buffer RT1.w에 저장되어 Deferred Lighting에서 분기 처리
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ShadingModelId {
    /// 표준 PBR (환경, 소품)
    #[default]
    StandardPBR = 0,
    /// 얼굴 전용 (노멀 시프트 + 색조작)
    Face = 1,
    /// 피부 (SSS + 색조작)
    Skin = 2,
    /// 눈
    Eye = 3,
    /// 머리카락 Card
    HairCard = 4,
    /// 머리카락 Strand
    HairStrand = 5,
    /// 외곽선
    Outline = 6,
    /// 투명/반투명
    Translucent = 7,
}

impl ShadingModelId {
    /// G-Buffer에 저장할 값 (0~1 범위로 정규화)
    pub fn to_gbuffer_value(self) -> f32 {
        (self as u8) as f32 / 255.0
    }

    /// G-Buffer 값에서 복원
    pub fn from_gbuffer_value(value: f32) -> Self {
        let id = (value * 255.0).round() as u8;
        match id {
            0 => Self::StandardPBR,
            1 => Self::Face,
            2 => Self::Skin,
            3 => Self::Eye,
            4 => Self::HairCard,
            5 => Self::HairStrand,
            6 => Self::Outline,
            7 => Self::Translucent,
            _ => Self::StandardPBR,
        }
    }

    /// 셰이더에서 사용할 u32 상수
    pub fn shader_constant(self) -> u32 {
        self as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gbuffer_roundtrip() {
        for id in [
            ShadingModelId::StandardPBR,
            ShadingModelId::Face,
            ShadingModelId::Skin,
            ShadingModelId::Eye,
        ] {
            let value = id.to_gbuffer_value();
            let restored = ShadingModelId::from_gbuffer_value(value);
            assert_eq!(id, restored);
        }
    }
}
