//! 스프라이트 애니메이션 ECS 시스템
//!
//! 스프라이트 프레임 애니메이션 업데이트 및 이벤트 처리.

use std::collections::HashMap;
use bevy_ecs::prelude::*;

use crate::ecs_components::{SpriteRenderer, SpriteAnimator};
use crate::sprite::{SpriteSheet, SpriteClip};

/// 스프라이트 시트 리소스
///
/// 로드된 모든 스프라이트 시트를 관리.
#[derive(Resource, Default)]
pub struct SpriteSheetAssets {
    /// 인덱스로 접근 가능한 스프라이트 시트들
    pub sheets: HashMap<usize, SpriteSheet>,
    /// 다음 할당할 인덱스
    next_index: usize,
}

impl SpriteSheetAssets {
    pub fn new() -> Self {
        Self::default()
    }

    /// 스프라이트 시트 추가
    pub fn add(&mut self, sheet: SpriteSheet) -> usize {
        let index = self.next_index;
        self.sheets.insert(index, sheet);
        self.next_index += 1;
        index
    }

    /// 스프라이트 시트 가져오기
    pub fn get(&self, index: usize) -> Option<&SpriteSheet> {
        self.sheets.get(&index)
    }

    /// 스프라이트 시트 가변 가져오기
    pub fn get_mut(&mut self, index: usize) -> Option<&mut SpriteSheet> {
        self.sheets.get_mut(&index)
    }

    /// 스프라이트 시트 제거
    pub fn remove(&mut self, index: usize) -> Option<SpriteSheet> {
        self.sheets.remove(&index)
    }

    /// 등록된 시트 수
    pub fn count(&self) -> usize {
        self.sheets.len()
    }
}

/// 스프라이트 애니메이션 완료 이벤트
#[derive(Debug, Clone)]
pub struct SpriteAnimationCompleteEvent {
    /// 완료된 엔티티
    pub entity: Entity,
    /// 완료된 클립 이름
    pub clip_name: String,
    /// 사용자 정의 이벤트 이름 (on_complete)
    pub event_name: Option<String>,
}

/// 스프라이트 애니메이션 완료 이벤트 리소스
#[derive(Resource, Default)]
pub struct SpriteAnimationEvents {
    pub completed: Vec<SpriteAnimationCompleteEvent>,
}

impl SpriteAnimationEvents {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.completed.clear();
    }

    pub fn push(&mut self, event: SpriteAnimationCompleteEvent) {
        self.completed.push(event);
    }

    pub fn drain(&mut self) -> Vec<SpriteAnimationCompleteEvent> {
        std::mem::take(&mut self.completed)
    }
}

/// 스프라이트 애니메이션 업데이트 시스템
///
/// 매 프레임 호출되어 스프라이트 애니메이션을 진행시킴.
pub fn sprite_animation_update_system(
    delta_time: f32,
    sprite_sheets: &SpriteSheetAssets,
    events: &mut SpriteAnimationEvents,
    query: &mut [(Entity, &mut SpriteRenderer, &mut SpriteAnimator)],
) {
    for (entity, renderer, animator) in query.iter_mut() {
        if !animator.playing {
            continue;
        }

        // 스프라이트 시트 가져오기
        let Some(sheet) = sprite_sheets.get(renderer.sprite_sheet_index) else {
            continue;
        };

        // 현재 클립 가져오기
        let Some(clip) = sheet.animations.get(&animator.current_clip) else {
            continue;
        };

        if clip.frames.is_empty() {
            continue;
        }

        // 시간 업데이트
        animator.elapsed += delta_time * animator.speed;

        // 현재 프레임의 지속 시간
        let frame = &clip.frames[animator.frame_index];
        let frame_duration = frame.duration.unwrap_or(1.0 / clip.fps);

        // 프레임 진행
        while animator.elapsed >= frame_duration && animator.playing {
            animator.elapsed -= frame_duration;
            animator.frame_index += 1;

            // 클립 끝 도달
            if animator.frame_index >= clip.frames.len() {
                if clip.looping {
                    animator.frame_index = 0;
                } else {
                    animator.frame_index = clip.frames.len() - 1;
                    animator.playing = false;

                    // 완료 이벤트 발생
                    events.push(SpriteAnimationCompleteEvent {
                        entity: *entity,
                        clip_name: animator.current_clip.clone(),
                        event_name: animator.on_complete.clone(),
                    });
                    break;
                }
            }
        }

        // 렌더러에 현재 프레임 설정
        if animator.frame_index < clip.frames.len() {
            renderer.current_frame = clip.frames[animator.frame_index].index;
        }
    }
}

