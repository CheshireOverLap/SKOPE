// Animation Blending System for SKOPE Engine
// Phase: Animation Blending (Multi-animation mixing)

use glam::{Quat, Vec3};
use std::collections::HashMap;
use crate::gltf_loader::Animation;
use super::animation::{sample_animation, NodeTransform};

/// 블렌딩 가능한 애니메이션 인스턴스
#[derive(Debug, Clone)]
pub struct AnimationInstance {
    /// 애니메이션 인덱스 (GltfScene.animations 배열의 인덱스)
    pub animation_index: usize,
    /// 현재 재생 시간 (초)
    pub time: f32,
    /// 블렌딩 가중치 (0.0 ~ 1.0)
    pub weight: f32,
    /// 재생 속도 (1.0 = 정상)
    pub speed: f32,
    /// 루프 재생 여부
    pub looping: bool,
    /// 재생 중인지
    pub playing: bool,
    /// 블렌딩 모드
    pub blend_mode: BlendMode,
}

impl Default for AnimationInstance {
    fn default() -> Self {
        Self {
            animation_index: 0,
            time: 0.0,
            weight: 1.0,
            speed: 1.0,
            looping: true,
            playing: true,
            blend_mode: BlendMode::Override,
        }
    }
}

impl AnimationInstance {
    /// 새 인스턴스 생성
    pub fn new(animation_index: usize) -> Self {
        Self {
            animation_index,
            ..Default::default()
        }
    }

    /// 가중치와 함께 생성
    pub fn with_weight(animation_index: usize, weight: f32) -> Self {
        Self {
            animation_index,
            weight,
            ..Default::default()
        }
    }

    /// 시간 업데이트
    pub fn update(&mut self, delta_seconds: f32, animation_duration: f32) {
        if !self.playing || animation_duration <= 0.0 {
            return;
        }

        self.time += delta_seconds * self.speed;

        if self.looping {
            while self.time >= animation_duration {
                self.time -= animation_duration;
            }
            while self.time < 0.0 {
                self.time += animation_duration;
            }
        } else {
            self.time = self.time.clamp(0.0, animation_duration);
            if self.time >= animation_duration {
                self.playing = false;
            }
        }
    }
}

/// 블렌딩 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlendMode {
    /// Override: 기본 포즈를 대체 (일반적인 블렌딩)
    #[default]
    Override,
    /// Additive: 기존 포즈에 더함 (표정, 반동 등)
    Additive,
}

/// 블렌딩된 노드 변환 (항상 값이 있음)
#[derive(Debug, Clone)]
pub struct BlendedNodeTransform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for BlendedNodeTransform {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl BlendedNodeTransform {
    /// 두 트랜스폼을 가중치로 블렌딩
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        Self {
            translation: self.translation.lerp(other.translation, t),
            rotation: self.rotation.slerp(other.rotation, t),
            scale: self.scale.lerp(other.scale, t),
        }
    }

    /// Additive 블렌딩 (other의 delta를 가중치만큼 더함)
    pub fn add(&self, other_delta: &Self, weight: f32) -> Self {
        Self {
            translation: self.translation + other_delta.translation * weight,
            rotation: self.rotation * Quat::IDENTITY.slerp(other_delta.rotation, weight),
            scale: self.scale + (other_delta.scale - Vec3::ONE) * weight,
        }
    }

    /// NodeTransform에서 변환 (기본값 사용)
    pub fn from_node_transform(nt: &NodeTransform, defaults: &crate::gltf_loader::Transform) -> Self {
        Self {
            translation: nt.translation.unwrap_or_else(|| Vec3::from_array(defaults.translation)),
            rotation: nt.rotation.unwrap_or_else(|| Quat::from_array(defaults.rotation)),
            scale: nt.scale.unwrap_or_else(|| Vec3::from_array(defaults.scale)),
        }
    }
}

/// 애니메이션 믹서 (여러 애니메이션 블렌딩)
#[derive(Debug, Clone, Default)]
pub struct AnimationMixer {
    /// 활성 애니메이션 인스턴스들
    pub instances: Vec<AnimationInstance>,
    /// 캐시된 포즈 (성능 최적화용)
    cached_poses: Vec<HashMap<usize, BlendedNodeTransform>>,
}

