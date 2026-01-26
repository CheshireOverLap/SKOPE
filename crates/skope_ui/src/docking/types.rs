//! 도킹 시스템 핵심 타입

use glam::Vec2;
use serde::{Serialize, Deserialize};

/// 노드 고유 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

impl NodeId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// 탭 고유 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TabId(pub u64);

impl TabId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// 분할 방향
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitDirection {
    /// 가로 분할 (좌우로 나눔)
    Horizontal,
    /// 세로 분할 (상하로 나눔)
    Vertical,
}

impl SplitDirection {
    /// 반대 방향
    pub fn opposite(&self) -> Self {
        match self {
            Self::Horizontal => Self::Vertical,
            Self::Vertical => Self::Horizontal,
        }
    }

    /// 이 방향으로 분할할 때의 주 축
    pub fn main_axis(&self) -> Axis {
        match self {
            Self::Horizontal => Axis::X,
            Self::Vertical => Axis::Y,
        }
    }
}

/// 축
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
}

/// 도킹 위치
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DockPosition {
    /// 탭으로 병합 (가운데)
    Center,
    /// 왼쪽에 분할 도킹
    Left,
    /// 오른쪽에 분할 도킹
    Right,
    /// 위쪽에 분할 도킹
    Top,
    /// 아래쪽에 분할 도킹
    Bottom,
}

impl DockPosition {
    /// 분할 방향 반환 (Center는 None)
    pub fn split_direction(&self) -> Option<SplitDirection> {
        match self {
            Self::Center => None,
            Self::Left | Self::Right => Some(SplitDirection::Horizontal),
            Self::Top | Self::Bottom => Some(SplitDirection::Vertical),
        }
    }

    /// 분할 시 새 노드가 첫 번째 자식인지
    pub fn is_first_child(&self) -> bool {
        matches!(self, Self::Left | Self::Top)
    }
}

/// 노드 종류
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    /// 루트 영역 (OS 윈도우)
    Area,
    /// 분할자
    Splitter,
    /// 탭 스택
    TabStack,
}

/// 노드 레이아웃 정보
#[derive(Debug, Clone, Copy, Default)]
pub struct NodeRect {
    /// 위치 (부모 기준)
    pub position: Vec2,
    /// 크기
    pub size: Vec2,
}

impl NodeRect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            position: Vec2::new(x, y),
            size: Vec2::new(width, height),
        }
    }

    /// 절대 좌표로 변환
    pub fn to_absolute(&self, parent_pos: Vec2) -> Self {
        Self {
            position: parent_pos + self.position,
            size: self.size,
        }
    }

    /// 점이 이 영역 안에 있는지
    pub fn contains(&self, point: Vec2) -> bool {
        point.x >= self.position.x
            && point.x <= self.position.x + self.size.x
            && point.y >= self.position.y
            && point.y <= self.position.y + self.size.y
    }

    /// 중심점
    pub fn center(&self) -> Vec2 {
        self.position + self.size * 0.5
    }

    /// 왼쪽 절반
    pub fn left_half(&self) -> Self {
        Self {
            position: self.position,
            size: Vec2::new(self.size.x * 0.5, self.size.y),
        }
    }

    /// 오른쪽 절반
    pub fn right_half(&self) -> Self {
        Self {
            position: Vec2::new(self.position.x + self.size.x * 0.5, self.position.y),
            size: Vec2::new(self.size.x * 0.5, self.size.y),
        }
    }

    /// 위쪽 절반
    pub fn top_half(&self) -> Self {
        Self {
            position: self.position,
            size: Vec2::new(self.size.x, self.size.y * 0.5),
        }
    }

    /// 아래쪽 절반
    pub fn bottom_half(&self) -> Self {
        Self {
            position: Vec2::new(self.position.x, self.position.y + self.size.y * 0.5),
            size: Vec2::new(self.size.x, self.size.y * 0.5),
        }
    }
}

/// 탭 스택 스타일
#[derive(Debug, Clone)]
pub struct TabStackStyle {
    /// 탭 바 높이
    pub tab_bar_height: f32,
    /// 탭 최소 너비
    pub tab_min_width: f32,
    /// 탭 최대 너비
    pub tab_max_width: f32,
    /// 탭 간격
    pub tab_spacing: f32,
    /// 탭 패딩
    pub tab_padding: f32,
}

impl Default for TabStackStyle {
    fn default() -> Self {
        Self {
            tab_bar_height: 28.0,
            tab_min_width: 60.0,
            tab_max_width: 200.0,
            tab_spacing: 2.0,
            tab_padding: 8.0,
        }
    }
}

/// 스플리터 스타일
#[derive(Debug, Clone)]
pub struct SplitterStyle {
    /// 분할선 두께
    pub thickness: f32,
    /// 드래그 히트 영역 (두께보다 넓게)
    pub hit_area: f32,
}

impl Default for SplitterStyle {
    fn default() -> Self {
        Self {
            thickness: 4.0,
            hit_area: 8.0,
        }
    }
}
