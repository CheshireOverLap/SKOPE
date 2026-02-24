//! RT 명령 프로파일링 — 프레임 타이밍 계측
//!
//! UE5: FRenderCommandTag + CSV 계측
//! GT가 읽는 공유 통계(AtomicU64)와 RT 로컬 누산기.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// 공유 프로파일링 통계 (GT 읽기, RT 쓰기 — lock-free AtomicU64)
pub struct RtProfilingStats {
    /// 프레임 총 처리 시간 (마이크로초)
    pub frame_time_us: AtomicU64,
    /// recv 대기 시간 (마이크로초)
    pub idle_time_us: AtomicU64,
    /// DrawWindows 처리 시간 (마이크로초)
    pub draw_windows_us: AtomicU64,
    /// RenderScene 처리 시간 (마이크로초)
    pub render_scene_us: AtomicU64,
    /// 프레임당 명령 수
    pub command_count: AtomicU64,
    /// 해당 프레임 번호
    pub frame_number: AtomicU64,
}

impl RtProfilingStats {
    pub fn new() -> Self {
        Self {
            frame_time_us: AtomicU64::new(0),
            idle_time_us: AtomicU64::new(0),
            draw_windows_us: AtomicU64::new(0),
            render_scene_us: AtomicU64::new(0),
            command_count: AtomicU64::new(0),
            frame_number: AtomicU64::new(0),
        }
    }
}

/// GT 스냅샷 (plain values, Atomic에서 읽어온 복사본)
pub struct ProfilingSnapshot {
    pub frame_time: Duration,
    pub idle_time: Duration,
    pub draw_windows_time: Duration,
    pub render_scene_time: Duration,
    pub command_count: u64,
    pub frame_number: u64,
}

impl ProfilingSnapshot {
    pub fn from_stats(stats: &RtProfilingStats) -> Self {
        Self {
            frame_time: Duration::from_micros(stats.frame_time_us.load(Ordering::Relaxed)),
            idle_time: Duration::from_micros(stats.idle_time_us.load(Ordering::Relaxed)),
            draw_windows_time: Duration::from_micros(stats.draw_windows_us.load(Ordering::Relaxed)),
            render_scene_time: Duration::from_micros(stats.render_scene_us.load(Ordering::Relaxed)),
            command_count: stats.command_count.load(Ordering::Relaxed),
            frame_number: stats.frame_number.load(Ordering::Relaxed),
        }
    }
}

/// RT 로컬 누산기 — 프레임 내 타이밍을 축적한 뒤 publish()로 공유 통계에 기록
pub(crate) struct RtProfilingAccumulator {
    frame_start: Instant,
    idle_start: Option<Instant>,
    total_idle_us: u64,
    draw_windows_us: u64,
    render_scene_us: u64,
    command_count: u64,
}

impl RtProfilingAccumulator {
    pub fn new() -> Self {
        Self {
            frame_start: Instant::now(),
            idle_start: None,
            total_idle_us: 0,
            draw_windows_us: 0,
            render_scene_us: 0,
            command_count: 0,
        }
    }

    /// recv 대기 시작
    pub fn begin_idle(&mut self) {
        self.idle_start = Some(Instant::now());
    }

    /// recv 대기 종료
    pub fn end_idle(&mut self) {
        if let Some(start) = self.idle_start.take() {
            self.total_idle_us += start.elapsed().as_micros() as u64;
        }
    }

    /// 명령 처리 기록
    pub fn record_command(&mut self, name: &str, duration: Duration) {
        self.command_count += 1;
        let us = duration.as_micros() as u64;
        match name {
            "DrawWindows" => self.draw_windows_us += us,
            "RenderScene" => self.render_scene_us += us,
            _ => {}
        }
    }

    /// 프레임 종료 시 공유 통계에 기록 + 리셋
    pub fn publish(&mut self, stats: &RtProfilingStats, frame_number: u64) {
        let frame_time_us = self.frame_start.elapsed().as_micros() as u64;
        stats.frame_time_us.store(frame_time_us, Ordering::Relaxed);
        stats.idle_time_us.store(self.total_idle_us, Ordering::Relaxed);
        stats.draw_windows_us.store(self.draw_windows_us, Ordering::Relaxed);
        stats.render_scene_us.store(self.render_scene_us, Ordering::Relaxed);
        stats.command_count.store(self.command_count, Ordering::Relaxed);
        stats.frame_number.store(frame_number, Ordering::Relaxed);

        // 리셋
        self.frame_start = Instant::now();
        self.total_idle_us = 0;
        self.draw_windows_us = 0;
        self.render_scene_us = 0;
        self.command_count = 0;
    }
}
