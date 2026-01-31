//! Magic Circle Node System
//!
//! 원소 노드 및 극좌표 위치 시스템

use serde::{Deserialize, Serialize};

/// 원소 타입 (노드의 본질)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ElementType {
    Fire,      // 빨강, 데미지 증가
    Water,     // 파랑, 힐/슬로우
    Lightning, // 노랑, 연쇄/스턴
    Wind,      // 초록, 이동속도/넉백
    Earth,     // 갈색, 방어/지속시간
    Void,      // 보라, 특수효과
}

impl ElementType {
    /// 원소의 기본 색상 (RGB)
    pub fn color(&self) -> [f32; 3] {
        match self {
            ElementType::Fire => [1.0, 0.3, 0.1],
            ElementType::Water => [0.2, 0.5, 1.0],
            ElementType::Lightning => [1.0, 0.9, 0.2],
            ElementType::Wind => [0.3, 0.9, 0.4],
            ElementType::Earth => [0.6, 0.4, 0.2],
            ElementType::Void => [0.6, 0.2, 0.9],
        }
    }

    /// 원소의 shader index (0-5)
    pub fn shader_index(&self) -> u32 {
        match self {
            ElementType::Fire => 0,
            ElementType::Water => 1,
            ElementType::Lightning => 2,
            ElementType::Wind => 3,
            ElementType::Earth => 4,
            ElementType::Void => 5,
        }
    }
}

/// 극좌표 위치
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PolarPosition {
    /// 반지름 (0.0 = 중심, 1.0 = 가장자리)
    pub radius: f32,
    /// 각도 (라디안, 0 = 오른쪽, 반시계방향 양수)
    pub angle: f32,
}

impl PolarPosition {
    pub fn new(radius: f32, angle: f32) -> Self {
        Self { radius, angle }
    }

    /// 극좌표를 직교좌표로 변환
    pub fn to_cartesian(&self) -> (f32, f32) {
        (self.radius * self.angle.cos(), self.radius * self.angle.sin())
    }

    /// 직교좌표에서 극좌표로 변환
    pub fn from_cartesian(x: f32, y: f32) -> Self {
        Self {
            radius: (x * x + y * y).sqrt(),
            angle: y.atan2(x),
        }
    }

    /// 각도를 정규화 (0 ~ 2π)
    pub fn normalize_angle(&mut self) {
        use std::f32::consts::TAU;
        self.angle = self.angle.rem_euclid(TAU);
    }
}

impl Default for PolarPosition {
    fn default() -> Self {
        Self {
            radius: 0.0,
            angle: 0.0,
        }
    }
}

/// 노드 정의
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeDef {
    /// 원소 타입
    pub element: ElementType,
    /// 극좌표 위치
    pub position: PolarPosition,
    /// 크기 배율 (기본 1.0)
    #[serde(default = "default_size")]
    pub size: f32,
}

fn default_size() -> f32 {
    1.0
}

impl NodeDef {
    pub fn new(element: ElementType, radius: f32, angle: f32) -> Self {
        Self {
            element,
            position: PolarPosition::new(radius, angle),
            size: 1.0,
        }
    }

    /// 직교좌표 반환
    pub fn cartesian(&self) -> (f32, f32) {
        self.position.to_cartesian()
    }
}

/// 노드 간 연결
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    /// 시작 노드 인덱스
    pub from: usize,
    /// 끝 노드 인덱스
    pub to: usize,
    /// 에너지 흐름 속도 (기본 1.0)
    #[serde(default = "default_flow_speed")]
    pub flow_speed: f32,
}

fn default_flow_speed() -> f32 {
    1.0
}

impl Connection {
    pub fn new(from: usize, to: usize) -> Self {
        Self {
            from,
            to,
            flow_speed: 1.0,
        }
    }

    pub fn with_speed(from: usize, to: usize, flow_speed: f32) -> Self {
        Self {
            from,
            to,
            flow_speed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_polar_to_cartesian() {
        let pos = PolarPosition::new(1.0, 0.0);
        let (x, y) = pos.to_cartesian();
        assert!((x - 1.0).abs() < 0.001);
        assert!(y.abs() < 0.001);

        let pos = PolarPosition::new(1.0, PI / 2.0);
        let (x, y) = pos.to_cartesian();
        assert!(x.abs() < 0.001);
        assert!((y - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_element_colors() {
        assert_eq!(ElementType::Fire.shader_index(), 0);
        assert_eq!(ElementType::Void.shader_index(), 5);
    }
}
