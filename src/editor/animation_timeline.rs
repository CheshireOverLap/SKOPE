//! Animation Timeline Editor Panel
//!
//! 3D 스켈레탈 애니메이션 편집을 위한 타임라인 UI

use egui::{Color32, Rect, Ui, Vec2, Sense, Stroke};
use std::collections::HashSet;
use crate::gltf_loader::Animation;

/// 타임라인 상태
#[derive(Debug, Clone)]
pub struct AnimationTimelineState {
    /// 현재 재생 시간 (초)
    pub current_time: f32,
    /// 애니메이션 전체 길이 (초)
    pub duration: f32,
    /// 재생 중인지
    pub playing: bool,
    /// 선택된 트랙들
    pub selected_tracks: HashSet<String>,
    /// 줌 레벨 (pixels per second)
    pub zoom: f32,
    /// 가로 스크롤 오프셋
    pub scroll_x: f32,
    /// 루프 재생
    pub looping: bool,
    /// 재생 속도
    pub speed: f32,
    /// 확장된 노드들
    pub expanded_nodes: HashSet<String>,
    /// 선택된 키프레임 (track_name, keyframe_index)
    pub selected_keyframes: HashSet<(String, usize)>,
    /// 스냅 활성화
    pub snap_enabled: bool,
    /// 스냅 간격 (초)
    pub snap_interval: f32,
}

impl Default for AnimationTimelineState {
    fn default() -> Self {
        Self {
            current_time: 0.0,
            duration: 1.0,
            playing: false,
            selected_tracks: HashSet::new(),
            zoom: 100.0, // 100 pixels per second
            scroll_x: 0.0,
            looping: true,
            speed: 1.0,
            expanded_nodes: HashSet::new(),
            selected_keyframes: HashSet::new(),
            snap_enabled: true,
            snap_interval: 0.1, // 100ms
        }
    }
}

impl AnimationTimelineState {
    /// 새 타임라인 상태 생성
    pub fn new() -> Self {
        Self::default()
    }

    /// 애니메이션 설정
    pub fn set_animation(&mut self, duration: f32) {
        self.duration = duration;
        self.current_time = 0.0;
        self.scroll_x = 0.0;
    }

    /// 시간 업데이트 (재생 중일 때)
    pub fn update(&mut self, delta_seconds: f32) {
        if !self.playing {
            return;
        }

        self.current_time += delta_seconds * self.speed;

        if self.looping {
            while self.current_time >= self.duration {
                self.current_time -= self.duration;
            }
            while self.current_time < 0.0 {
                self.current_time += self.duration;
            }
        } else {
            if self.current_time >= self.duration {
                self.current_time = self.duration;
                self.playing = false;
            }
        }
    }

    /// 시간을 스냅
    fn snap_time(&self, time: f32) -> f32 {
        if self.snap_enabled && self.snap_interval > 0.0 {
            (time / self.snap_interval).round() * self.snap_interval
        } else {
            time
        }
    }

    /// 시간 → 픽셀 변환
    fn time_to_x(&self, time: f32) -> f32 {
        time * self.zoom - self.scroll_x
    }

    /// 픽셀 → 시간 변환
    fn x_to_time(&self, x: f32) -> f32 {
        (x + self.scroll_x) / self.zoom
    }

