// SKOPE Engine - GPU Profiler
//
// wgpu timestamp queries를 사용한 패스별 GPU 시간 측정
// 설계 문서 Section 9.1 GPU PROFILER 구현
//
// Reference: SKOPE Engine Rendering Pipeline v1.1 Design Doc

use std::collections::HashMap;
use wgpu;

/// 프로파일링할 렌더 패스 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderPass {
    // Phase 1: Visibility
    ZPrepass,
    VBuffer,
    VBufferOIT,

    // Phase 2: Geometry Processing
    MotionVectors,
    HZBGeneration,

    // Phase 3: Shadows
    CascadedShadows,
    ShadowAtlas,
    ContactShadows,

    // Phase 4: Global Illumination
    DDGIRayTrace,
    DDGIProbeUpdate,
    DDGIIrradianceBlend,

    // Phase 5: Material & Lighting
    MaterialEval,
    ClusteredLighting,
    DeferredLighting,

    // Phase 6: Screen-Space Effects
    GTAO,
    SSR,
    SSS,
    ScreenComposite,

    // Phase 7: Volumetrics & VFX
    VolumetricFog,
    MagicCircle,
    StochasticTransparency,
    OITResolve,

    // Phase 8: Post Processing
    TAA,
    Bloom,
    DoF,
    MotionBlur,
    Tonemapping,

    // Phase 9: Final
    Blit,
    UI,

    // Total frame
    FrameTotal,
}

impl RenderPass {
    /// 모든 패스 목록
    pub fn all() -> &'static [RenderPass] {
        &[
            RenderPass::ZPrepass,
            RenderPass::VBuffer,
            RenderPass::VBufferOIT,
            RenderPass::MotionVectors,
            RenderPass::HZBGeneration,
            RenderPass::CascadedShadows,
            RenderPass::ShadowAtlas,
            RenderPass::ContactShadows,
            RenderPass::DDGIRayTrace,
            RenderPass::DDGIProbeUpdate,
            RenderPass::DDGIIrradianceBlend,
            RenderPass::MaterialEval,
            RenderPass::ClusteredLighting,
            RenderPass::DeferredLighting,
            RenderPass::GTAO,
            RenderPass::SSR,
            RenderPass::SSS,
            RenderPass::ScreenComposite,
            RenderPass::VolumetricFog,
            RenderPass::MagicCircle,
            RenderPass::StochasticTransparency,
            RenderPass::OITResolve,
            RenderPass::TAA,
            RenderPass::Bloom,
            RenderPass::DoF,
            RenderPass::MotionBlur,
            RenderPass::Tonemapping,
            RenderPass::Blit,
            RenderPass::UI,
            RenderPass::FrameTotal,
        ]
    }

    /// 패스 이름 (UI 표시용)
    pub fn name(&self) -> &'static str {
        match self {
            RenderPass::ZPrepass => "Z-Prepass",
            RenderPass::VBuffer => "V-Buffer",
            RenderPass::VBufferOIT => "V-Buffer OIT",
            RenderPass::MotionVectors => "Motion Vectors",
            RenderPass::HZBGeneration => "HZB Generation",
            RenderPass::CascadedShadows => "CSM Shadows",
            RenderPass::ShadowAtlas => "Shadow Atlas",
            RenderPass::ContactShadows => "Contact Shadows",
            RenderPass::DDGIRayTrace => "DDGI Ray Trace",
            RenderPass::DDGIProbeUpdate => "DDGI Probe Update",
            RenderPass::DDGIIrradianceBlend => "DDGI Irradiance",
            RenderPass::MaterialEval => "Material Eval",
            RenderPass::ClusteredLighting => "Clustered Lighting",
            RenderPass::DeferredLighting => "Deferred Lighting",
            RenderPass::GTAO => "GTAO",
            RenderPass::SSR => "SSR",
            RenderPass::SSS => "SSS",
            RenderPass::ScreenComposite => "SS Composite",
            RenderPass::VolumetricFog => "Volumetric Fog",
            RenderPass::MagicCircle => "Magic Circle",
            RenderPass::StochasticTransparency => "Stochastic Trans",
            RenderPass::OITResolve => "OIT Resolve",
            RenderPass::TAA => "TAA",
            RenderPass::Bloom => "Bloom",
            RenderPass::DoF => "Depth of Field",
            RenderPass::MotionBlur => "Motion Blur",
            RenderPass::Tonemapping => "Tonemapping",
            RenderPass::Blit => "Blit",
            RenderPass::UI => "UI",
            RenderPass::FrameTotal => "Total Frame",
        }
    }

    /// 패스가 속한 Phase 번호
    pub fn phase(&self) -> u32 {
        match self {
            RenderPass::ZPrepass | RenderPass::VBuffer | RenderPass::VBufferOIT => 1,
            RenderPass::MotionVectors | RenderPass::HZBGeneration => 2,
            RenderPass::CascadedShadows | RenderPass::ShadowAtlas | RenderPass::ContactShadows => 3,
            RenderPass::DDGIRayTrace | RenderPass::DDGIProbeUpdate | RenderPass::DDGIIrradianceBlend => 4,
            RenderPass::MaterialEval | RenderPass::ClusteredLighting | RenderPass::DeferredLighting => 5,
            RenderPass::GTAO | RenderPass::SSR | RenderPass::SSS | RenderPass::ScreenComposite => 6,
            RenderPass::VolumetricFog | RenderPass::MagicCircle | RenderPass::StochasticTransparency | RenderPass::OITResolve => 7,
            RenderPass::TAA | RenderPass::Bloom | RenderPass::DoF | RenderPass::MotionBlur | RenderPass::Tonemapping => 8,
            RenderPass::Blit | RenderPass::UI => 9,
            RenderPass::FrameTotal => 0,
        }
    }
}