/// Bevy ECS 스타일 시스템 (Query 사용)
pub fn sprite_animation_system(
    delta_time: f32,
    sprite_sheets: &SpriteSheetAssets,
    events: &mut SpriteAnimationEvents,
    mut query: Query<(Entity, &mut SpriteRenderer, &mut SpriteAnimator)>,
) {
    for (entity, mut renderer, mut animator) in query.iter_mut() {
        if !animator.playing {
            continue;
        }

        // 스프라이트 시트 가져오기
        let Some(sheet) = sprite_sheets.get(renderer.sprite_sheet_index) else {
            continue;
        };

        // 현재 클립 가져오기
        let Some(clip) = sheet.animations.get(&animator.current_clip) else {
            continue;
        };

        if clip.frames.is_empty() {
            continue;
        }

        // 시간 업데이트
        animator.elapsed += delta_time * animator.speed;

        // 현재 프레임의 지속 시간
        let frame = &clip.frames[animator.frame_index];
        let frame_duration = frame.duration.unwrap_or(1.0 / clip.fps);

        // 프레임 진행
        while animator.elapsed >= frame_duration && animator.playing {
            animator.elapsed -= frame_duration;
            animator.frame_index += 1;

            // 클립 끝 도달
            if animator.frame_index >= clip.frames.len() {
                if clip.looping {
                    animator.frame_index = 0;
                } else {
                    animator.frame_index = clip.frames.len() - 1;
                    animator.playing = false;

                    // 완료 이벤트 발생
                    events.push(SpriteAnimationCompleteEvent {
                        entity,
                        clip_name: animator.current_clip.clone(),
                        event_name: animator.on_complete.clone(),
                    });
                    break;
                }
            }
        }

        // 렌더러에 현재 프레임 설정
        if animator.frame_index < clip.frames.len() {
            renderer.current_frame = clip.frames[animator.frame_index].index;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_sheet() -> SpriteSheet {
        use crate::sprite::{SpriteClip, SpriteFrame};

        let mut sheet = SpriteSheet::new("test.png", (32, 32), 4, 2);

        let mut walk_clip = SpriteClip::new("walk", 8.0);
        walk_clip.add_frames_range(0, 3);

        let mut idle_clip = SpriteClip::new("idle", 4.0);
        idle_clip.add_frames_range(4, 5);

        sheet.add_animation(walk_clip);
        sheet.add_animation(idle_clip);

        sheet
    }

    #[test]
    fn test_sprite_sheet_assets() {
        let mut assets = SpriteSheetAssets::new();
        let sheet = create_test_sheet();

        let index = assets.add(sheet);
        assert_eq!(index, 0);
        assert_eq!(assets.count(), 1);

        let retrieved = assets.get(index).unwrap();
        assert_eq!(retrieved.columns, 4);
        assert_eq!(retrieved.rows, 2);
    }

    #[test]
    fn test_animation_frame_progression() {
        let sheet = create_test_sheet();
        let clip = sheet.animations.get("walk").unwrap();

        assert_eq!(clip.frames.len(), 4);
        assert_eq!(clip.fps, 8.0);

        // 프레임당 0.125초
        let frame_duration = 1.0 / clip.fps;
        assert!((frame_duration - 0.125).abs() < 0.001);
    }
}