impl AnimationMixer {
    pub fn new() -> Self {
        Self::default()
    }

    /// 애니메이션 추가
    pub fn add_animation(&mut self, animation_index: usize, weight: f32) -> usize {
        let instance = AnimationInstance::with_weight(animation_index, weight);
        self.instances.push(instance);
        self.cached_poses.push(HashMap::new());
        self.instances.len() - 1
    }

    /// 애니메이션 제거
    pub fn remove_animation(&mut self, instance_index: usize) {
        if instance_index < self.instances.len() {
            self.instances.remove(instance_index);
            self.cached_poses.remove(instance_index);
        }
    }

    /// 가중치 설정
    pub fn set_weight(&mut self, instance_index: usize, weight: f32) {
        if let Some(instance) = self.instances.get_mut(instance_index) {
            instance.weight = weight.clamp(0.0, 1.0);
        }
    }

    /// 블렌드 모드 설정
    pub fn set_blend_mode(&mut self, instance_index: usize, mode: BlendMode) {
        if let Some(instance) = self.instances.get_mut(instance_index) {
            instance.blend_mode = mode;
        }
    }

    /// 모든 인스턴스 업데이트
    pub fn update(&mut self, delta_seconds: f32, animations: &[Animation]) {
        for instance in &mut self.instances {
            if let Some(anim) = animations.get(instance.animation_index) {
                instance.update(delta_seconds, anim.duration);
            }
        }
    }

    /// 블렌딩된 포즈 샘플링
    pub fn sample(
        &mut self,
        animations: &[Animation],
        default_transforms: &[crate::gltf_loader::Transform],
    ) -> HashMap<usize, BlendedNodeTransform> {
        let mut result: HashMap<usize, BlendedNodeTransform> = HashMap::new();

        // 가중치 정규화를 위한 총합 (Override 모드만)
        let total_override_weight: f32 = self.instances.iter()
            .filter(|i| i.blend_mode == BlendMode::Override && i.weight > 0.0)
            .map(|i| i.weight)
            .sum();

        // Override 블렌딩 먼저 처리
        for (idx, instance) in self.instances.iter().enumerate() {
            if instance.weight <= 0.0 || instance.blend_mode != BlendMode::Override {
                continue;
            }

            let anim = match animations.get(instance.animation_index) {
                Some(a) => a,
                None => continue,
            };

            // 샘플링
            let sampled = sample_animation(anim, instance.time);

            // 정규화된 가중치
            let normalized_weight = if total_override_weight > 0.0 {
                instance.weight / total_override_weight
            } else {
                instance.weight
            };

            // 각 노드에 대해 블렌딩
            for (&node_idx, node_transform) in &sampled {
                let default_transform = default_transforms.get(node_idx)
                    .cloned()
                    .unwrap_or_default();

                let blended = BlendedNodeTransform::from_node_transform(node_transform, &default_transform);

                result.entry(node_idx)
                    .and_modify(|existing| {
                        *existing = existing.lerp(&blended, normalized_weight);
                    })
                    .or_insert_with(|| {
                        // 첫 애니메이션이면 가중치만큼 블렌딩
                        BlendedNodeTransform::default().lerp(&blended, normalized_weight)
                    });
            }

            // 캐시 업데이트
            if idx < self.cached_poses.len() {
                self.cached_poses[idx] = sampled.iter()
                    .map(|(&node_idx, nt)| {
                        let dt = default_transforms.get(node_idx).cloned().unwrap_or_default();
                        (node_idx, BlendedNodeTransform::from_node_transform(nt, &dt))
                    })
                    .collect();
            }
        }

        // Additive 블렌딩 (Override 결과 위에 적용)
        for instance in &self.instances {
            if instance.weight <= 0.0 || instance.blend_mode != BlendMode::Additive {
                continue;
            }

            let anim = match animations.get(instance.animation_index) {
                Some(a) => a,
                None => continue,
            };

            let sampled = sample_animation(anim, instance.time);

            for (&node_idx, node_transform) in &sampled {
                let default_transform = default_transforms.get(node_idx)
                    .cloned()
                    .unwrap_or_default();

                // Additive는 기본 포즈와의 차이를 적용
                let reference = BlendedNodeTransform {
                    translation: Vec3::from_array(default_transform.translation),
                    rotation: Quat::from_array(default_transform.rotation),
                    scale: Vec3::from_array(default_transform.scale),
                };

                let animated = BlendedNodeTransform::from_node_transform(node_transform, &default_transform);

                // Delta 계산 (애니메이션 - 기준)
                let delta = BlendedNodeTransform {
                    translation: animated.translation - reference.translation,
                    rotation: reference.rotation.inverse() * animated.rotation,
                    scale: animated.scale - reference.scale + Vec3::ONE,
                };

                result.entry(node_idx)
                    .and_modify(|existing| {
                        *existing = existing.add(&delta, instance.weight);
                    })
                    .or_insert_with(|| {
                        reference.add(&delta, instance.weight)
                    });
            }
        }

        result
    }

