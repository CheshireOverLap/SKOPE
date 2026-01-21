//! Tear-off State Machine - 탭 분리 상태 머신
//!
//! 탭을 OS 네이티브 윈도우로 분리하는 상태 머신입니다.
//! 드래그 시작 → 내부 도킹 → 외부 분리 → 윈도우 생성 순으로 진행됩니다.

use egui::{Pos2, Rect, Vec2};

use super::types::Tab;

/// Tear-off 상태
#[derive(Debug, Clone)]
pub enum TearOffState {
    /// 초기 상태 (드래그 없음)
    Idle,

    /// 드래그 시작됨 (임계값 미만)
    Pending {
        tab: Tab,
        start_pos: Pos2,
    },

    /// 내부 도킹 모드 (egui_dock 영역 내)
    Internal {
        tab: Tab,
        compass_active: bool,
        hovered_zone: Option<DockZone>,
    },

    /// 외부 영역 (메인 윈도우 밖)
    External {
        tab: Tab,
        screen_pos: Pos2,
    },

    /// OS 윈도우 생성 요청됨
    CreateNativeWindow {
        tab: Tab,
        position: (i32, i32),
        size: (u32, u32),
    },
}

impl Default for TearOffState {
    fn default() -> Self {
        TearOffState::Idle
    }
}

/// 도킹 방향 (Compass)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockZone {
    /// 중앙 (탭 병합)
    Center,
    /// 상단
    Top,
    /// 하단
    Bottom,
    /// 좌측
    Left,
    /// 우측
    Right,
}

impl DockZone {
    /// 방향에 따른 프리뷰 영역 비율 계산
    pub fn preview_fraction(&self) -> f32 {
        match self {
            DockZone::Center => 1.0,
            _ => 0.5,
        }
    }

    /// 방향에 따른 프리뷰 영역 계산
    pub fn calculate_preview_rect(&self, container: Rect) -> Rect {
        match self {
            DockZone::Center => container,
            DockZone::Top => {
                let height = container.height() * 0.5;
                Rect::from_min_size(container.min, Vec2::new(container.width(), height))
            }
            DockZone::Bottom => {
                let height = container.height() * 0.5;
                Rect::from_min_size(
                    Pos2::new(container.min.x, container.max.y - height),
                    Vec2::new(container.width(), height),
                )
            }
            DockZone::Left => {
                let width = container.width() * 0.5;
                Rect::from_min_size(container.min, Vec2::new(width, container.height()))
            }
            DockZone::Right => {
                let width = container.width() * 0.5;
                Rect::from_min_size(
                    Pos2::new(container.max.x - width, container.min.y),
                    Vec2::new(width, container.height()),
                )
            }
        }
    }
}

/// Tear-off 결과
#[derive(Debug, Clone)]
pub enum TearOffResult {
    /// 아무것도 안함 (드롭 안됨)
    None,
    /// 내부 도킹 (egui_dock 처리)
    InternalDock {
        tab: Tab,
        zone: DockZone,
    },
    /// OS 윈도우 생성
    CreateWindow {
        tab: Tab,
        position: (i32, i32),
        size: (u32, u32),
    },
    /// 취소됨
    Cancelled,
}

/// Tear-off 상태 머신
#[derive(Debug, Clone)]
pub struct TearOffStateMachine {
    /// 현재 상태
    pub state: TearOffState,
    /// 드래그 임계값 (픽셀)
    pub drag_threshold: f32,
    /// 외부 영역 임계값 (윈도우 경계 밖 픽셀)
    pub external_threshold: f32,
    /// 기본 새 윈도우 크기
    pub default_window_size: (u32, u32),
    /// 드래그 시작 위치 (Screen 좌표)
    start_screen_pos: Option<Pos2>,
}

impl Default for TearOffStateMachine {
    fn default() -> Self {
        Self {
            state: TearOffState::Idle,
            drag_threshold: 5.0,
            external_threshold: 20.0,
            default_window_size: (400, 300),
            start_screen_pos: None,
        }
    }
}

impl TearOffStateMachine {
    /// 새 상태 머신 생성
    pub fn new() -> Self {
        Self::default()
    }

