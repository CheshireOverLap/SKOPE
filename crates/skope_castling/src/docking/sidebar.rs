//! Sidebar (AutoHide) 시스템
//!
//! 탭을 사이드바로 이동하면 아이콘 버튼으로 축소.
//! 클릭하면 서랍처럼 펼침, 외부 클릭 시 닫힘.

use serde::{Serialize, Deserialize};
use super::TabId;

/// 사이드바 위치
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SidebarSide {
    Left,
    Right,
}

/// 사이드바에 들어간 탭 항목
#[derive(Debug, Clone)]
pub struct SidebarTabEntry {
    /// 원래 탭 ID
    pub tab_id: TabId,
    /// 탭 타입명 (스포너 복원용)
    pub tab_type_name: String,
    /// 표시 이름
    pub display_name: String,
    /// 아이콘
    pub icon: Option<String>,
}

/// 사이드바 패널
pub struct SidebarPanel {
    /// 좌/우 위치
    pub location: SidebarSide,
    /// 사이드바에 있는 탭들
    pub tabs: Vec<SidebarTabEntry>,
    /// 현재 펼쳐진 탭 인덱스 (None이면 모두 닫힘)
    pub expanded: Option<usize>,
    /// 사이드바 버튼 영역 폭 (기본 32px)
    pub width: f32,
    /// 서랍 펼침 폭 (기본 280px)
    pub drawer_width: f32,
    /// 호버 중인 버튼 인덱스
    pub hovered_index: Option<usize>,
    /// 애니메이션 진행도 (0.0 = 닫힘, 1.0 = 열림)
    pub animation_progress: f32,
    /// 애니메이션 목표 (열림=true, 닫힘=false)
    pub animation_target_open: bool,
}

impl SidebarPanel {
    pub fn new(location: SidebarSide) -> Self {
        Self {
            location,
            tabs: Vec::new(),
            expanded: None,
            width: 32.0,
            drawer_width: 280.0,
            hovered_index: None,
            animation_progress: 0.0,
            animation_target_open: false,
        }
    }

    /// 탭 추가
    pub fn add_tab(&mut self, entry: SidebarTabEntry) {
        self.tabs.push(entry);
    }

    /// 탭 제거 (TabId로)
    pub fn remove_tab(&mut self, tab_id: TabId) -> Option<SidebarTabEntry> {
        if let Some(idx) = self.tabs.iter().position(|t| t.tab_id == tab_id) {
            // expanded 인덱스 조정
            if let Some(exp) = self.expanded {
                if exp == idx {
                    self.expanded = None;
                } else if exp > idx {
                    self.expanded = Some(exp - 1);
                }
            }
            Some(self.tabs.remove(idx))
        } else {
            None
        }
    }

    /// 서랍 토글 (열려있으면 닫고, 닫혀있으면 열기)
    pub fn toggle(&mut self, index: usize) {
        if self.expanded == Some(index) {
            self.expanded = None;
            self.animation_target_open = false;
        } else if index < self.tabs.len() {
            self.expanded = Some(index);
            self.animation_target_open = true;
        }
    }

    /// 서랍 닫기
    pub fn close(&mut self) {
        self.expanded = None;
        self.animation_target_open = false;
    }

    /// 애니메이션 틱 (매 프레임 호출, dt_seconds)
    pub fn tick_animation(&mut self, dt: f32) {
        let speed = 8.0; // 빠른 슬라이드
        let target = if self.animation_target_open { 1.0 } else { 0.0 };
        if (self.animation_progress - target).abs() > 0.001 {
            self.animation_progress += (target - self.animation_progress) * (speed * dt).min(1.0);
        } else {
            self.animation_progress = target;
        }
    }

    /// 현재 애니메이션 적용된 서랍 폭
    pub fn animated_drawer_width(&self) -> f32 {
        self.drawer_width * self.animation_progress
    }

    /// 탭이 있는지
    pub fn has_tabs(&self) -> bool {
        !self.tabs.is_empty()
    }

    /// 탭 수
    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    /// 비었는지
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    /// 실제 차지하는 폭 (탭이 없으면 0)
    pub fn total_width(&self) -> f32 {
        if self.tabs.is_empty() { 0.0 } else { self.width }
    }

    /// 서랍이 열려있는지
    pub fn is_expanded(&self) -> bool {
        self.expanded.is_some()
    }

    /// 현재 펼쳐진 탭 ID
    pub fn expanded_tab_id(&self) -> Option<TabId> {
        self.expanded.and_then(|idx| self.tabs.get(idx).map(|t| t.tab_id))
    }

    /// TabId로 탭 검색
    pub fn find_by_tab_id(&self, tab_id: TabId) -> Option<usize> {
        self.tabs.iter().position(|t| t.tab_id == tab_id)
    }
}
