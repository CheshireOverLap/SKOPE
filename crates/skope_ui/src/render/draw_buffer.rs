//! SlateDrawBuffer — 더블 버퍼링 드로우 버퍼
//!
//! 프레임 간 드로우 데이터를 더블 버퍼링하여 렌더 스레드가
//! 이전 프레임을 그리는 동안 새 프레임을 준비할 수 있습니다.

use crate::core::PaintGeometry;
use crate::core::Color;

/// 드로우 커맨드 타입
#[derive(Debug, Clone)]
pub enum DrawCommand {
    Box {
        layer: u32,
        geometry: PaintGeometry,
        color: Color,
    },
    Border {
        layer: u32,
        geometry: PaintGeometry,
        border_color: Color,
        border_width: f32,
    },
    Text {
        layer: u32,
        geometry: PaintGeometry,
        text: String,
        color: Color,
        font_size: f32,
    },
    Line {
        layer: u32,
        start: glam::Vec2,
        end: glam::Vec2,
        color: Color,
        thickness: f32,
    },
    Clear {
        color: Color,
    },
}

/// 단일 프레임의 드로우 데이터
#[derive(Debug, Clone)]
pub struct DrawFrame {
    commands: Vec<DrawCommand>,
    generation: u64,
    is_complete: bool,
}

impl DrawFrame {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            generation: 0,
            is_complete: false,
        }
    }

    pub fn clear(&mut self) {
        self.commands.clear();
        self.is_complete = false;
    }

    pub fn push(&mut self, cmd: DrawCommand) {
        self.commands.push(cmd);
    }

    pub fn finish(&mut self, generation: u64) {
        self.generation = generation;
        self.is_complete = true;
    }

    pub fn commands(&self) -> &[DrawCommand] { &self.commands }
    pub fn command_count(&self) -> usize { self.commands.len() }
    pub fn generation(&self) -> u64 { self.generation }
    pub fn is_complete(&self) -> bool { self.is_complete }
}

/// 더블 버퍼 드로우 시스템
pub struct SlateDrawBuffer {
    front: DrawFrame,
    back: DrawFrame,
    current_generation: u64,
    frames_drawn: u64,
    peak_commands: usize,
}

impl SlateDrawBuffer {
    pub fn new() -> Self {
        Self {
            front: DrawFrame::new(),
            back: DrawFrame::new(),
            current_generation: 0,
            frames_drawn: 0,
            peak_commands: 0,
        }
    }

    /// 백 버퍼에 새 프레임 시작
    pub fn begin_frame(&mut self) {
        self.current_generation += 1;
        self.back.clear();
    }

    /// 백 버퍼에 커맨드 추가
    pub fn push_command(&mut self, cmd: DrawCommand) {
        self.back.push(cmd);
    }

    /// 백 버퍼 완료 → 프론트/백 교환
    pub fn end_frame(&mut self) {
        self.back.finish(self.current_generation);
        self.peak_commands = self.peak_commands.max(self.back.command_count());
        std::mem::swap(&mut self.front, &mut self.back);
        self.frames_drawn += 1;
    }

    /// 렌더링할 현재 프레임 (프론트 버퍼)
    pub fn render_frame(&self) -> &DrawFrame {
        &self.front
    }

    /// 현재 기록 중인 프레임 (백 버퍼)
    pub fn recording_frame(&self) -> &DrawFrame {
        &self.back
    }

    pub fn current_generation(&self) -> u64 { self.current_generation }
    pub fn frames_drawn(&self) -> u64 { self.frames_drawn }
    pub fn peak_commands(&self) -> usize { self.peak_commands }

    /// 프론트 버퍼에 유효한 프레임이 있는지
    pub fn has_valid_frame(&self) -> bool {
        self.front.is_complete()
    }
}

/// 위젯별 서브트리 캐시 엔트리
#[derive(Debug, Clone)]
pub struct SubtreeCache {
    pub widget_id: u64,
    pub commands: Vec<DrawCommand>,
    pub generation: u64,
    pub is_valid: bool,
}

impl SubtreeCache {
    pub fn new(widget_id: u64) -> Self {
        Self {
            widget_id,
            commands: Vec::new(),
            generation: 0,
            is_valid: false,
        }
    }

