//! Render Thread -- GT/RT 분리 인프라
//!
//! 렌더링 커맨드를 전용 스레드에서 실행하여 GT(Game Thread)와 RT(Render Thread)를 분리.
//! wgpu 28.0의 Send+Sync 보장 덕분에 unsafe 코드 없이 구현.

mod commands;
mod fence;
mod draw_buffer_rt;
mod health;
mod profiling;
mod render_loop;

pub use commands::*;
pub use fence::*;
pub use draw_buffer_rt::*;
pub use health::RenderThreadHealth;
pub use profiling::{RtProfilingStats, ProfilingSnapshot};
pub(crate) use render_loop::render_thread_main;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Condvar, Mutex};

/// 3D 씬 렌더러 인터페이스 (크레이트 경계 추상화)
///
/// main crate의 RenderState가 구현.
/// RT가 `Box<dyn SceneRenderer>`로 소유하여 RenderScene 커맨드 처리.
pub trait SceneRenderer: Send + 'static {
    /// 3D 씬 렌더링 실행
    ///
    /// `data`는 `Box<SceneRenderData>`를 downcast하여 사용.
    fn render(&mut self, data: Box<dyn std::any::Any + Send>);

    /// 렌더링 후 뷰포트 텍스처 정보 반환 (UI 등록용)
    fn viewport_texture_infos(&self) -> Vec<ViewportTextureInfo>;
}
use std::thread::JoinHandle;

/// RT → GT 프레임 완료 시그널 (Condvar 기반)
///
/// UE5 FFrameEndSync 패턴: RT가 명령 처리 후 AtomicU64 업데이트 + Condvar notify,
/// GT가 frame_gate()에서 Condvar wait으로 블록 대기.
pub(crate) struct FrameCompleteSignal {
    pub last_completed_frame: AtomicU64,
    pub mutex: Mutex<()>,
    pub condvar: Condvar,
}

impl FrameCompleteSignal {
    fn new() -> Self {
        Self {
            last_completed_frame: AtomicU64::new(0),
            mutex: Mutex::new(()),
            condvar: Condvar::new(),
        }
    }

    /// RT에서 호출: 프레임 완료 기록 + GT 깨우기
    pub fn signal(&self, frame_number: u64) {
        self.last_completed_frame.store(frame_number, Ordering::Release);
        // Condvar notify — GT가 wait 중이면 깨움
        // lock을 잡지 않고 notify해도 안전 (spurious wakeup 허용 설계)
        self.condvar.notify_one();
    }
}

/// 렌더 스레드 핸들
///
/// GT에서 렌더링 커맨드를 전송하고, RT 완료를 동기화하는 인터페이스.
/// UE5의 FRenderCommandFence + TaskGraph 패턴을 단순화한 구조.
pub struct RenderThread {
    thread: Option<JoinHandle<()>>,
    cmd_tx: SyncSender<RenderCommand>,
    fence_counter: AtomicU64,
    // Feature 1: RT 헬스 모니터링
    health: RenderThreadHealth,
    // Feature 2: 프레임 파이프라인 동기화 (Condvar 기반)
    frame_signal: Arc<FrameCompleteSignal>,
    current_gt_frame: AtomicU64,
    pipeline_depth: u32,
    // Feature 3: 명령 프로파일링
    profiling_stats: Arc<RtProfilingStats>,
}

impl RenderThread {
    /// RT 스레드 생성 및 시작
    ///
    /// `init_data`의 모든 GPU 리소스가 RT로 move됨 (Send -- unsafe 불필요).
    /// `sync_channel(2)` -> 최대 N-1 프레임 오버랩 허용.
    pub fn spawn(init_data: RenderThreadInitData) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::sync_channel::<RenderCommand>(2);

        let health = RenderThreadHealth::new();
        let rt_health = health.clone();

        let profiling_stats = Arc::new(RtProfilingStats::new());
        let rt_profiling = Arc::clone(&profiling_stats);

        let frame_signal = Arc::new(FrameCompleteSignal::new());
        let rt_frame_signal = Arc::clone(&frame_signal);