    /// 드래그 시작
    pub fn on_drag_start(&mut self, tab: Tab, pos: Pos2) {
        self.state = TearOffState::Pending {
            tab,
            start_pos: pos,
        };
        self.start_screen_pos = Some(pos);

        log::debug!("[TearOff] Drag started for {:?} at {:?}", tab, pos);
    }

    /// 드래그 이동
    pub fn on_drag_move(&mut self, pos: Pos2, window_rect: Rect) {
        match &self.state {
            TearOffState::Pending { tab, start_pos } => {
                let distance = (pos - *start_pos).length();

                if distance > self.drag_threshold {
                    // 임계값 초과 → Internal 모드로 전환
                    let tab = *tab;
                    self.state = TearOffState::Internal {
                        tab,
                        compass_active: true,
                        hovered_zone: None,
                    };
                    log::debug!("[TearOff] State: Pending -> Internal");
                }
            }

            TearOffState::Internal { tab, .. } => {
                let tab = *tab;

                // 윈도우 밖으로 나갔는지 확인
                let expanded_rect = window_rect.expand(self.external_threshold);

                if !expanded_rect.contains(pos) {
                    // 외부로 이동
                    self.state = TearOffState::External {
                        tab,
                        screen_pos: pos,
                    };
                    log::debug!("[TearOff] State: Internal -> External");
                } else {
                    // 도킹 존 감지
                    let zone = self.detect_dock_zone(pos, window_rect);
                    self.state = TearOffState::Internal {
                        tab,
                        compass_active: true,
                        hovered_zone: zone,
                    };
                }
            }

            TearOffState::External { tab, .. } => {
                let tab = *tab;

                // 다시 내부로 돌아왔는지 확인
                if window_rect.contains(pos) {
                    self.state = TearOffState::Internal {
                        tab,
                        compass_active: true,
                        hovered_zone: None,
                    };
                    log::debug!("[TearOff] State: External -> Internal");
                } else {
                    // 외부 위치 업데이트
                    self.state = TearOffState::External {
                        tab,
                        screen_pos: pos,
                    };
                }
            }

            _ => {}
        }
    }

    /// 드래그 종료
    pub fn on_drag_end(&mut self) -> TearOffResult {
        let result = match &self.state {
            TearOffState::Idle | TearOffState::Pending { .. } => {
                TearOffResult::None
            }

            TearOffState::Internal { tab, hovered_zone, .. } => {
                if let Some(zone) = hovered_zone {
                    TearOffResult::InternalDock {
                        tab: *tab,
                        zone: *zone,
                    }
                } else {
                    TearOffResult::None
                }
            }

            TearOffState::External { tab, screen_pos } => {
                TearOffResult::CreateWindow {
                    tab: *tab,
                    position: (screen_pos.x as i32, screen_pos.y as i32),
                    size: self.default_window_size,
                }
            }

            TearOffState::CreateNativeWindow { tab, position, size } => {
                TearOffResult::CreateWindow {
                    tab: *tab,
                    position: *position,
                    size: *size,
                }
            }
        };

        log::debug!("[TearOff] Drag ended with result: {:?}", result);

        self.reset();
        result
    }

    /// 드래그 취소 (ESC)
    pub fn on_cancel(&mut self) -> TearOffResult {
        log::debug!("[TearOff] Drag cancelled");
        self.reset();
        TearOffResult::Cancelled
    }

    /// 상태 리셋
    pub fn reset(&mut self) {
        self.state = TearOffState::Idle;
        self.start_screen_pos = None;
    }

    /// 드래그 중인지 확인
    pub fn is_dragging(&self) -> bool {
        !matches!(self.state, TearOffState::Idle)
    }

    /// 활성 드래그인지 확인 (임계값 초과)
    pub fn is_active(&self) -> bool {
        matches!(
            self.state,
            TearOffState::Internal { .. } | TearOffState::External { .. }
        )
    }

    /// 외부 모드인지 확인
    pub fn is_external(&self) -> bool {
        matches!(self.state, TearOffState::External { .. })
    }

    /// 현재 드래그 중인 탭
    pub fn dragging_tab(&self) -> Option<Tab> {
        match &self.state {
            TearOffState::Pending { tab, .. } => Some(*tab),
            TearOffState::Internal { tab, .. } => Some(*tab),
            TearOffState::External { tab, .. } => Some(*tab),
            TearOffState::CreateNativeWindow { tab, .. } => Some(*tab),
            TearOffState::Idle => None,
        }
    }