    pub fn store(&mut self, commands: Vec<DrawCommand>, generation: u64) {
        self.commands = commands;
        self.generation = generation;
        self.is_valid = true;
    }

    pub fn invalidate(&mut self) {
        self.is_valid = false;
    }

    pub fn command_count(&self) -> usize { self.commands.len() }
}

/// Deferred painting 큐
pub struct DeferredPaintQueue {
    entries: Vec<DeferredPaintEntry>,
}

#[derive(Debug, Clone)]
pub struct DeferredPaintEntry {
    pub widget_id: u64,
    pub layer: u32,
    pub priority: i32,
}

impl DeferredPaintQueue {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn enqueue(&mut self, widget_id: u64, layer: u32, priority: i32) {
        self.entries.push(DeferredPaintEntry { widget_id, layer, priority });
    }

    /// 우선순위 높은 순서로 정렬
    pub fn sort_by_priority(&mut self) {
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    pub fn drain(&mut self) -> Vec<DeferredPaintEntry> {
        let result = std::mem::take(&mut self.entries);
        result
    }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
    pub fn len(&self) -> usize { self.entries.len() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    #[test]
    fn test_draw_frame() {
        let mut frame = DrawFrame::new();
        frame.push(DrawCommand::Clear { color: Color::BLACK });
        frame.push(DrawCommand::Box {
            layer: 0,
            geometry: PaintGeometry::new(Vec2::ZERO, Vec2::new(100.0, 50.0), 1.0),
            color: Color::WHITE,
        });
        assert_eq!(frame.command_count(), 2);
        assert!(!frame.is_complete());
        frame.finish(1);
        assert!(frame.is_complete());
        assert_eq!(frame.generation(), 1);
    }

    #[test]
    fn test_double_buffer() {
        let mut buf = SlateDrawBuffer::new();
        assert!(!buf.has_valid_frame());

        // 프레임 1
        buf.begin_frame();
        buf.push_command(DrawCommand::Clear { color: Color::BLACK });
        buf.push_command(DrawCommand::Box {
            layer: 0,
            geometry: PaintGeometry::new(Vec2::ZERO, Vec2::new(100.0, 50.0), 1.0),
            color: Color::WHITE,
        });
        buf.end_frame();
        assert!(buf.has_valid_frame());
        assert_eq!(buf.render_frame().command_count(), 2);
        assert_eq!(buf.frames_drawn(), 1);

        // 프레임 2
        buf.begin_frame();
        buf.push_command(DrawCommand::Clear { color: Color::BLACK });
        buf.end_frame();
        assert_eq!(buf.render_frame().command_count(), 1);
        assert_eq!(buf.frames_drawn(), 2);
    }

    #[test]
    fn test_subtree_cache() {
        let mut cache = SubtreeCache::new(42);
        assert!(!cache.is_valid);
        cache.store(vec![DrawCommand::Clear { color: Color::BLACK }], 1);
        assert!(cache.is_valid);
        assert_eq!(cache.command_count(), 1);
        cache.invalidate();
        assert!(!cache.is_valid);
    }

    #[test]
    fn test_deferred_paint_queue() {
        let mut queue = DeferredPaintQueue::new();
        queue.enqueue(1, 0, 10);
        queue.enqueue(2, 1, 5);
        queue.enqueue(3, 0, 20);
        queue.sort_by_priority();
        let entries = queue.drain();
        assert_eq!(entries[0].widget_id, 3); // 최고 우선순위
        assert_eq!(entries[1].widget_id, 1);
        assert_eq!(entries[2].widget_id, 2);
    }

    #[test]
    fn test_peak_commands() {
        let mut buf = SlateDrawBuffer::new();
        buf.begin_frame();
        for _ in 0..10 {
            buf.push_command(DrawCommand::Clear { color: Color::BLACK });
        }
        buf.end_frame();
        assert_eq!(buf.peak_commands(), 10);

        buf.begin_frame();
        for _ in 0..5 {
            buf.push_command(DrawCommand::Clear { color: Color::BLACK });
        }
        buf.end_frame();
        assert_eq!(buf.peak_commands(), 10); // peak는 유지
    }
}