/// 패스별 시간 측정 결과
#[derive(Debug, Clone, Default)]
pub struct PassTiming {
    /// 현재 프레임 시간 (ms)
    pub current_ms: f64,

    /// 이동 평균 시간 (ms)
    pub avg_ms: f64,

    /// 최소 시간 (ms)
    pub min_ms: f64,

    /// 최대 시간 (ms)
    pub max_ms: f64,

    /// 측정 횟수
    pub sample_count: u64,
}

impl PassTiming {
    pub fn new() -> Self {
        Self {
            current_ms: 0.0,
            avg_ms: 0.0,
            min_ms: f64::MAX,
            max_ms: 0.0,
            sample_count: 0,
        }
    }

    /// 새 샘플 추가
    pub fn add_sample(&mut self, time_ms: f64) {
        self.current_ms = time_ms;
        self.min_ms = self.min_ms.min(time_ms);
        self.max_ms = self.max_ms.max(time_ms);

        // 지수 이동 평균 (EMA)
        const ALPHA: f64 = 0.1;
        if self.sample_count == 0 {
            self.avg_ms = time_ms;
        } else {
            self.avg_ms = self.avg_ms * (1.0 - ALPHA) + time_ms * ALPHA;
        }
        self.sample_count += 1;
    }

    /// 통계 리셋
    pub fn reset_stats(&mut self) {
        self.min_ms = f64::MAX;
        self.max_ms = 0.0;
        self.sample_count = 0;
    }
}

/// GPU 프로파일러 설정
#[derive(Debug, Clone)]
pub struct ProfilerConfig {
    /// 프로파일링 활성화
    pub enabled: bool,

    /// 버퍼링할 프레임 수 (latency)
    pub buffer_frames: u32,

    /// 결과 평균화 프레임 수
    pub averaging_frames: u32,
}

impl Default for ProfilerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            buffer_frames: 3,
            averaging_frames: 60,
        }
    }
}

/// GPU 프로파일러
///
/// wgpu timestamp queries를 사용해 각 렌더 패스의 GPU 실행 시간을 측정합니다.
pub struct GpuProfiler {
    /// 설정
    config: ProfilerConfig,

    /// Timestamp query set (begin/end per pass)
    query_set: Option<wgpu::QuerySet>,

    /// 결과 resolve용 버퍼
    resolve_buffer: Option<wgpu::Buffer>,

    /// CPU 읽기용 스테이징 버퍼
    staging_buffer: Option<wgpu::Buffer>,

    /// 패스별 타이밍 결과
    timings: HashMap<RenderPass, PassTiming>,

    /// 타임스탬프 주기 (나노초/틱)
    timestamp_period: f32,

    /// 현재 쿼리 인덱스
    current_query_index: u32,

    /// 프레임 인덱스 (버퍼링용)
    frame_index: u64,

    /// 대기 중인 읽기 요청
    pending_reads: Vec<PendingRead>,

    /// 패스별 쿼리 인덱스 매핑
    pass_query_indices: HashMap<RenderPass, (u32, u32)>,

    /// 기능 지원 여부
    timestamps_supported: bool,
}

struct PendingRead {
    frame: u64,
    buffer: wgpu::Buffer,
    query_count: u32,
}

/// 최대 쿼리 수 (begin + end per pass)
const MAX_QUERIES: u32 = 64;

