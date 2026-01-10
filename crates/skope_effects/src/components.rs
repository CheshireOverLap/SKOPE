// SKOPE Engine - Effect ECS Components
// Phase E1: FlipbookEffect, VatEffect components

use super::data::*;
use bevy_ecs::prelude::*;

/// Flipbook 이펙트 컴포넌트
#[derive(Component, Debug, Clone)]
pub struct FlipbookEffect {
    /// 에셋 이름 (EffectAssets에서 조회)
    pub asset_name: String,
    /// 현재 프레임 (0.0 ~ frame_count)
    pub current_frame: f32,
    /// 재생 속도 배율
    pub speed: f32,
    /// 일시정지
    pub paused: bool,
    /// 역재생 (PingPong 모드용)
    pub reverse: bool,
    /// 삭제 예정 플래그
    pub should_despawn: bool,
    /// 완료 콜백 (Lua 함수 레퍼런스)
    pub on_complete_ref: Option<i32>,
    /// 루프 모드 오버라이드
    pub loop_mode: Option<LoopMode>,
    /// 색상 틴트
    pub color: [f32; 4],
    /// 이미션 강도
    pub emission: f32,
    /// 부착된 엔티티 (따라다님)
    pub attached_to: Option<Entity>,
    /// 오프셋 (부착 시)
    pub offset: [f32; 3],
}

impl Default for FlipbookEffect {
    fn default() -> Self {
        Self {
            asset_name: String::new(),
            current_frame: 0.0,
            speed: 1.0,
            paused: false,
            reverse: false,
            should_despawn: false,
            on_complete_ref: None,
            loop_mode: None,
            color: [1.0, 1.0, 1.0, 1.0],
            emission: 1.0,
            attached_to: None,
            offset: [0.0, 0.0, 0.0],
        }
    }
}

impl FlipbookEffect {
    pub fn new(asset_name: &str) -> Self {
        Self {
            asset_name: asset_name.to_string(),
            ..Default::default()
        }
    }

    /// 프레임 업데이트 (delta: 초 단위)
    pub fn update(&mut self, delta: f32, meta: &FlipbookMeta) {
        if self.paused || self.should_despawn {
            return;
        }

        let loop_mode = self.loop_mode.unwrap_or(meta.loop_mode);
        let frame_delta = delta * meta.fps * self.speed;
        let max_frame = meta.frame_count as f32;

        if self.reverse {
            self.current_frame -= frame_delta;
        } else {
            self.current_frame += frame_delta;
        }

        match loop_mode {
            LoopMode::Once => {
                if self.current_frame >= max_frame {
                    self.current_frame = max_frame - 0.001;
                    self.should_despawn = true;
                }
            }
            LoopMode::Loop => {
                if self.current_frame >= max_frame {
                    self.current_frame %= max_frame;
                }
                if self.current_frame < 0.0 {
                    self.current_frame = max_frame + (self.current_frame % max_frame);
                }
            }
            LoopMode::Hold => {
                if self.current_frame >= max_frame {
                    self.current_frame = max_frame - 0.001;
                    self.paused = true;
                }
            }
            LoopMode::PingPong => {
                if self.current_frame >= max_frame {
                    self.current_frame = max_frame - 0.001;
                    self.reverse = true;
                } else if self.current_frame < 0.0 {
                    self.current_frame = 0.0;
                    self.reverse = false;
                }
            }
        }
    }

    /// 프레임 블렌드 값 계산
    pub fn frame_blend(&self) -> f32 {
        self.current_frame.fract()
    }

    /// 현재 정수 프레임
    pub fn frame_index(&self) -> u32 {
        self.current_frame.floor() as u32
    }
}

/// VAT 이펙트 컴포넌트
#[derive(Component, Debug, Clone)]
pub struct VatEffect {
    /// 에셋 이름 (EffectAssets에서 조회)
    pub asset_name: String,
    /// 현재 프레임 (0.0 ~ frame_count)
    pub current_frame: f32,
    /// 재생 속도 배율
    pub speed: f32,
    /// 일시정지
    pub paused: bool,
    /// 역재생
    pub reverse: bool,
    /// 삭제 예정 플래그
    pub should_despawn: bool,
    /// 완료 콜백 (Lua 함수 레퍼런스)
    pub on_complete_ref: Option<i32>,
    /// 루프 모드 오버라이드
    pub loop_mode: Option<LoopMode>,
    /// 색상 틴트
    pub color: [f32; 4],
}