    /// 타임라인 UI 렌더링
    pub fn ui(&mut self, ui: &mut Ui, animation: Option<&Animation>) -> TimelineAction {
        let mut action = TimelineAction::None;

        // 툴바
        ui.horizontal(|ui| {
            // 재생 컨트롤
            if ui.button(if self.playing { "⏸" } else { "▶" }).clicked() {
                self.playing = !self.playing;
                if self.playing {
                    action = TimelineAction::Play;
                } else {
                    action = TimelineAction::Pause;
                }
            }

            if ui.button("⏹").clicked() {
                self.playing = false;
                self.current_time = 0.0;
                action = TimelineAction::Stop;
            }

            ui.separator();

            // 시간 표시
            let minutes = (self.current_time / 60.0) as u32;
            let seconds = self.current_time % 60.0;
            ui.label(format!("{:02}:{:05.2} / {:02}:{:05.2}",
                minutes, seconds,
                (self.duration / 60.0) as u32, self.duration % 60.0
            ));

            ui.separator();

            // 루프 토글
            if ui.selectable_label(self.looping, "Loop").clicked() {
                self.looping = !self.looping;
            }

            // 속도
            ui.label("Speed:");
            ui.add(egui::Slider::new(&mut self.speed, 0.1..=2.0).text("x").step_by(0.1));

            ui.separator();

            // 스냅 토글
            if ui.selectable_label(self.snap_enabled, "Snap").clicked() {
                self.snap_enabled = !self.snap_enabled;
            }

            // 줌
            ui.label("Zoom:");
            ui.add(egui::Slider::new(&mut self.zoom, 50.0..=500.0).logarithmic(true));
        });

        ui.separator();

        // 메인 타임라인 영역
        let available = ui.available_size();
        let (response, painter) = ui.allocate_painter(
            Vec2::new(available.x, available.y.min(300.0)),
            Sense::click_and_drag(),
        );

        let rect = response.rect;
        let track_panel_width = 150.0;
        let timeline_rect = Rect::from_min_size(
            rect.min + Vec2::new(track_panel_width, 0.0),
            Vec2::new(rect.width() - track_panel_width, rect.height()),
        );
        let track_rect = Rect::from_min_size(
            rect.min,
            Vec2::new(track_panel_width, rect.height()),
        );

        // 배경
        painter.rect_filled(rect, 0.0, Color32::from_rgb(30, 30, 30));
        painter.rect_filled(track_rect, 0.0, Color32::from_rgb(40, 40, 40));

        // 트랙 패널 구분선
        painter.line_segment(
            [track_rect.right_top(), track_rect.right_bottom()],
            Stroke::new(1.0, Color32::from_rgb(60, 60, 60)),
        );

        // 타임 눈금 그리기
        self.draw_time_ruler(&painter, timeline_rect);

        // 트랙 그리기
        if let Some(anim) = animation {
            self.draw_tracks(ui, &painter, track_rect, timeline_rect, anim, &mut action);
        } else {
            // 애니메이션 없음 메시지
            let center = rect.center();
            painter.text(
                center,
                egui::Align2::CENTER_CENTER,
                "No animation loaded",
                egui::FontId::default(),
                Color32::GRAY,
            );
        }

        // 재생 헤드 그리기
        let playhead_x = timeline_rect.left() + self.time_to_x(self.current_time);
        if playhead_x >= timeline_rect.left() && playhead_x <= timeline_rect.right() {
            painter.line_segment(
                [
                    egui::pos2(playhead_x, timeline_rect.top()),
                    egui::pos2(playhead_x, timeline_rect.bottom()),
                ],
                Stroke::new(2.0, Color32::from_rgb(255, 100, 100)),
            );

            // 재생 헤드 핸들
            let handle_size = 8.0;
            let handle_rect = Rect::from_center_size(
                egui::pos2(playhead_x, timeline_rect.top() + handle_size / 2.0),
                Vec2::splat(handle_size),
            );
            painter.rect_filled(handle_rect, 2.0, Color32::from_rgb(255, 100, 100));
        }

        // 클릭/드래그 처리
        if response.clicked() || response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                if timeline_rect.contains(pos) {
                    let local_x = pos.x - timeline_rect.left();
                    let new_time = self.snap_time(self.x_to_time(local_x).clamp(0.0, self.duration));
                    self.current_time = new_time;
                    action = TimelineAction::Seek(new_time);
                }
            }
        }

        // 마우스 휠로 줌/스크롤
        let scroll_delta = ui.input(|i| i.raw_scroll_delta);
        if response.hovered() {
            if ui.input(|i| i.modifiers.ctrl) {
                // Ctrl + 휠: 줌
                self.zoom = (self.zoom + scroll_delta.y * 0.5).clamp(50.0, 500.0);
            } else {
                // 휠: 스크롤
                self.scroll_x = (self.scroll_x - scroll_delta.x - scroll_delta.y).max(0.0);
            }
        }

        action
    }

    /// 타임 눈금 그리기
    fn draw_time_ruler(&self, painter: &egui::Painter, rect: Rect) {
        let ruler_height = 20.0;
        let ruler_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), ruler_height));

        // 배경
        painter.rect_filled(ruler_rect, 0.0, Color32::from_rgb(50, 50, 50));

        // 눈금 간격 계산 (줌 레벨에 따라)
        let major_interval = if self.zoom >= 200.0 {
            0.1 // 100ms
        } else if self.zoom >= 100.0 {
            0.5 // 500ms
        } else {
            1.0 // 1s
        };

        let minor_interval = major_interval / 5.0;

        // 눈금 그리기
        let start_time = (self.scroll_x / self.zoom / minor_interval).floor() * minor_interval;
        let end_time = ((self.scroll_x + rect.width()) / self.zoom / minor_interval).ceil() * minor_interval;

        let mut time = start_time;
        while time <= end_time {
            let x = rect.left() + self.time_to_x(time);

            if x >= rect.left() && x <= rect.right() {
                let is_major = (time / major_interval).round() * major_interval == time;

                if is_major {
                    // 큰 눈금
                    painter.line_segment(
                        [egui::pos2(x, ruler_rect.bottom() - 15.0), egui::pos2(x, ruler_rect.bottom())],
                        Stroke::new(1.0, Color32::from_rgb(150, 150, 150)),
                    );

                    // 시간 레이블
                    let label = if time >= 60.0 {
                        format!("{}:{:04.1}", (time / 60.0) as u32, time % 60.0)
                    } else {
                        format!("{:.1}s", time)
                    };
                    painter.text(
                        egui::pos2(x + 2.0, ruler_rect.top() + 2.0),
                        egui::Align2::LEFT_TOP,
                        label,
                        egui::FontId::proportional(10.0),
                        Color32::from_rgb(150, 150, 150),
                    );
                } else {
                    // 작은 눈금
                    painter.line_segment(
                        [egui::pos2(x, ruler_rect.bottom() - 5.0), egui::pos2(x, ruler_rect.bottom())],
                        Stroke::new(1.0, Color32::from_rgb(80, 80, 80)),
                    );
                }
            }

            time += minor_interval;
        }
    }

    /// 트랙 그리기
    fn draw_tracks(
        &mut self,
        ui: &mut Ui,
        painter: &egui::Painter,
        track_rect: Rect,
        timeline_rect: Rect,
        animation: &Animation,
        _action: &mut TimelineAction,
    ) {
        let ruler_height = 20.0;
        let track_height = 24.0;
        let mut y = timeline_rect.top() + ruler_height;

        // 채널별 그룹핑 (노드별)
        let mut node_channels: std::collections::HashMap<usize, Vec<&crate::gltf_loader::AnimationChannel>> =
            std::collections::HashMap::new();

        for channel in &animation.channels {
            node_channels.entry(channel.node_index).or_default().push(channel);
        }

        // 정렬된 노드 목록
        let mut node_indices: Vec<_> = node_channels.keys().copied().collect();
        node_indices.sort();

        for node_idx in node_indices {
            let channels = &node_channels[&node_idx];
            let node_name = format!("Node {}", node_idx);
            let is_expanded = self.expanded_nodes.contains(&node_name);

            // 노드 헤더
            let header_rect = Rect::from_min_size(
                egui::pos2(track_rect.left(), y),
                Vec2::new(track_rect.width(), track_height),
            );

            // 클릭으로 확장/축소
            let header_response = ui.allocate_rect(header_rect, Sense::click());
            if header_response.clicked() {
                if is_expanded {
                    self.expanded_nodes.remove(&node_name);
                } else {
                    self.expanded_nodes.insert(node_name.clone());
                }
            }

            // 노드 헤더 배경
            let header_color = if header_response.hovered() {
                Color32::from_rgb(55, 55, 55)
            } else {
                Color32::from_rgb(45, 45, 45)
            };
            painter.rect_filled(header_rect, 0.0, header_color);

            // 확장 아이콘
            let expand_icon = if is_expanded { "▼" } else { "▶" };
            painter.text(
                egui::pos2(header_rect.left() + 5.0, header_rect.center().y),
                egui::Align2::LEFT_CENTER,
                expand_icon,
                egui::FontId::proportional(12.0),
                Color32::WHITE,
            );

            // 노드 이름
            painter.text(
                egui::pos2(header_rect.left() + 20.0, header_rect.center().y),
                egui::Align2::LEFT_CENTER,
                &node_name,
                egui::FontId::proportional(12.0),
                Color32::WHITE,
            );

            y += track_height;

            // 확장된 경우 채널들 표시
            if is_expanded {
                for channel in channels {
                    let property_name = match channel.property {
                        crate::gltf_loader::AnimationProperty::Translation => "Position",
                        crate::gltf_loader::AnimationProperty::Rotation => "Rotation",
                        crate::gltf_loader::AnimationProperty::Scale => "Scale",
                        crate::gltf_loader::AnimationProperty::MorphTargetWeights => "Morph",
                    };
                    let track_name = format!("{}.{}", node_name, property_name);
                    let is_selected = self.selected_tracks.contains(&track_name);

                    // 트랙 패널
                    let track_panel_rect = Rect::from_min_size(
                        egui::pos2(track_rect.left(), y),
                        Vec2::new(track_rect.width(), track_height),
                    );

                    // 트랙 타임라인 영역
                    let track_timeline_rect = Rect::from_min_size(
                        egui::pos2(timeline_rect.left(), y),
                        Vec2::new(timeline_rect.width(), track_height),
                    );

                    // 트랙 배경
                    let track_bg = if is_selected {
                        Color32::from_rgb(60, 80, 100)
                    } else if y as i32 / track_height as i32 % 2 == 0 {
                        Color32::from_rgb(35, 35, 35)
                    } else {
                        Color32::from_rgb(30, 30, 30)
                    };
                    painter.rect_filled(track_timeline_rect, 0.0, track_bg);
                    painter.rect_filled(track_panel_rect, 0.0, track_bg);

                    // 트랙 이름 (들여쓰기)
                    painter.text(
                        egui::pos2(track_panel_rect.left() + 30.0, track_panel_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        property_name,
                        egui::FontId::proportional(11.0),
                        Color32::from_rgb(180, 180, 180),
                    );

                    // 키프레임 그리기
                    for (kf_idx, keyframe) in channel.keyframes.iter().enumerate() {
                        let kf_x = timeline_rect.left() + self.time_to_x(keyframe.time);

                        if kf_x >= timeline_rect.left() && kf_x <= timeline_rect.right() {
                            let kf_id = (track_name.clone(), kf_idx);
                            let is_kf_selected = self.selected_keyframes.contains(&kf_id);

                            let kf_color = if is_kf_selected {
                                Color32::from_rgb(255, 200, 100)
                            } else {
                                Color32::from_rgb(100, 200, 255)
                            };

                            // 키프레임 다이아몬드
                            let kf_size = 6.0;
                            let kf_center = egui::pos2(kf_x, track_timeline_rect.center().y);

                            let diamond = [
                                egui::pos2(kf_center.x, kf_center.y - kf_size),
                                egui::pos2(kf_center.x + kf_size, kf_center.y),
                                egui::pos2(kf_center.x, kf_center.y + kf_size),
                                egui::pos2(kf_center.x - kf_size, kf_center.y),
                            ];

                            painter.add(egui::Shape::convex_polygon(
                                diamond.to_vec(),
                                kf_color,
                                Stroke::new(1.0, Color32::BLACK),
                            ));
                        }
                    }

                    // 트랙 클릭으로 선택
                    let track_response = ui.allocate_rect(track_panel_rect, Sense::click());
                    if track_response.clicked() {
                        if ui.input(|i| i.modifiers.shift) {
                            if is_selected {
                                self.selected_tracks.remove(&track_name);
                            } else {
                                self.selected_tracks.insert(track_name.clone());
                            }
                        } else {
                            self.selected_tracks.clear();
                            self.selected_tracks.insert(track_name);
                        }
                    }

                    y += track_height;

                    if y > timeline_rect.bottom() {
                        break;
                    }
                }
            }

            if y > timeline_rect.bottom() {
                break;
            }
        }
    }
}

/// 타임라인 액션
#[derive(Debug, Clone, PartialEq)]
pub enum TimelineAction {
    None,
    Play,
    Pause,
    Stop,
    Seek(f32),
    SelectKeyframe(String, usize),
    DeleteKeyframe(String, usize),
    MoveKeyframe(String, usize, f32),
    AddKeyframe(String, f32),
}