impl GpuProfiler {
    pub fn new(device: &wgpu::Device, config: ProfilerConfig) -> Self {
        // 타임스탬프 쿼리 지원 확인
        let timestamps_supported = device.features().contains(wgpu::Features::TIMESTAMP_QUERY);

        // Note: timestamp_period는 queue에서 가져와야 하지만, 생성 시점에는 queue가 없으므로
        // 나중에 update_timestamp_period()로 설정하거나 기본값 1.0 사용
        // 대부분의 GPU에서 나노초 단위이므로 1.0이 적절한 기본값
        let timestamp_period = 1.0_f32;

        let (query_set, resolve_buffer, staging_buffer) = if timestamps_supported && config.enabled {
            let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("GPU Profiler Query Set"),
                ty: wgpu::QueryType::Timestamp,
                count: MAX_QUERIES,
            });

            let resolve_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("GPU Profiler Resolve Buffer"),
                size: (MAX_QUERIES as u64) * 8, // u64 per timestamp
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });

            let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("GPU Profiler Staging Buffer"),
                size: (MAX_QUERIES as u64) * 8,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });

            (Some(query_set), Some(resolve_buffer), Some(staging_buffer))
        } else {
            if config.enabled && !timestamps_supported {
                log::warn!("[GpuProfiler] Timestamp queries not supported by device");
            }
            (None, None, None)
        };

        let mut timings = HashMap::new();
        for pass in RenderPass::all() {
            timings.insert(*pass, PassTiming::new());
        }

        Self {
            config,
            query_set,
            resolve_buffer,
            staging_buffer,
            timings,
            timestamp_period,
            current_query_index: 0,
            frame_index: 0,
            pending_reads: Vec::new(),
            pass_query_indices: HashMap::new(),
            timestamps_supported,
        }
    }

    /// 프레임 시작
    pub fn begin_frame(&mut self) {
        self.current_query_index = 0;
        self.pass_query_indices.clear();
    }

    /// 패스 시작 타임스탬프 기록
    pub fn begin_pass(&mut self, encoder: &mut wgpu::CommandEncoder, pass: RenderPass) {
        if !self.config.enabled || !self.timestamps_supported {
            return;
        }

        if let Some(ref query_set) = self.query_set {
            if self.current_query_index + 2 <= MAX_QUERIES {
                let begin_idx = self.current_query_index;
                encoder.write_timestamp(query_set, begin_idx);
                self.pass_query_indices.insert(pass, (begin_idx, begin_idx + 1));
                self.current_query_index += 1;
            }
        }
    }

    /// 패스 종료 타임스탬프 기록
    pub fn end_pass(&mut self, encoder: &mut wgpu::CommandEncoder, pass: RenderPass) {
        if !self.config.enabled || !self.timestamps_supported {
            return;
        }

        if let Some(ref query_set) = self.query_set {
            if let Some(&(_, end_idx)) = self.pass_query_indices.get(&pass) {
                encoder.write_timestamp(query_set, end_idx);
                self.current_query_index += 1;
            }
        }
    }

    /// 렌더 패스에 타임스탬프 writes 생성
    pub fn timestamp_writes(&self, pass: RenderPass) -> Option<wgpu::RenderPassTimestampWrites> {
        if !self.config.enabled || !self.timestamps_supported {
            return None;
        }

        self.query_set.as_ref().and_then(|query_set| {
            self.pass_query_indices.get(&pass).map(|&(begin, end)| {
                wgpu::RenderPassTimestampWrites {
                    query_set,
                    beginning_of_pass_write_index: Some(begin),
                    end_of_pass_write_index: Some(end),
                }
            })
        })
    }

    /// 렌더 패스용 타임스탬프 인덱스 예약
    pub fn reserve_pass_indices(&mut self, pass: RenderPass) -> Option<(u32, u32)> {
        if !self.config.enabled || !self.timestamps_supported {
            return None;
        }

        if self.current_query_index + 2 <= MAX_QUERIES {
            let begin_idx = self.current_query_index;
            let end_idx = begin_idx + 1;
            self.pass_query_indices.insert(pass, (begin_idx, end_idx));
            self.current_query_index += 2;
            Some((begin_idx, end_idx))
        } else {
            None
        }
    }

    /// 프레임 종료 - 쿼리 결과 resolve
    pub fn end_frame(&mut self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder) {
        if !self.config.enabled || !self.timestamps_supported {
            return;
        }

        let query_count = self.current_query_index;
        if query_count == 0 {
            return;
        }

        if let (Some(ref query_set), Some(ref resolve_buffer)) = (&self.query_set, &self.resolve_buffer) {
            // Resolve queries to buffer
            encoder.resolve_query_set(query_set, 0..query_count, resolve_buffer, 0);

            // Create staging buffer for this frame
            let staging = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("GPU Profiler Staging (Frame)"),
                size: (query_count as u64) * 8,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });

            // Copy to staging
            encoder.copy_buffer_to_buffer(resolve_buffer, 0, &staging, 0, (query_count as u64) * 8);

            // Queue for reading
            self.pending_reads.push(PendingRead {
                frame: self.frame_index,
                buffer: staging,
                query_count,
            });
        }

        self.frame_index += 1;
    }

    /// 대기 중인 결과 폴링
    ///
    /// Note: wgpu 27.0에서는 비동기 버퍼 매핑이 자동으로 처리됨
    /// 이 함수는 이미 매핑된 버퍼를 처리합니다.
    pub fn poll_results(&mut self, _device: &wgpu::Device) {
        if !self.config.enabled || !self.timestamps_supported {
            return;
        }

        // wgpu 27.0+에서는 buffer polling이 자동으로 처리됨
        // 여기서는 이미 완료된 pending reads만 처리

        let timestamp_period = self.timestamp_period;
        let timings = &mut self.timings;
        let pass_indices = self.pass_query_indices.clone();

        self.pending_reads.retain_mut(|pending| {
            // 버퍼가 매핑 가능한지 확인 (non-blocking)
            let slice = pending.buffer.slice(..);

            // 비동기 매핑 시도
            let mapped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let mapped_clone = mapped.clone();

            slice.map_async(wgpu::MapMode::Read, move |result| {
                if result.is_ok() {
                    mapped_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            });

            // 즉시 확인 (non-blocking)
            if mapped.load(std::sync::atomic::Ordering::SeqCst) {
                // 매핑 성공
                let data = slice.get_mapped_range();
                let timestamps: &[u64] = bytemuck::cast_slice(&data);

                // Calculate timings for each pass
                for (pass, (begin_idx, end_idx)) in &pass_indices {
                    let begin_idx = *begin_idx as usize;
                    let end_idx = *end_idx as usize;

                    if begin_idx < timestamps.len() && end_idx < timestamps.len() {
                        let begin_ts = timestamps[begin_idx];
                        let end_ts = timestamps[end_idx];

                        if end_ts >= begin_ts {
                            let ticks = (end_ts - begin_ts) as f64;
                            let ns = ticks * (timestamp_period as f64);
                            let ms = ns / 1_000_000.0;

                            if let Some(timing) = timings.get_mut(pass) {
                                timing.add_sample(ms);
                            }
                        }
                    }
                }

                drop(data);
                pending.buffer.unmap();
                false // Remove from pending
            } else {
                // 아직 준비되지 않음 - 다음 프레임에 재시도
                // 오래된 요청은 버림 (3프레임 이상)
                if self.frame_index.saturating_sub(pending.frame) > 3 {
                    false // 너무 오래됨, 버림
                } else {
                    true // 유지
                }
            }
        });
    }

    /// 패스별 타이밍 조회
    pub fn get_timing(&self, pass: RenderPass) -> Option<&PassTiming> {
        self.timings.get(&pass)
    }

    /// 모든 타이밍 조회
    pub fn all_timings(&self) -> &HashMap<RenderPass, PassTiming> {
        &self.timings
    }

    /// 총 프레임 시간 (ms)
    pub fn total_frame_time_ms(&self) -> f64 {
        let mut total = 0.0;
        for (pass, timing) in &self.timings {
            if *pass != RenderPass::FrameTotal {
                total += timing.current_ms;
            }
        }
        total
    }

    /// 예상 FPS
    pub fn estimated_fps(&self) -> f32 {
        let frame_ms = self.total_frame_time_ms();
        if frame_ms > 0.0 {
            (1000.0 / frame_ms) as f32
        } else {
            0.0
        }
    }

    /// 가장 느린 패스 찾기
    pub fn slowest_pass(&self) -> Option<(RenderPass, f64)> {
        self.timings
            .iter()
            .filter(|(p, _)| **p != RenderPass::FrameTotal)
            .max_by(|(_, a), (_, b)| a.current_ms.partial_cmp(&b.current_ms).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(pass, timing)| (*pass, timing.current_ms))
    }

    /// Phase별 총 시간 계산
    pub fn phase_timings(&self) -> HashMap<u32, f64> {
        let mut phases: HashMap<u32, f64> = HashMap::new();

        for (pass, timing) in &self.timings {
            if *pass != RenderPass::FrameTotal {
                let phase = pass.phase();
                *phases.entry(phase).or_insert(0.0) += timing.current_ms;
            }
        }

        phases
    }

    /// 통계 리셋
    pub fn reset_stats(&mut self) {
        for timing in self.timings.values_mut() {
            timing.reset_stats();
        }
    }

    /// 프로파일링 활성화/비활성화
    pub fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
    }

    /// 프로파일링 활성화 여부
    pub fn is_enabled(&self) -> bool {
        self.config.enabled && self.timestamps_supported
    }

    /// 타임스탬프 지원 여부
    pub fn is_supported(&self) -> bool {
        self.timestamps_supported
    }

    /// Query set 참조 (렌더 패스 생성용)
    pub fn query_set(&self) -> Option<&wgpu::QuerySet> {
        if self.config.enabled && self.timestamps_supported {
            self.query_set.as_ref()
        } else {
            None
        }
    }

    /// 프로파일링 보고서 생성
    pub fn generate_report(&self) -> ProfilerReport {
        let mut pass_times: Vec<(RenderPass, f64)> = self.timings
            .iter()
            .filter(|(p, _)| **p != RenderPass::FrameTotal)
            .map(|(p, t)| (*p, t.avg_ms))
            .collect();

        pass_times.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

        let phase_times = self.phase_timings();
        let total_ms = self.total_frame_time_ms();

        ProfilerReport {
            total_frame_ms: total_ms,
            estimated_fps: self.estimated_fps(),
            pass_times,
            phase_times,
            slowest_pass: self.slowest_pass(),
        }
    }
}