impl Default for VatEffect {
    fn default() -> Self {
        Self {
            asset_name: String::new(),
            current_frame: 0.0,
            speed: 1.0,
            paused: false,
            reverse: false,
            should_despawn: false,
            on_complete_ref: None,
            loop_mode: None,
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

impl VatEffect {
    pub fn new(asset_name: &str) -> Self {
        Self {
            asset_name: asset_name.to_string(),
            ..Default::default()
        }
    }

    /// 프레임 업데이트
    pub fn update(&mut self, delta: f32, meta: &VatMeta) {
        if self.paused || self.should_despawn {
            return;
        }

        let loop_mode = self.loop_mode.unwrap_or(meta.loop_mode);
        let frame_delta = delta * meta.fps * self.speed;
        let max_frame = meta.frame_count as f32;

        if self.reverse {
            self.current_frame -= frame_delta;
        } else {
            self.current_frame += frame_delta;
        }

        match loop_mode {
            LoopMode::Once => {
                if self.current_frame >= max_frame {
                    self.current_frame = max_frame - 0.001;
                    self.should_despawn = true;
                }
            }
            LoopMode::Loop => {
                if self.current_frame >= max_frame {
                    self.current_frame %= max_frame;
                }
                if self.current_frame < 0.0 {
                    self.current_frame = max_frame + (self.current_frame % max_frame);
                }
            }
            LoopMode::Hold => {
                if self.current_frame >= max_frame {
                    self.current_frame = max_frame - 0.001;
                    self.paused = true;
                }
            }
            LoopMode::PingPong => {
                if self.current_frame >= max_frame {
                    self.current_frame = max_frame - 0.001;
                    self.reverse = true;
                } else if self.current_frame < 0.0 {
                    self.current_frame = 0.0;
                    self.reverse = false;
                }
            }
        }
    }

    /// 프레임 블렌드 값 계산
    pub fn frame_blend(&self) -> f32 {
        self.current_frame.fract()
    }

    /// 현재 정수 프레임
    pub fn frame_index(&self) -> u32 {
        self.current_frame.floor() as u32
    }
}

/// Transform 컴포넌트 (기존 ECS에 있으면 사용, 없으면 정의)
#[derive(Component, Debug, Clone, Copy)]
pub struct EffectTransform {
    pub position: [f32; 3],
    pub rotation: [f32; 4], // Quaternion
    pub scale: [f32; 3],
}

impl Default for EffectTransform {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
        }
    }
}

impl EffectTransform {
    pub fn from_position(pos: [f32; 3]) -> Self {
        Self {
            position: pos,
            ..Default::default()
        }
    }