    /// 모든 애니메이션 정지
    pub fn stop_all(&mut self) {
        for instance in &mut self.instances {
            instance.playing = false;
        }
    }

    /// 모든 애니메이션 재생
    pub fn play_all(&mut self) {
        for instance in &mut self.instances {
            instance.playing = true;
            instance.time = 0.0;
        }
    }

    /// 활성 인스턴스 수
    pub fn active_count(&self) -> usize {
        self.instances.iter().filter(|i| i.playing && i.weight > 0.0).count()
    }

    /// 인스턴스 참조 얻기
    pub fn get(&self, index: usize) -> Option<&AnimationInstance> {
        self.instances.get(index)
    }

    /// 인스턴스 가변 참조 얻기
    pub fn get_mut(&mut self, index: usize) -> Option<&mut AnimationInstance> {
        self.instances.get_mut(index)
    }
}

/// 크로스페이드 트랜지션 헬퍼
#[derive(Debug, Clone)]
pub struct CrossfadeTransition {
    /// 페이드 아웃 중인 인스턴스 인덱스
    pub from_index: usize,
    /// 페이드 인 중인 인스턴스 인덱스
    pub to_index: usize,
    /// 트랜지션 지속 시간 (초)
    pub duration: f32,
    /// 현재 경과 시간
    pub elapsed: f32,
    /// 완료 여부
    pub completed: bool,
}

impl CrossfadeTransition {
    pub fn new(from_index: usize, to_index: usize, duration: f32) -> Self {
        Self {
            from_index,
            to_index,
            duration,
            elapsed: 0.0,
            completed: false,
        }
    }

    /// 트랜지션 업데이트
    pub fn update(&mut self, delta_seconds: f32, mixer: &mut AnimationMixer) {
        if self.completed {
            return;
        }

        self.elapsed += delta_seconds;
        let t = (self.elapsed / self.duration).clamp(0.0, 1.0);

        // from: 1 -> 0, to: 0 -> 1
        mixer.set_weight(self.from_index, 1.0 - t);
        mixer.set_weight(self.to_index, t);

        if t >= 1.0 {
            self.completed = true;
            // 페이드 아웃된 애니메이션 정지
            if let Some(instance) = mixer.get_mut(self.from_index) {
                instance.playing = false;
            }
        }
    }

    /// 완료 여부
    pub fn is_completed(&self) -> bool {
        self.completed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blended_transform_lerp() {
        let a = BlendedNodeTransform {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        };
        let b = BlendedNodeTransform {
            translation: Vec3::new(10.0, 0.0, 0.0),
            rotation: Quat::from_rotation_y(std::f32::consts::PI),
            scale: Vec3::splat(2.0),
        };

        let mid = a.lerp(&b, 0.5);
        assert!((mid.translation.x - 5.0).abs() < 0.001);
        assert!((mid.scale.x - 1.5).abs() < 0.001);
    }

    #[test]
    fn test_animation_mixer() {
        let mut mixer = AnimationMixer::new();
        let idx = mixer.add_animation(0, 1.0);
        assert_eq!(mixer.instances.len(), 1);

        mixer.set_weight(idx, 0.5);
        assert!((mixer.instances[idx].weight - 0.5).abs() < 0.001);

        mixer.remove_animation(idx);
        assert!(mixer.instances.is_empty());
    }
}