/// 프로파일링 보고서
#[derive(Debug, Clone)]
pub struct ProfilerReport {
    pub total_frame_ms: f64,
    pub estimated_fps: f32,
    pub pass_times: Vec<(RenderPass, f64)>,
    pub phase_times: HashMap<u32, f64>,
    pub slowest_pass: Option<(RenderPass, f64)>,
}

impl ProfilerReport {
    /// 텍스트 형식으로 출력
    pub fn to_string_pretty(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("═══════════════════════════════════════════════════\n"));
        s.push_str(&format!(" GPU PROFILER REPORT\n"));
        s.push_str(&format!("═══════════════════════════════════════════════════\n"));
        s.push_str(&format!(" Total Frame: {:.2}ms ({:.1} FPS)\n", self.total_frame_ms, self.estimated_fps));
        s.push_str(&format!("───────────────────────────────────────────────────\n"));

        // Phase별 시간
        let mut phases: Vec<_> = self.phase_times.iter().collect();
        phases.sort_by_key(|(p, _)| *p);

        for (phase, time) in phases {
            let percent = if self.total_frame_ms > 0.0 {
                (time / self.total_frame_ms * 100.0) as u32
            } else {
                0
            };
            let bar = "█".repeat((percent / 5).min(20) as usize);
            s.push_str(&format!(" Phase {}: {:>6.2}ms {:>3}% {}\n", phase, time, percent, bar));
        }

