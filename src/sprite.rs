//! 스프라이트 애니메이션 시스템
//!
//! 2D 스프라이트 시트 기반 프레임 애니메이션 지원.
//! 게임 엔티티용 스프라이트 렌더링 및 애니메이션 처리.
//!
//! 현재 3D 엔진에서 미사용 - 향후 2D 기능 추가 시 활용 예정

#![allow(dead_code)]

use std::collections::HashMap;

/// 스프라이트 시트 정의
///
/// 단일 텍스처에서 여러 프레임을 그리드 형태로 저장한 스프라이트 시트.
/// 각 프레임은 동일한 크기의 타일로 구성됨.
#[derive(Debug, Clone)]
pub struct SpriteSheet {
    /// 텍스처 경로
    pub texture_path: String,
    /// 타일 크기 (픽셀): (width, height)
    pub tile_size: (u32, u32),
    /// 열 수
    pub columns: u32,
    /// 행 수
    pub rows: u32,
    /// 정의된 애니메이션 클립들
    pub animations: HashMap<String, SpriteClip>,
}

impl SpriteSheet {
    /// 새 스프라이트 시트 생성
    pub fn new(texture_path: impl Into<String>, tile_size: (u32, u32), columns: u32, rows: u32) -> Self {
        Self {
            texture_path: texture_path.into(),
            tile_size,
            columns,
            rows,
            animations: HashMap::new(),
        }
    }

    /// 애니메이션 클립 추가
    pub fn add_animation(&mut self, clip: SpriteClip) {
        self.animations.insert(clip.name.clone(), clip);
    }

    /// 총 프레임 수
    pub fn total_frames(&self) -> u32 {
        self.columns * self.rows
    }

    /// 프레임 인덱스에서 UV 좌표 계산
    ///
    /// Returns: (u_min, v_min, u_max, v_max)
    pub fn get_uv(&self, frame_index: u32) -> (f32, f32, f32, f32) {
        let col = frame_index % self.columns;
        let row = frame_index / self.columns;

        let u_size = 1.0 / self.columns as f32;
        let v_size = 1.0 / self.rows as f32;

        let u_min = col as f32 * u_size;
        let v_min = row as f32 * v_size;
        let u_max = u_min + u_size;
        let v_max = v_min + v_size;

        (u_min, v_min, u_max, v_max)
    }

    /// 프레임 인덱스에서 UV 좌표 계산 (flip 지원)
    pub fn get_uv_flipped(&self, frame_index: u32, flip_x: bool, flip_y: bool) -> (f32, f32, f32, f32) {
        let (mut u_min, mut v_min, mut u_max, mut v_max) = self.get_uv(frame_index);

        if flip_x {
            std::mem::swap(&mut u_min, &mut u_max);
        }
        if flip_y {
            std::mem::swap(&mut v_min, &mut v_max);
        }

        (u_min, v_min, u_max, v_max)
    }
}

/// 스프라이트 애니메이션 클립
///
/// 여러 프레임으로 구성된 애니메이션 시퀀스.
#[derive(Debug, Clone)]
pub struct SpriteClip {
    /// 클립 이름 (예: "idle", "walk", "attack")
    pub name: String,
    /// 프레임 정보들
    pub frames: Vec<SpriteFrame>,
    /// 루프 여부
    pub looping: bool,
    /// 초당 프레임 수 (FPS)
    pub fps: f32,
}

impl SpriteClip {
    /// 새 애니메이션 클립 생성
    pub fn new(name: impl Into<String>, fps: f32) -> Self {
        Self {
            name: name.into(),
            frames: Vec::new(),
            looping: true,
            fps,
        }
    }

    /// 프레임 추가
    pub fn add_frame(&mut self, index: u32) -> &mut Self {
        self.frames.push(SpriteFrame {
            index,
            duration: None,
        });
        self
    }

    /// 지속 시간이 지정된 프레임 추가
    pub fn add_frame_with_duration(&mut self, index: u32, duration: f32) -> &mut Self {
        self.frames.push(SpriteFrame {
            index,
            duration: Some(duration),
        });
        self
    }

    /// 연속 프레임 범위 추가
    pub fn add_frames_range(&mut self, start: u32, end: u32) -> &mut Self {
        for i in start..=end {
            self.frames.push(SpriteFrame {
                index: i,
                duration: None,
            });
        }
        self
    }

    /// 루프 설정
    pub fn set_looping(&mut self, looping: bool) -> &mut Self {
        self.looping = looping;
        self
    }

    /// 클립 총 지속 시간 계산
    pub fn total_duration(&self) -> f32 {
        let default_frame_duration = 1.0 / self.fps;
        self.frames.iter()
            .map(|f| f.duration.unwrap_or(default_frame_duration))
            .sum()
    }
}

/// 단일 프레임
#[derive(Debug, Clone)]
pub struct SpriteFrame {
    /// 프레임 인덱스 (0부터 시작, 좌상단→우하단)
    pub index: u32,
    /// 프레임 지속 시간 (초, None이면 1/fps 사용)
    pub duration: Option<f32>,
}

impl SpriteFrame {
    /// 새 프레임 생성
    pub fn new(index: u32) -> Self {
        Self {
            index,
            duration: None,
        }
    }

    /// 지속 시간이 지정된 프레임 생성
    pub fn with_duration(index: u32, duration: f32) -> Self {
        Self {
            index,
            duration: Some(duration),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sprite_sheet_uv() {
        let sheet = SpriteSheet::new("test.png", (32, 32), 4, 2);

        // 첫 번째 프레임 (좌상단)
        let (u_min, v_min, u_max, v_max) = sheet.get_uv(0);
        assert_eq!(u_min, 0.0);
        assert_eq!(v_min, 0.0);
        assert_eq!(u_max, 0.25);
        assert_eq!(v_max, 0.5);

        // 두 번째 프레임
        let (u_min, v_min, u_max, v_max) = sheet.get_uv(1);
        assert_eq!(u_min, 0.25);
        assert_eq!(v_min, 0.0);
        assert_eq!(u_max, 0.5);
        assert_eq!(v_max, 0.5);

        // 다섯 번째 프레임 (두 번째 행 첫 번째)
        let (u_min, v_min, u_max, v_max) = sheet.get_uv(4);
        assert_eq!(u_min, 0.0);
        assert_eq!(v_min, 0.5);
        assert_eq!(u_max, 0.25);
        assert_eq!(v_max, 1.0);
    }

    #[test]
    fn test_sprite_clip_builder() {
        let mut clip = SpriteClip::new("walk", 8.0);
        clip.add_frames_range(0, 3)
            .set_looping(true);

        assert_eq!(clip.frames.len(), 4);
        assert!(clip.looping);
        assert_eq!(clip.fps, 8.0);
    }
}
