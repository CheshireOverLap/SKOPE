//! SKOPE Engine - Unified Effect Definition
//!
//! 통합 이펙트 정의: 파티클, Flipbook, VAT를 조합하여 하나의 이펙트로 관리

use serde::{Deserialize, Serialize};
use crate::data::{LoopMode, BlendMode, BillboardType, VatType};

/// 통합 이펙트 정의 (RON 파일에서 로드)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectDefinition {
    /// 이펙트 이름
    pub name: String,
    /// 전체 재생 시간 (초)
    pub duration: f32,
    /// 루프 모드
    #[serde(default)]
    pub loop_mode: LoopMode,
    /// 이펙트 모듈들
    pub modules: Vec<EffectModule>,
    /// 이벤트 트리거
    #[serde(default)]
    pub events: Vec<EffectEvent>,
}

impl Default for EffectDefinition {
    fn default() -> Self {
        Self {
            name: String::new(),
            duration: 1.0,
            loop_mode: LoopMode::Once,
            modules: Vec::new(),
            events: Vec::new(),
        }
    }
}

/// 이펙트 모듈 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EffectModule {
    /// 파티클 모듈
    Particle(ParticleModuleDef),
    /// Flipbook 스프라이트 모듈
    Flipbook(FlipbookModuleDef),
    /// VAT (Vertex Animation Texture) 모듈
    Vat(VatModuleDef),
}

/// 파티클 모듈 정의
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticleModuleDef {
    /// 시작 딜레이 (초)
    #[serde(default)]
    pub start_delay: f32,
    /// 지속 시간 (-1 = 전체 재생시간)
    #[serde(default = "default_negative_one")]
    pub duration: f32,
    /// GPU 파티클 사용 여부
    #[serde(default)]
    pub use_gpu: bool,
    /// 파티클 수
    #[serde(default = "default_particle_count")]
    pub particle_count: u32,
    /// 초당 스폰 수
    #[serde(default = "default_spawn_rate")]
    pub spawn_rate: f32,
    /// 버스트 스폰 (시작 시 즉시 스폰)
    #[serde(default)]
    pub burst_count: u32,
    /// 스폰 형태
    #[serde(default)]
    pub spawn_shape: SpawnShape,
    /// 수명 범위
    #[serde(default = "default_lifetime")]
    pub lifetime: RangeF32,
    /// 초기 속도 범위
    #[serde(default)]
    pub velocity: VelocityDef,
    /// 크기 범위
    #[serde(default = "default_size_range")]
    pub size: RangeF32,
    /// 크기 커브 (수명에 따른 변화)
    #[serde(default)]
    pub size_over_lifetime: Option<CurveDef>,
    /// 시작 색상
    #[serde(default = "default_color")]
    pub color_start: [f32; 4],
    /// 끝 색상
    #[serde(default = "default_color_fade")]
    pub color_end: [f32; 4],
    /// 중력
    #[serde(default = "default_gravity")]
    pub gravity: [f32; 3],
    /// Force fields
    #[serde(default)]
    pub force_fields: Vec<ForceFieldDef>,
    /// 회전 속도 범위
    #[serde(default)]
    pub rotation_speed: RangeF32,
    /// 블렌드 모드
    #[serde(default)]
    pub blend_mode: BlendMode,
}

impl Default for ParticleModuleDef {
    fn default() -> Self {
        Self {
            start_delay: 0.0,
            duration: -1.0,
            use_gpu: false,
            particle_count: 1000,
            spawn_rate: 100.0,
            burst_count: 0,
            spawn_shape: SpawnShape::default(),
            lifetime: RangeF32 { min: 1.0, max: 2.0 },
            velocity: VelocityDef::default(),
            size: RangeF32 { min: 0.05, max: 0.15 },
            size_over_lifetime: None,
            color_start: [1.0, 1.0, 1.0, 1.0],
            color_end: [1.0, 1.0, 1.0, 0.0],
            gravity: [0.0, -9.8, 0.0],
            force_fields: Vec::new(),
            rotation_speed: RangeF32 { min: -1.0, max: 1.0 },
            blend_mode: BlendMode::Alpha,
        }
    }
}

/// Flipbook 모듈 정의
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlipbookModuleDef {
    /// 시작 딜레이 (초)
    #[serde(default)]
    pub start_delay: f32,
    /// 텍스처 에셋 이름
    pub asset: String,
    /// 오프셋 위치
    #[serde(default)]
    pub offset: [f32; 3],
    /// 스케일
    #[serde(default = "default_one")]
    pub scale: f32,
    /// 재생 속도
    #[serde(default = "default_one")]
    pub speed: f32,
    /// 색상 틴트
    #[serde(default = "default_color")]
    pub color: [f32; 4],
    /// 이미션 강도
    #[serde(default = "default_one")]
    pub emission: f32,
    /// 빌보드 타입
    #[serde(default)]
    pub billboard: BillboardType,
}