        let thread = std::thread::Builder::new()
            .name("RenderThread".to_string())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    log::info!("[RenderThread] Started");
                    render_thread_main(init_data, cmd_rx, rt_frame_signal, rt_profiling);
                    log::info!("[RenderThread] Stopped");
                }));
                if let Err(panic_info) = result {
                    let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                        s.to_string()
                    } else if let Some(s) = panic_info.downcast_ref::<String>() {
                        s.clone()
                    } else {
                        "Unknown panic".to_string()
                    };
                    log::error!("[RenderThread] Panic: {}", msg);
                    rt_health.set_panicked(msg);
                }
            })
            .expect("Failed to spawn render thread");

        Self {
            thread: Some(thread),
            cmd_tx,
            fence_counter: AtomicU64::new(0),
            health,
            frame_signal,
            current_gt_frame: AtomicU64::new(0),
            pipeline_depth: 1,
            profiling_stats,
        }
    }

    // ========================================================================
    // Feature 1: RT 헬스 모니터링
    // ========================================================================

    /// RT가 정상 동작 중인지 확인
    pub fn is_healthy(&self) -> bool {
        self.health.is_healthy()
    }

    /// RT 패닉 메시지 조회
    pub fn get_error(&self) -> Option<String> {
        self.health.get_error()
    }

    /// 렌더링 커맨드 전송 (non-blocking if channel has capacity)
    pub fn send(&self, cmd: RenderCommand) {
        if !self.is_healthy() {
            log::error!("[RenderThread] Cannot send — RT has panicked");
            return;
        }
        if let Err(e) = self.cmd_tx.send(cmd) {
            log::error!("[RenderThread] Failed to send command: {}", e);
        }
    }

    /// UE5 FlushRenderingCommands 패턴: RT가 현재까지의 모든 커맨드를 완료할 때까지 블록
    ///
    /// 모달 리사이즈, 종료 전 동기화 등에 사용.
    pub fn flush(&self) {
        let fence = RenderCommandFence::new(&self.fence_counter);
        let signal = fence.signal_pair();
        let fence_id = fence.id();
        self.send(RenderCommand::SignalFence { fence_id, signal });
        fence.wait();
    }

    // ========================================================================
    // Feature 2: 프레임 파이프라인 동기화 (UE5 FFrameEndSync — Condvar 기반)
    // ========================================================================

    /// GT가 N-pipeline_depth 프레임 완료까지 대기 (UE5 FFrameEndSync::Sync)
    ///
    /// Condvar 기반 블록 대기 — busy-wait 없음.
    /// RT가 명령 처리 후 `FrameCompleteSignal::signal()`로 Condvar notify.
    pub fn frame_gate(&self) {
        let current = self.current_gt_frame.load(Ordering::Relaxed);
        if current <= self.pipeline_depth as u64 {
            // 초기 프레임은 대기 불필요
            return;
        }
        let target = current - self.pipeline_depth as u64;

        // 이미 완료된 경우 즉시 반환 (lock 없이 fast path)
        if self.frame_signal.last_completed_frame.load(Ordering::Acquire) >= target {
            return;
        }

        // Condvar 대기 — RT의 signal()이 깨움
        let guard = self.frame_signal.mutex.lock().unwrap();
        let timeout = std::time::Duration::from_secs(5);
        let result = self.frame_signal.condvar.wait_timeout_while(
            guard,
            timeout,
            |_| self.frame_signal.last_completed_frame.load(Ordering::Acquire) < target,
        ).unwrap();

        if result.1.timed_out() {
            log::warn!("[RenderThread] frame_gate timeout (5s) — target frame {}", target);
        }
    }

    /// 프레임 종료 시 호출 — GT 프레임 카운터 증가
    pub fn advance_frame(&self) -> u64 {
        self.current_gt_frame.fetch_add(1, Ordering::Release) + 1
    }

    /// 현재 GT 프레임 번호
    pub fn current_frame(&self) -> u64 {
        self.current_gt_frame.load(Ordering::Relaxed)
    }

    /// 파이프라인 깊이 설정
    pub fn set_pipeline_depth(&mut self, depth: u32) {
        self.pipeline_depth = depth.max(1);
    }

    // ========================================================================
    // Feature 3: 명령 프로파일링
    // ========================================================================

    /// 현재 프로파일링 스냅샷 조회 (GT에서 호출)
    pub fn profiling_snapshot(&self) -> ProfilingSnapshot {
        ProfilingSnapshot::from_stats(&self.profiling_stats)
    }

    // ========================================================================
    // Feature 5: Config RT 전파
    // ========================================================================

    /// RT에 설정 업데이트 전송
    pub fn update_config(&self, config: RenderConfig) {
        self.send(RenderCommand::UpdateConfig(config));
    }

    /// RT 종료 + join
    pub fn shutdown(mut self) {
        log::info!("[RenderThread] Requesting shutdown...");
        self.send(RenderCommand::Shutdown);
        if let Some(thread) = self.thread.take() {
            if let Err(e) = thread.join() {
                log::error!("[RenderThread] Join failed: {:?}", e);
            }
        }
    }
}

impl Drop for RenderThread {
    fn drop(&mut self) {
        if self.thread.is_some() {
            log::warn!("[RenderThread] Dropped without explicit shutdown -- sending Shutdown");
            let _ = self.cmd_tx.send(RenderCommand::Shutdown);
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }
}