    /// 현재 호버된 도킹 존
    pub fn hovered_zone(&self) -> Option<DockZone> {
        if let TearOffState::Internal { hovered_zone, .. } = &self.state {
            *hovered_zone
        } else {
            None
        }
    }

    /// 도킹 존 감지
    fn detect_dock_zone(&self, pos: Pos2, container: Rect) -> Option<DockZone> {
        // 중앙 영역 (컨테이너의 중앙 40%)
        let center_margin = 0.3;
        let center_rect = Rect::from_min_max(
            Pos2::new(
                container.min.x + container.width() * center_margin,
                container.min.y + container.height() * center_margin,
            ),
            Pos2::new(
                container.max.x - container.width() * center_margin,
                container.max.y - container.height() * center_margin,
            ),
        );

        if center_rect.contains(pos) {
            return Some(DockZone::Center);
        }

        // 가장자리 영역 (25% 임계값)
        let edge_threshold = 0.25;
        let relative_x = (pos.x - container.min.x) / container.width();
        let relative_y = (pos.y - container.min.y) / container.height();

        if relative_y < edge_threshold {
            Some(DockZone::Top)
        } else if relative_y > 1.0 - edge_threshold {
            Some(DockZone::Bottom)
        } else if relative_x < edge_threshold {
            Some(DockZone::Left)
        } else if relative_x > 1.0 - edge_threshold {
            Some(DockZone::Right)
        } else {
            None
        }
    }

    /// 프리뷰 영역 계산
    pub fn get_preview_rect(&self, container: Rect) -> Option<Rect> {
        self.hovered_zone().map(|zone| zone.calculate_preview_rect(container))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_machine_lifecycle() {
        let mut sm = TearOffStateMachine::new();

        // 초기 상태
        assert!(!sm.is_dragging());

        // 드래그 시작
        sm.on_drag_start(Tab::Inspector, Pos2::new(100.0, 100.0));
        assert!(sm.is_dragging());
        assert!(!sm.is_active());

        // 임계값 미만 이동
        let window_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        sm.on_drag_move(Pos2::new(102.0, 102.0), window_rect);
        assert!(matches!(sm.state, TearOffState::Pending { .. }));

        // 임계값 초과 이동
        sm.on_drag_move(Pos2::new(120.0, 120.0), window_rect);
        assert!(matches!(sm.state, TearOffState::Internal { .. }));
        assert!(sm.is_active());

        // 드래그 종료
        let result = sm.on_drag_end();
        assert!(!sm.is_dragging());
        assert!(matches!(result, TearOffResult::None | TearOffResult::InternalDock { .. }));
    }

    #[test]
    fn test_external_mode() {
        let mut sm = TearOffStateMachine::new();
        let window_rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));

        sm.on_drag_start(Tab::Console, Pos2::new(100.0, 100.0));
        sm.on_drag_move(Pos2::new(200.0, 200.0), window_rect); // Internal
        sm.on_drag_move(Pos2::new(900.0, 300.0), window_rect); // External

        assert!(sm.is_external());

        let result = sm.on_drag_end();
        assert!(matches!(result, TearOffResult::CreateWindow { .. }));
    }

    #[test]
    fn test_dock_zone_detection() {
        let sm = TearOffStateMachine::new();
        let container = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 400.0));

        // 상단
        assert_eq!(sm.detect_dock_zone(Pos2::new(200.0, 30.0), container), Some(DockZone::Top));
        // 하단
        assert_eq!(sm.detect_dock_zone(Pos2::new(200.0, 370.0), container), Some(DockZone::Bottom));
        // 좌측
        assert_eq!(sm.detect_dock_zone(Pos2::new(30.0, 200.0), container), Some(DockZone::Left));
        // 우측
        assert_eq!(sm.detect_dock_zone(Pos2::new(370.0, 200.0), container), Some(DockZone::Right));
        // 중앙
        assert_eq!(sm.detect_dock_zone(Pos2::new(200.0, 200.0), container), Some(DockZone::Center));
    }
}