impl Default for FlipbookModuleDef {
    fn default() -> Self {
        Self {
            start_delay: 0.0,
            asset: String::new(),
            offset: [0.0, 0.0, 0.0],
            scale: 1.0,
            speed: 1.0,
            color: [1.0, 1.0, 1.0, 1.0],
            emission: 1.0,
            billboard: BillboardType::CameraFacing,
        }
    }
}

/// VAT 모듈 정의
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VatModuleDef {
    /// 시작 딜레이 (초)
    #[serde(default)]
    pub start_delay: f32,
    /// VAT 에셋 이름
    pub asset: String,
    /// 오프셋 위치
    #[serde(default)]
    pub offset: [f32; 3],
    /// 스케일
    #[serde(default = "default_one")]
    pub scale: f32,
    /// 재생 속도
    #[serde(default = "default_one")]
    pub speed: f32,
    /// 색상 틴트
    #[serde(default = "default_color")]
    pub color: [f32; 4],
    /// VAT 타입
    #[serde(default)]
    pub vat_type: VatType,
}

impl Default for VatModuleDef {
    fn default() -> Self {
        Self {
            start_delay: 0.0,
            asset: String::new(),
            offset: [0.0, 0.0, 0.0],
            scale: 1.0,
            speed: 1.0,
            color: [1.0, 1.0, 1.0, 1.0],
            vat_type: VatType::Soft,
        }
    }
}

/// 스폰 형태 정의
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "shape")]
pub enum SpawnShape {
    /// 점 스폰
    Point,
    /// 박스 영역
    Box { extent: [f32; 3] },
    /// 구 영역
    Sphere { radius: f32 },
    /// 원뿔 영역
    Cone { angle: f32, height: f32 },
    /// 원 (디스크)
    Circle { radius: f32 },
    /// 에지 (선)
    Edge { length: f32 },
    /// 링 (도넛)
    Ring { inner_radius: f32, outer_radius: f32 },
    /// 반구
    Hemisphere { radius: f32 },
}

impl Default for SpawnShape {
    fn default() -> Self {
        SpawnShape::Point
    }
}

/// 속도 정의
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode")]
pub enum VelocityDef {
    /// 랜덤 박스 범위
    Random { min: [f32; 3], max: [f32; 3] },
    /// 방사형 (스폰 위치에서 바깥으로)
    Radial { speed: RangeF32 },
    /// 원뿔형 (방향 + 각도)
    Cone { direction: [f32; 3], angle: f32, speed: RangeF32 },
}

impl Default for VelocityDef {
    fn default() -> Self {
        VelocityDef::Random {
            min: [-1.0, 2.0, -1.0],
            max: [1.0, 5.0, 1.0],
        }
    }
}

/// 범위 값 (f32)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RangeF32 {
    pub min: f32,
    pub max: f32,
}

impl Default for RangeF32 {
    fn default() -> Self {
        Self { min: 0.0, max: 1.0 }
    }
}

/// 커브 정의 (수명에 따른 값 변화)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurveDef {
    /// 키프레임 (시간 0~1, 값)
    pub keys: Vec<(f32, f32)>,
}

impl CurveDef {
    /// 시간 t (0~1)에서 값 샘플링
    pub fn sample(&self, t: f32) -> f32 {
        if self.keys.is_empty() {
            return 1.0;
        }
        if self.keys.len() == 1 {
            return self.keys[0].1;
        }

        let t = t.clamp(0.0, 1.0);

        // 이전 키와 다음 키 찾기
        let mut prev_idx = 0;
        for (i, &(key_t, _)) in self.keys.iter().enumerate() {
            if key_t <= t {
                prev_idx = i;
            }
        }

        if prev_idx >= self.keys.len() - 1 {
            return self.keys.last().unwrap().1;
        }

        let (t0, v0) = self.keys[prev_idx];
        let (t1, v1) = self.keys[prev_idx + 1];

        // 선형 보간
        let alpha = (t - t0) / (t1 - t0);
        v0 + (v1 - v0) * alpha
    }
}