        s.push_str(&format!("───────────────────────────────────────────────────\n"));
        s.push_str(&format!(" Top 10 Passes:\n"));

        for (i, (pass, time)) in self.pass_times.iter().take(10).enumerate() {
            let percent = if self.total_frame_ms > 0.0 {
                (time / self.total_frame_ms * 100.0) as u32
            } else {
                0
            };
            s.push_str(&format!(" {:2}. {:20} {:>6.2}ms {:>3}%\n", i + 1, pass.name(), time, percent));
        }

        if let Some((pass, time)) = &self.slowest_pass {
            s.push_str(&format!("───────────────────────────────────────────────────\n"));
            s.push_str(&format!(" ⚠ Bottleneck: {} ({:.2}ms)\n", pass.name(), time));
        }

        s.push_str(&format!("═══════════════════════════════════════════════════\n"));
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pass_timing() {
        let mut timing = PassTiming::new();
        timing.add_sample(1.0);
        timing.add_sample(2.0);
        timing.add_sample(3.0);

        assert!(timing.avg_ms > 0.0);
        assert_eq!(timing.min_ms, 1.0);
        assert_eq!(timing.max_ms, 3.0);
    }

    #[test]
    fn test_render_pass_phases() {
        assert_eq!(RenderPass::ZPrepass.phase(), 1);
        assert_eq!(RenderPass::MaterialEval.phase(), 5);
        assert_eq!(RenderPass::TAA.phase(), 8);
    }
}