    pub fn to_matrix(&self) -> [[f32; 4]; 4] {
        // 간단한 TRS 행렬 생성
        let [qx, qy, qz, qw] = self.rotation;
        let [sx, sy, sz] = self.scale;
        let [tx, ty, tz] = self.position;

        // 쿼터니온 -> 회전 행렬
        let xx = qx * qx;
        let yy = qy * qy;
        let zz = qz * qz;
        let xy = qx * qy;
        let xz = qx * qz;
        let yz = qy * qz;
        let wx = qw * qx;
        let wy = qw * qy;
        let wz = qw * qz;

        [
            [sx * (1.0 - 2.0 * (yy + zz)), sx * 2.0 * (xy + wz), sx * 2.0 * (xz - wy), 0.0],
            [sy * 2.0 * (xy - wz), sy * (1.0 - 2.0 * (xx + zz)), sy * 2.0 * (yz + wx), 0.0],
            [sz * 2.0 * (xz + wy), sz * 2.0 * (yz - wx), sz * (1.0 - 2.0 * (xx + yy)), 0.0],
            [tx, ty, tz, 1.0],
        ]
    }
}

/// 통합 이펙트 인스턴스 컴포넌트
/// EffectDefinition을 기반으로 스폰된 이펙트의 런타임 상태
#[derive(Component, Debug, Clone)]
pub struct EffectInstance {
    /// 이펙트 정의 이름 (에셋 레지스트리에서 조회)
    pub definition_name: String,
    /// 현재 재생 시간 (초)
    pub current_time: f32,
    /// 재생 속도 배율
    pub speed: f32,
    /// 일시정지 여부
    pub paused: bool,
    /// 삭제 예정 플래그
    pub should_despawn: bool,
    /// 색상 틴트 (전체 이펙트에 적용)
    pub color: [f32; 4],
    /// 스케일 (전체 이펙트에 적용)
    pub scale: f32,
    /// 부착된 엔티티 (따라다님)
    pub attached_to: Option<Entity>,
    /// 오프셋 (부착 시)
    pub offset: [f32; 3],
    /// 모듈별 상태
    pub module_states: Vec<ModuleState>,
    /// 완료 콜백 (Lua 함수 레퍼런스)
    pub on_complete_ref: Option<i32>,
}

impl Default for EffectInstance {
    fn default() -> Self {
        Self {
            definition_name: String::new(),
            current_time: 0.0,
            speed: 1.0,
            paused: false,
            should_despawn: false,
            color: [1.0, 1.0, 1.0, 1.0],
            scale: 1.0,
            attached_to: None,
            offset: [0.0, 0.0, 0.0],
            module_states: Vec::new(),
            on_complete_ref: None,
        }
    }
}

impl EffectInstance {
    pub fn new(definition_name: &str) -> Self {
        Self {
            definition_name: definition_name.to_string(),
            ..Default::default()
        }
    }

    /// 정규화된 재생 시간 (0.0 ~ 1.0)
    pub fn normalized_time(&self, duration: f32) -> f32 {
        if duration <= 0.0 {
            return 0.0;
        }
        (self.current_time / duration).clamp(0.0, 1.0)
    }
}

/// 모듈별 런타임 상태
#[derive(Debug, Clone)]
pub enum ModuleState {
    /// 파티클 모듈 상태
    Particle {
        /// 스폰된 ParticleEmitter 엔티티
        entity: Option<Entity>,
        /// 누적 스폰 시간 (연속 스폰용)
        spawn_accumulator: f32,
        /// GPU 파이프라인 인덱스 (GPU 파티클용)
        gpu_pipeline_index: Option<usize>,
        /// 스폰된 파티클 수
        spawned_count: u32,
        /// 활성 상태
        active: bool,
    },
    /// Flipbook 모듈 상태
    Flipbook {
        /// 스폰된 엔티티
        entity: Option<Entity>,
        /// 활성 상태
        active: bool,
    },
    /// VAT 모듈 상태
    Vat {
        /// 스폰된 엔티티
        entity: Option<Entity>,
        /// 활성 상태
        active: bool,
    },
}

impl ModuleState {
    pub fn new_particle(use_gpu: bool) -> Self {
        ModuleState::Particle {
            entity: None,
            spawn_accumulator: 0.0,
            gpu_pipeline_index: if use_gpu { Some(0) } else { None },
            spawned_count: 0,
            active: false,
        }
    }

    pub fn new_flipbook() -> Self {
        ModuleState::Flipbook {
            entity: None,
            active: false,
        }
    }

    pub fn new_vat() -> Self {
        ModuleState::Vat {
            entity: None,
            active: false,
        }
    }

    pub fn is_active(&self) -> bool {
        match self {
            ModuleState::Particle { active, .. } => *active,
            ModuleState::Flipbook { active, .. } => *active,
            ModuleState::Vat { active, .. } => *active,
        }
    }

    pub fn set_active(&mut self, value: bool) {
        match self {
            ModuleState::Particle { active, .. } => *active = value,
            ModuleState::Flipbook { active, .. } => *active = value,
            ModuleState::Vat { active, .. } => *active = value,
        }
    }
}