/// Force Field 정의
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ForceFieldDef {
    /// 터뷸런스 (난류)
    Turbulence {
        strength: f32,
        frequency: f32,
        #[serde(default = "default_octaves")]
        octaves: u32,
    },
    /// 보텍스 (회오리)
    Vortex {
        position: [f32; 3],
        axis: [f32; 3],
        strength: f32,
        radius: f32,
    },
    /// 어트랙터 (끌어당김)
    Attractor {
        position: [f32; 3],
        strength: f32,
        radius: f32,
    },
    /// 바람
    Wind {
        direction: [f32; 3],
        strength: f32,
    },
    /// 드래그 (공기 저항)
    Drag {
        coefficient: f32,
    },
}

/// 이펙트 이벤트 (Lua 콜백 트리거)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectEvent {
    /// 이벤트 이름
    pub name: String,
    /// 트리거 시간 (초)
    pub time: f32,
    /// Lua 콜백 함수 이름
    #[serde(default)]
    pub callback: Option<String>,
}

// Default value helpers
fn default_negative_one() -> f32 { -1.0 }
fn default_particle_count() -> u32 { 1000 }
fn default_spawn_rate() -> f32 { 100.0 }
fn default_lifetime() -> RangeF32 { RangeF32 { min: 1.0, max: 2.0 } }
fn default_size_range() -> RangeF32 { RangeF32 { min: 0.05, max: 0.15 } }
fn default_color() -> [f32; 4] { [1.0, 1.0, 1.0, 1.0] }
fn default_color_fade() -> [f32; 4] { [1.0, 1.0, 1.0, 0.0] }
fn default_gravity() -> [f32; 3] { [0.0, -9.8, 0.0] }
fn default_one() -> f32 { 1.0 }
fn default_octaves() -> u32 { 3 }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effect_def_serialization() {
        // RON 파일에서 직접 파싱하는 테스트
        // Note: EffectModule uses #[serde(tag = "type")] for internal tagging
        let ron_str = r#"
        (
            name: "test_fire",
            duration: 2.0,
            loop_mode: Once,
            modules: [
                (
                    type: "Particle",
                    use_gpu: true,
                    particle_count: 500,
                    spawn_rate: 50.0,
                ),
                (
                    type: "Flipbook",
                    asset: "fire_flipbook",
                ),
            ],
            events: [
                (
                    name: "on_burst",
                    time: 0.0,
                    callback: Some("handle_fire_burst"),
                ),
            ],
        )
        "#;

        let parsed: EffectDefinition = ron::from_str(ron_str).unwrap();
        assert_eq!(parsed.name, "test_fire");
        assert_eq!(parsed.duration, 2.0);
        assert_eq!(parsed.modules.len(), 2);
        assert_eq!(parsed.events.len(), 1);

        // 모듈 타입 확인
        match &parsed.modules[0] {
            EffectModule::Particle(p) => {
                assert!(p.use_gpu);
                assert_eq!(p.particle_count, 500);
            }
            _ => panic!("Expected Particle module"),
        }

        match &parsed.modules[1] {
            EffectModule::Flipbook(f) => {
                assert_eq!(f.asset, "fire_flipbook");
            }
            _ => panic!("Expected Flipbook module"),
        }
    }

    #[test]
    fn test_curve_sample() {
        let curve = CurveDef {
            keys: vec![
                (0.0, 1.0),
                (0.5, 2.0),
                (1.0, 0.0),
            ],
        };

        assert!((curve.sample(0.0) - 1.0).abs() < 0.001);
        assert!((curve.sample(0.25) - 1.5).abs() < 0.001);
        assert!((curve.sample(0.5) - 2.0).abs() < 0.001);
        assert!((curve.sample(1.0) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_ring_spawn_shape() {
        // Test Ring spawn shape parsing (used in healing_aura.effect.ron)
        let ron_str = r#"
        (
            name: "ring_test",
            duration: 3.0,
            loop_mode: Loop,
            modules: [
                (
                    type: "Particle",
                    spawn_shape: (shape: "Ring", inner_radius: 0.5, outer_radius: 1.0),
                    particle_count: 200,
                    spawn_rate: 25.0,
                ),
            ],
            events: [],
        )
        "#;

        let parsed: EffectDefinition = ron::from_str(ron_str).unwrap();
        assert_eq!(parsed.name, "ring_test");
        assert_eq!(parsed.modules.len(), 1);

        match &parsed.modules[0] {
            EffectModule::Particle(p) => {
                match &p.spawn_shape {
                    SpawnShape::Ring { inner_radius, outer_radius } => {
                        assert!((inner_radius - 0.5).abs() < 0.001);
                        assert!((outer_radius - 1.0).abs() < 0.001);
                    }
                    _ => panic!("Expected Ring spawn shape"),
                }
            }
            _ => panic!("Expected Particle module"),
        }
    }

    #[test]
    fn test_force_fields() {
        // Test ForceField parsing (Vortex, Turbulence, Drag)
        let ron_str = r#"
        (
            name: "force_test",
            duration: 2.0,
            modules: [
                (
                    type: "Particle",
                    force_fields: [
                        (
                            type: "Vortex",
                            position: [0.0, 1.0, 0.0],
                            axis: [0.0, 1.0, 0.0],
                            strength: 0.3,
                            radius: 1.5,
                        ),
                        (
                            type: "Turbulence",
                            strength: 1.0,
                            frequency: 0.8,
                            octaves: 3,
                        ),
                        (
                            type: "Drag",
                            coefficient: 0.5,
                        ),
                    ],
                ),
            ],
        )
        "#;

        let parsed: EffectDefinition = ron::from_str(ron_str).unwrap();

        match &parsed.modules[0] {
            EffectModule::Particle(p) => {
                assert_eq!(p.force_fields.len(), 3);

                match &p.force_fields[0] {
                    ForceFieldDef::Vortex { strength, .. } => {
                        assert!((strength - 0.3).abs() < 0.001);
                    }
                    _ => panic!("Expected Vortex"),
                }

                match &p.force_fields[1] {
                    ForceFieldDef::Turbulence { octaves, .. } => {
                        assert_eq!(*octaves, 3);
                    }
                    _ => panic!("Expected Turbulence"),
                }

                match &p.force_fields[2] {
                    ForceFieldDef::Drag { coefficient } => {
                        assert!((coefficient - 0.5).abs() < 0.001);
                    }
                    _ => panic!("Expected Drag"),
                }
            }
            _ => panic!("Expected Particle module"),
        }
    }

    #[test]
    fn test_flipbook_module_parsing() {
        // Test Flipbook module with billboard
        // RON requires enum variants as strings in EffectModule (internally tagged)
        let ron_str = r#"
        (
            name: "test_flipbook",
            duration: 1.0,
            modules: [
                (
                    type: "Flipbook",
                    start_delay: 0.0,
                    asset: "explosion_flash",
                    offset: [0.0, 0.0, 0.0],
                    scale: 3.0,
                    speed: 2.0,
                    color: [1.0, 0.9, 0.7, 1.0],
                    emission: 5.0,
                    billboard: "FullCameraFacing",
                ),
            ],
            events: [],
        )
        "#;

        let parsed: EffectDefinition = ron::from_str(ron_str).unwrap();
        assert_eq!(parsed.name, "test_flipbook");
        assert_eq!(parsed.modules.len(), 1);
    }

    #[test]
    fn test_full_explosion_effect() {
        // Test full explosion effect similar to explosion.effect.ron
        let ron_str = r#"
        (
            name: "explosion",
            duration: 3.0,
            loop_mode: Once,
            modules: [
                (
                    type: "Flipbook",
                    start_delay: 0.0,
                    asset: "explosion_flash",
                    offset: [0.0, 0.0, 0.0],
                    scale: 3.0,
                    speed: 2.0,
                    color: [1.0, 0.9, 0.7, 1.0],
                    emission: 5.0,
                    billboard: "FullCameraFacing",
                ),
                (
                    type: "Vat",
                    start_delay: 0.05,
                    asset: "debris_scatter",
                    offset: [0.0, 0.0, 0.0],
                    scale: 1.0,
                    speed: 1.0,
                    color: [0.8, 0.7, 0.6, 1.0],
                    vat_type: "Rigid",
                ),
                (
                    type: "Particle",
                    start_delay: 0.0,
                    duration: 0.5,
                    use_gpu: true,
                    particle_count: 150,
                    spawn_rate: 0.0,
                    burst_count: 80,
                    spawn_shape: (shape: "Sphere", radius: 0.5),
                    lifetime: (min: 0.3, max: 0.8),
                    velocity: (mode: "Radial", speed: (min: 5.0, max: 12.0)),
                    size: (min: 0.1, max: 0.3),
                    color_start: [1.0, 0.7, 0.2, 1.0],
                    color_end: [1.0, 0.3, 0.0, 0.0],
                    gravity: [0.0, -3.0, 0.0],
                    force_fields: [],
                    rotation_speed: (min: -3.0, max: 3.0),
                    blend_mode: "Additive",
                ),
            ],
            events: [],
        )
        "#;

        let parsed: EffectDefinition = ron::from_str(ron_str).unwrap();
        assert_eq!(parsed.name, "explosion");
        assert_eq!(parsed.modules.len(), 3);
    }
}
