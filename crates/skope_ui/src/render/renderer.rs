//! RSlate Renderer
//!
//! wgpu-based renderer for skope_ui widgets.
//!
//! Architecture (UE5-style shared resources):
//! - SlateRenderResources: created once, holds pipeline + textures + text shared resources
//! - RSlateRenderer: per-window, holds only buffers + screen_size + caches
//! - Backward-compat: RSlateRenderer::new() creates owned resources internally

use std::collections::HashMap;
use std::path::Path;
use wgpu::util::DeviceExt;

use glam::Vec2;
use crate::core::{Geometry, SlateRect, PaintGeometry, SlateClippingState, FontFamily};
use crate::widget::{Widget, DrawElementList, DrawElement, PaintArgs};
use super::types::{SlateVertex, SlateRoundedVertex, SlateUniforms, SlateTexture};
use super::text_renderer::{SharedTextResources, TextViewport};

/// 배치 종류 (Normal 파이프라인 vs RoundedBox SDF 파이프라인)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BatchKind {
    Normal,
    RoundedBox,
}

/// 드로우 배치 (같은 텍스처 + 같은 클립 상태 + 같은 파이프라인을 사용하는 쿼드들)
#[derive(Clone)]
struct DrawBatch {
    kind: BatchKind,
    texture_name: Option<String>,
    /// 클립 상태 인덱스 (ClippingManager 내 인덱스, None = 클리핑 없음)
    clip_state_index: Option<usize>,
    index_start: u32,
    index_count: u32,
}

/// UE5 FSlateRenderBatch.NextBatchIndex 패턴 대응
/// 지오메트리 배치와 텍스트 범위를 하나의 레이어 순서 리스트로 통합
#[derive(Clone)]
enum DrawCommand {
    /// 지오메트리 배치 (cached_batches 인덱스)
    GeometryBatch(usize),
    /// 텍스트 인덱스 범위 (text_index_start..text_index_end)
    TextRange {
        index_start: u32,
        index_end: u32,
        clip_state_index: Option<usize>,
    },
}

/// 스텐실 write 엔트리 (하나의 스텐실 쿼드 draw call)
#[derive(Clone)]
struct StencilWriteEntry {
    /// 참조하는 클리핑 상태 인덱스
    clip_state_index: usize,
    /// 스텐실 참조값
    stencil_ref: u32,
    /// 인덱스 버퍼 시작 위치
    index_start: u32,
    /// 인덱스 수
    index_count: u32,
}

// ============================================================================
// Clipping Helpers
// ============================================================================

/// SlateClippingState → scissor rect [x, y, w, h] 변환
///
/// Scissor 상태: scissor_rect 직접 반환.
/// Stencil 상태: 보수적 AABB fallback (실제 stencil은 향후 구현).
fn resolve_scissor_rect(state: &SlateClippingState) -> [f32; 4] {
    if let Some(rect) = state.scissor_rect {
        rect
    } else if !state.stencil_quads.is_empty() {
        // 축 정렬 최적화: 모든 쿼드가 축 정렬이면 정확한 scissor 사용
        if state.stencil_quads.iter().all(|q| q.is_axis_aligned) {
            let mut min_x = f32::MAX;
            let mut min_y = f32::MAX;
            let mut max_x = f32::MIN;
            let mut max_y = f32::MIN;
            for quad in &state.stencil_quads {
                let r = quad.to_scissor_rect();
                min_x = min_x.min(r[0]);
                min_y = min_y.min(r[1]);
                max_x = max_x.max(r[0] + r[2]);
                max_y = max_y.max(r[1] + r[3]);
            }
            return [min_x, min_y, (max_x - min_x).max(0.0), (max_y - min_y).max(0.0)];
        }
        // 보수적 AABB fallback — 모든 stencil quad의 AABB 교차
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        for quad in &state.stencil_quads {
            let aabb = quad.to_aabb();
            min_x = min_x.min(aabb[0]);
            min_y = min_y.min(aabb[1]);
            max_x = max_x.max(aabb[0] + aabb[2]);
            max_y = max_y.max(aabb[1] + aabb[3]);
        }
        [min_x, min_y, (max_x - min_x).max(0.0), (max_y - min_y).max(0.0)]
    } else {
        // 빈 상태 → 전체 화면 (클리핑 없음)
        [0.0, 0.0, 99999.0, 99999.0]
    }
}

// ============================================================================
// Tessellation Helpers (RT + RenderOpacity aware)
// ============================================================================

/// 렌더 불투명도를 색상 알파에 적용
#[inline]
fn apply_render_opacity(color: [f32; 4], opacity: f32) -> [f32; 4] {
    [color[0], color[1], color[2], color[3] * opacity]
}

/// PaintGeometry 전체 영역 쿼드 emit (RT/opacity 자동 분기)
///
/// RT 없음: position + size (기존 fast path)
/// RT 있음: local_size corners → accumulated_render_transform
fn emit_quad(
    vertices: &mut Vec<SlateVertex>,
    indices: &mut Vec<u32>,
    geo: &PaintGeometry,
    color: [f32; 4],
    uvs: [[f32; 2]; 4],
) {
    let c = apply_render_opacity(color, geo.render_opacity());
    let base = vertices.len() as u32;
    if let Some(rt) = geo.render_transform() {
        let ls = geo.local_size();
        let p0 = rt.transform_point2(Vec2::ZERO);
        let p1 = rt.transform_point2(Vec2::new(ls.x, 0.0));
        let p2 = rt.transform_point2(ls);
        let p3 = rt.transform_point2(Vec2::new(0.0, ls.y));
        vertices.push(SlateVertex { position: [p0.x, p0.y], uv: uvs[0], color: c });
        vertices.push(SlateVertex { position: [p1.x, p1.y], uv: uvs[1], color: c });
        vertices.push(SlateVertex { position: [p2.x, p2.y], uv: uvs[2], color: c });
        vertices.push(SlateVertex { position: [p3.x, p3.y], uv: uvs[3], color: c });
    } else {
        // UE5 FSlateRenderer PixelSnapping: 꼭짓점을 정수 픽셀에 스냅하여
        // 서브픽셀 래스터화 떨림(jitter) 방지
        let x0 = geo.position.x.round();
        let y0 = geo.position.y.round();
        let x1 = (geo.position.x + geo.size.x).round();
        let y1 = (geo.position.y + geo.size.y).round();
        vertices.push(SlateVertex { position: [x0, y0], uv: uvs[0], color: c });
        vertices.push(SlateVertex { position: [x1, y0], uv: uvs[1], color: c });
        vertices.push(SlateVertex { position: [x1, y1], uv: uvs[2], color: c });
        vertices.push(SlateVertex { position: [x0, y1], uv: uvs[3], color: c });
    }
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

/// 로컬 공간 서브렉트 emit (보더 스트립, 인셋 영역 등)
///
/// (lx, ly, lw, lh)는 PaintGeometry의 로컬(위젯) 공간 좌표.
/// RT 없음: position + local×scale → absolute fast path
/// RT 있음: local corners → accumulated_render_transform
fn emit_local_rect(
    vertices: &mut Vec<SlateVertex>,
    indices: &mut Vec<u32>,
    geo: &PaintGeometry,
    color: [f32; 4],
    lx: f32, ly: f32, lw: f32, lh: f32,
) {
    let c = apply_render_opacity(color, geo.render_opacity());
    let base = vertices.len() as u32;
    if let Some(rt) = geo.render_transform() {
        let p0 = rt.transform_point2(Vec2::new(lx, ly));
        let p1 = rt.transform_point2(Vec2::new(lx + lw, ly));
        let p2 = rt.transform_point2(Vec2::new(lx + lw, ly + lh));
        let p3 = rt.transform_point2(Vec2::new(lx, ly + lh));
        vertices.push(SlateVertex { position: [p0.x, p0.y], uv: [0.0, 0.0], color: c });
        vertices.push(SlateVertex { position: [p1.x, p1.y], uv: [1.0, 0.0], color: c });
        vertices.push(SlateVertex { position: [p2.x, p2.y], uv: [1.0, 1.0], color: c });
        vertices.push(SlateVertex { position: [p3.x, p3.y], uv: [0.0, 1.0], color: c });
    } else {
        let s = geo.scale;
        // 픽셀 스냅 (emit_quad와 동일)
        let x0 = (geo.position.x + lx * s).round();
        let y0 = (geo.position.y + ly * s).round();
        let x1 = (geo.position.x + (lx + lw) * s).round();
        let y1 = (geo.position.y + (ly + lh) * s).round();
        vertices.push(SlateVertex { position: [x0, y0], uv: [0.0, 0.0], color: c });
        vertices.push(SlateVertex { position: [x1, y0], uv: [1.0, 0.0], color: c });
        vertices.push(SlateVertex { position: [x1, y1], uv: [1.0, 1.0], color: c });
        vertices.push(SlateVertex { position: [x0, y1], uv: [0.0, 1.0], color: c });
    }
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

/// 그래디언트 쿼드 emit (코너별 색상, RT/opacity 자동 분기)
fn emit_quad_gradient(
    vertices: &mut Vec<SlateVertex>,
    indices: &mut Vec<u32>,
    geo: &PaintGeometry,
    colors: [[f32; 4]; 4], // TL, TR, BR, BL
    uvs: [[f32; 2]; 4],
) {
    let opacity = geo.render_opacity();
    let base = vertices.len() as u32;
    if let Some(rt) = geo.render_transform() {
        let ls = geo.local_size();
        let p0 = rt.transform_point2(Vec2::ZERO);
        let p1 = rt.transform_point2(Vec2::new(ls.x, 0.0));
        let p2 = rt.transform_point2(ls);
        let p3 = rt.transform_point2(Vec2::new(0.0, ls.y));
        vertices.push(SlateVertex { position: [p0.x, p0.y], uv: uvs[0], color: apply_render_opacity(colors[0], opacity) });
        vertices.push(SlateVertex { position: [p1.x, p1.y], uv: uvs[1], color: apply_render_opacity(colors[1], opacity) });
        vertices.push(SlateVertex { position: [p2.x, p2.y], uv: uvs[2], color: apply_render_opacity(colors[2], opacity) });
        vertices.push(SlateVertex { position: [p3.x, p3.y], uv: uvs[3], color: apply_render_opacity(colors[3], opacity) });
    } else {
        // 픽셀 스냅
        let x0 = geo.position.x.round();
        let y0 = geo.position.y.round();
        let x1 = (geo.position.x + geo.size.x).round();
        let y1 = (geo.position.y + geo.size.y).round();
        vertices.push(SlateVertex { position: [x0, y0], uv: uvs[0], color: apply_render_opacity(colors[0], opacity) });
        vertices.push(SlateVertex { position: [x1, y0], uv: uvs[1], color: apply_render_opacity(colors[1], opacity) });
        vertices.push(SlateVertex { position: [x1, y1], uv: uvs[2], color: apply_render_opacity(colors[2], opacity) });
        vertices.push(SlateVertex { position: [x0, y1], uv: uvs[3], color: apply_render_opacity(colors[3], opacity) });
    }
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

/// 보더/RoundedBox의 fill + border strips를 로컬 공간에서 emit
///
/// border_width는 절대(화면) 픽셀 → 내부에서 local 변환.
fn emit_border(
    vertices: &mut Vec<SlateVertex>,
    indices: &mut Vec<u32>,
    geo: &PaintGeometry,
    fill_color: [f32; 4],
    border_color: [f32; 4],
    border_width: f32,
) {
    let ls = geo.local_size();
    let local_bw = if geo.scale > 0.001 { border_width / geo.scale } else { border_width };

    // Fill rect
    emit_local_rect(vertices, indices, geo, fill_color,
        local_bw, local_bw, ls.x - 2.0 * local_bw, ls.y - 2.0 * local_bw);

    // Border strips (top, bottom, left, right)
    if border_width > 0.0 {
        for (lx, ly, lw, lh) in [
            (0.0, 0.0, ls.x, local_bw),
            (0.0, ls.y - local_bw, ls.x, local_bw),
            (0.0, local_bw, local_bw, ls.y - 2.0 * local_bw),
            (ls.x - local_bw, local_bw, local_bw, ls.y - 2.0 * local_bw),
        ] {
            emit_local_rect(vertices, indices, geo, border_color, lx, ly, lw, lh);
        }
    }
}

/// SDF RoundedBox 쿼드 emit
///
/// 4정점 쿼드를 emit하고, 프래그먼트 셰이더에서 SDF로 코너를 clip.
/// corner_radii: [TL, TR, BR, BL]
fn emit_rounded_box(
    vertices: &mut Vec<SlateRoundedVertex>,
    indices: &mut Vec<u32>,
    geo: &PaintGeometry,
    fill_color: [f32; 4],
    outline_color: [f32; 4],
    outline_width: f32,
    corner_radii: [f32; 4],
) {
    let opacity = geo.render_opacity();
    let fc = apply_render_opacity(fill_color, opacity);
    let oc = apply_render_opacity(outline_color, opacity);
    let base = vertices.len() as u32;

    let ls = geo.local_size();
    let w = ls.x;
    let h = ls.y;
    let rect_size = [w, h];

    // 4 corner local positions: TL, TR, BR, BL
    let local_positions: [[f32; 2]; 4] = [
        [0.0, 0.0],
        [w, 0.0],
        [w, h],
        [0.0, h],
    ];

    if let Some(rt) = geo.render_transform() {
        let p0 = rt.transform_point2(Vec2::ZERO);
        let p1 = rt.transform_point2(Vec2::new(ls.x, 0.0));
        let p2 = rt.transform_point2(ls);
        let p3 = rt.transform_point2(Vec2::new(0.0, ls.y));
        let positions = [[p0.x, p0.y], [p1.x, p1.y], [p2.x, p2.y], [p3.x, p3.y]];
        // RT 경로: rect_size는 local space이므로 radii도 local space로 클램프
        let half_min = w.min(h) * 0.5;
        let clamped_radii = [
            corner_radii[0].min(half_min),
            corner_radii[1].min(half_min),
            corner_radii[2].min(half_min),
            corner_radii[3].min(half_min),
        ];
        for i in 0..4 {
            vertices.push(SlateRoundedVertex {
                position: positions[i],
                local_pos: local_positions[i],
                color: fc,
                rect_size,
                corner_radii: clamped_radii,
                outline_color: oc,
                outline_width,
                _pad: 0.0,
            });
        }
    } else {
        let (x, y) = (geo.position.x, geo.position.y);
        let sw = geo.size.x;
        let sh = geo.size.y;
        let positions = [[x, y], [x + sw, y], [x + sw, y + sh], [x, y + sh]];
        // Scale local positions to match actual screen-space size
        let scale_x = if w > 0.0 { sw / w } else { 1.0 };
        let scale_y = if h > 0.0 { sh / h } else { 1.0 };
        let scaled_rect_size = [sw, sh];
        // corner_radii는 호출 측에서 이미 screen-space 값으로 전달됨.
        // rect_size도 screen-space이므로 radii를 다시 스케일하지 않는다.
        // 다만 min(half_w, half_h)로 클램프하여 SDF 안정성 보장.
        let half_min = sw.min(sh) * 0.5;
        let clamped_radii = [
            corner_radii[0].min(half_min),
            corner_radii[1].min(half_min),
            corner_radii[2].min(half_min),
            corner_radii[3].min(half_min),
        ];
        for i in 0..4 {
            vertices.push(SlateRoundedVertex {
                position: positions[i],
                local_pos: [local_positions[i][0] * scale_x, local_positions[i][1] * scale_y],
                color: fc,
                rect_size: scaled_rect_size,
                corner_radii: clamped_radii,
                outline_color: oc,
                outline_width: outline_width,
                _pad: 0.0,
            });
        }
    }

    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

// ============================================================================
// Pipeline Creation Helpers
// ============================================================================

/// 렌더 파이프라인 생성 헬퍼
fn create_render_pipeline(
    device: &wgpu::Device,
    label: &str,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    vertex_buffers: &[wgpu::VertexBufferLayout<'_>],
    format: wgpu::TextureFormat,
    color_writes: wgpu::ColorWrites,
    blend: Option<wgpu::BlendState>,
    depth_stencil: Option<wgpu::DepthStencilState>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: vertex_buffers,
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend,
                write_mask: color_writes,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

/// 스텐실 write 용 DepthStencilState (compare=Always, pass=Replace)
fn stencil_write_depth_stencil() -> wgpu::DepthStencilState {
    let face = wgpu::StencilFaceState {
        compare: wgpu::CompareFunction::Always,
        fail_op: wgpu::StencilOperation::Keep,
        depth_fail_op: wgpu::StencilOperation::Keep,
        pass_op: wgpu::StencilOperation::Replace,
    };
    wgpu::DepthStencilState {
        format: wgpu::TextureFormat::Depth24PlusStencil8,
        depth_write_enabled: false,
        depth_compare: wgpu::CompareFunction::Always,
        stencil: wgpu::StencilState {
            front: face,
            back: face,
            read_mask: 0xFF,
            write_mask: 0xFF,
        },
        bias: wgpu::DepthBiasState::default(),
    }
}

/// 스텐실 test 용 DepthStencilState (compare=Equal, write_mask=0)
fn stencil_test_depth_stencil() -> wgpu::DepthStencilState {
    let face = wgpu::StencilFaceState {
        compare: wgpu::CompareFunction::Equal,
        fail_op: wgpu::StencilOperation::Keep,
        depth_fail_op: wgpu::StencilOperation::Keep,
        pass_op: wgpu::StencilOperation::Keep,
    };
    wgpu::DepthStencilState {
        format: wgpu::TextureFormat::Depth24PlusStencil8,
        depth_write_enabled: false,
        depth_compare: wgpu::CompareFunction::Always,
        stencil: wgpu::StencilState {
            front: face,
            back: face,
            read_mask: 0xFF,
            write_mask: 0x00,
        },
        bias: wgpu::DepthBiasState::default(),
    }
}

/// 스텐실 passthrough 용 DepthStencilState (스텐실 패스 내 비스텐실 배치)
fn stencil_passthrough_depth_stencil() -> wgpu::DepthStencilState {
    let face = wgpu::StencilFaceState {
        compare: wgpu::CompareFunction::Always,
        fail_op: wgpu::StencilOperation::Keep,
        depth_fail_op: wgpu::StencilOperation::Keep,
        pass_op: wgpu::StencilOperation::Keep,
    };
    wgpu::DepthStencilState {
        format: wgpu::TextureFormat::Depth24PlusStencil8,
        depth_write_enabled: false,
        depth_compare: wgpu::CompareFunction::Always,
        stencil: wgpu::StencilState {
            front: face,
            back: face,
            read_mask: 0x00,
            write_mask: 0x00,
        },
        bias: wgpu::DepthBiasState::default(),
    }
}

// ============================================================================
// SlateRenderResources — 공유 렌더링 리소스 (1회 생성, 모든 윈도우 공유)
// UE5 FSlateRHIResourceManager에 대응
// ============================================================================

/// 공유 렌더링 리소스 (파이프라인, 텍스처 캐시, 텍스트 리소스)
///
/// 앱 초기화 시 1회 생성하고, 모든 윈도우의 RSlateRenderer가 공유합니다.
/// 멀티 윈도우 앱(SlateApp)에서 이 리소스를 공유하면
/// 윈도우 생성 비용이 ~2.7s에서 ~2ms로 줄어듭니다.
pub struct SlateRenderResources {
    /// UI 렌더 파이프라인
    pub(crate) pipeline: wgpu::RenderPipeline,
    /// SDF RoundedBox 파이프라인
    pub(crate) rounded_box_pipeline: wgpu::RenderPipeline,
    /// 텍스처 바인드 그룹 레이아웃
    pub(crate) texture_bind_group_layout: wgpu::BindGroupLayout,
    /// 유니폼 바인드 그룹 레이아웃 (RSlateRenderer viewport 생성 시 필요)
    pub(crate) uniform_bind_group_layout: wgpu::BindGroupLayout,
    /// 기본 흰색 텍스처
    pub(crate) white_texture: SlateTexture,
    /// 텍스처 캐시
    pub(crate) textures: HashMap<String, SlateTexture>,
    /// 에셋 기본 경로
    pub(crate) asset_base_path: String,
    /// 공유 텍스트 렌더링 리소스
    pub(crate) text: SharedTextResources,
    /// 스텐실 write 파이프라인 (Normal vertex, 색상 출력 없음)
    pub(crate) stencil_write_pipeline: wgpu::RenderPipeline,
    /// 스텐실 test 파이프라인 (Normal vertex, stencil Equal)
    pub(crate) stencil_test_pipeline: wgpu::RenderPipeline,
    /// 스텐실 test RoundedBox 파이프라인
    pub(crate) stencil_test_rounded_box_pipeline: wgpu::RenderPipeline,
    /// 스텐실 passthrough 파이프라인 (Normal vertex, 스텐실 패스 내 비스텐실 배치)
    pub(crate) stencil_passthrough_pipeline: wgpu::RenderPipeline,
    /// 스텐실 passthrough RoundedBox 파이프라인
    pub(crate) stencil_passthrough_rounded_box_pipeline: wgpu::RenderPipeline,
    /// 리소스 버전 카운터 (Feature 4: 텍스처 변형 시 증가)
    resource_version: u64,
}

impl SlateRenderResources {
    /// 공유 리소스 생성 (앱 초기화 시 1회)
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        font_data: Vec<u8>,
    ) -> Self {
        // 셰이더
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("RSlate Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/slate_shader.wgsl").into()),
        });

        // 텍스처 바인드 그룹 레이아웃
        let texture_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("RSlate Texture Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // 유니폼 바인드 그룹 레이아웃
        let uniform_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("RSlate Uniform Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // 파이프라인 레이아웃
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("RSlate Pipeline Layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, &texture_bind_group_layout],
            immediate_size: 0,
        });

        // 렌더 파이프라인
        let pipeline = create_render_pipeline(
            device, "RSlate Render Pipeline", &pipeline_layout,
            &shader, &[SlateVertex::desc()], format,
            wgpu::ColorWrites::ALL, Some(wgpu::BlendState::ALPHA_BLENDING), None,
        );

        // SDF RoundedBox 셰이더 + 파이프라인
        let rounded_box_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("RSlate RoundedBox Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/rounded_box_shader.wgsl").into()),
        });

        let rounded_box_pipeline = create_render_pipeline(
            device, "RSlate RoundedBox Pipeline", &pipeline_layout,
            &rounded_box_shader, &[SlateRoundedVertex::desc()], format,
            wgpu::ColorWrites::ALL, Some(wgpu::BlendState::ALPHA_BLENDING), None,
        );

        // 스텐실 파이프라인들
        let stencil_write_pipeline = create_render_pipeline(
            device, "RSlate Stencil Write Pipeline", &pipeline_layout,
            &shader, &[SlateVertex::desc()], format,
            wgpu::ColorWrites::empty(), None, Some(stencil_write_depth_stencil()),
        );
        let stencil_test_pipeline = create_render_pipeline(
            device, "RSlate Stencil Test Pipeline", &pipeline_layout,
            &shader, &[SlateVertex::desc()], format,
            wgpu::ColorWrites::ALL, Some(wgpu::BlendState::ALPHA_BLENDING),
            Some(stencil_test_depth_stencil()),
        );
        let stencil_test_rounded_box_pipeline = create_render_pipeline(
            device, "RSlate Stencil Test RoundedBox Pipeline", &pipeline_layout,
            &rounded_box_shader, &[SlateRoundedVertex::desc()], format,
            wgpu::ColorWrites::ALL, Some(wgpu::BlendState::ALPHA_BLENDING),
            Some(stencil_test_depth_stencil()),
        );
        let stencil_passthrough_pipeline = create_render_pipeline(
            device, "RSlate Stencil Passthrough Pipeline", &pipeline_layout,
            &shader, &[SlateVertex::desc()], format,
            wgpu::ColorWrites::ALL, Some(wgpu::BlendState::ALPHA_BLENDING),
            Some(stencil_passthrough_depth_stencil()),
        );
        let stencil_passthrough_rounded_box_pipeline = create_render_pipeline(
            device, "RSlate Stencil Passthrough RoundedBox Pipeline", &pipeline_layout,
            &rounded_box_shader, &[SlateRoundedVertex::desc()], format,
            wgpu::ColorWrites::ALL, Some(wgpu::BlendState::ALPHA_BLENDING),
            Some(stencil_passthrough_depth_stencil()),
        );

        // 기본 흰색 텍스처
        let white_texture = Self::create_white_texture(device, queue, &texture_bind_group_layout);

        // 공유 텍스트 리소스
        let mut font_chains = HashMap::new();
        if !font_data.is_empty() {
            font_chains.insert(FontFamily::UI, vec![font_data]);
        }
        let text = SharedTextResources::new(device, queue, format, font_chains);

        Self {
            pipeline,
            rounded_box_pipeline,
            texture_bind_group_layout,
            uniform_bind_group_layout,
            white_texture,
            textures: HashMap::new(),
            asset_base_path: String::new(),
            text,
            stencil_write_pipeline,
            stencil_test_pipeline,
            stencil_test_rounded_box_pipeline,
            stencil_passthrough_pipeline,
            stencil_passthrough_rounded_box_pipeline,
            resource_version: 0,
        }
    }

    fn create_white_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
    ) -> SlateTexture {
        let size = wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("RSlate White Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255u8, 255, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("RSlate White Texture Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        SlateTexture {
            texture,
            view,
            bind_group,
            size: (1, 1),
        }
    }

    /// 폰트 체인 설정 (패밀리별 폴백 체인)
    pub fn set_font_chain(&mut self, family: FontFamily, fonts: Vec<Vec<u8>>) {
        self.text.set_font_chain(family, fonts);
    }

    /// 에셋 기본 경로 설정
    pub fn set_asset_base_path(&mut self, path: impl Into<String>) {
        self.asset_base_path = path.into();
    }

    /// 텍스처 로드 (파일에서)
    pub fn load_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        name: &str,
    ) -> Result<(u32, u32), String> {
        if self.textures.contains_key(name) {
            let tex = self.textures.get(name).unwrap();
            return Ok(tex.size);
        }

        // 경로 결정
        let path = if Path::new(name).is_absolute() || self.asset_base_path.is_empty() {
            name.to_string()
        } else {
            format!("{}/{}", self.asset_base_path, name)
        };

        // 이미지 로드
        let img = image::open(&path)
            .map_err(|e| format!("Failed to load image '{}': {}", path, e))?;

        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        let data = rgba.into_raw();

        self.load_texture_from_data(device, queue, name, &data, width, height);
        // resource_version은 load_texture_from_data() 내부에서 이미 범프됨

        Ok((width, height))
    }

    /// 텍스처 로드 (데이터에서)
    pub fn load_texture_from_data(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        name: &str,
        data: &[u8],
        width: u32,
        height: u32,
    ) {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(name),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{} Bind Group", name)),
            layout: &self.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        self.textures.insert(name.to_string(), SlateTexture {
            texture,
            view,
            bind_group,
            size: (width, height),
        });
        self.resource_version += 1;
    }

    /// 텍스처 언로드
    pub fn unload_texture(&mut self, name: &str) {
        self.textures.remove(name);
        self.resource_version += 1;
    }

    /// 텍스처 크기 조회
    pub fn get_texture_size(&self, name: &str) -> Option<(u32, u32)> {
        self.textures.get(name).map(|t| t.size)
    }

    /// 외부 TextureView 등록 (ViewportTexture 등)
    pub fn register_external_texture(
        &mut self,
        device: &wgpu::Device,
        name: &str,
        texture_view: &wgpu::TextureView,
        size: (u32, u32),
    ) {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("{} External Bind Group", name)),
            layout: &self.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // 외부 텍스처는 texture 필드가 사용되지 않으므로 더미 생성
        let dummy_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Dummy External Texture"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let dummy_view = dummy_texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.textures.insert(name.to_string(), SlateTexture {
            texture: dummy_texture,
            view: dummy_view,
            bind_group,
            size,
        });

        self.resource_version += 1;
        log::debug!("[SlateRenderResources] Registered external texture '{}' ({}x{})", name, size.0, size.1);
    }

    /// 외부 TextureView 업데이트 (리사이즈 시)
    pub fn update_external_texture(
        &mut self,
        device: &wgpu::Device,
        name: &str,
        texture_view: &wgpu::TextureView,
        size: (u32, u32),
    ) {
        self.textures.remove(name);
        self.register_external_texture(device, name, texture_view, size);
    }

    /// 텍스처 바인드 그룹 레이아웃 참조 (외부에서 바인드 그룹 생성용)
    pub fn texture_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.texture_bind_group_layout
    }

    /// DrawElementList에서 참조하는 텍스처를 사전 로드 (lazy load)
    pub fn ensure_textures_loaded(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        draw_elements: &DrawElementList,
    ) {
        for (_layer, element) in &draw_elements.elements {
            match element {
                DrawElement::Image { path, .. } => {
                    if !path.is_empty() && !self.textures.contains_key(path.as_str()) {
                        if let Err(e) = self.load_texture(device, queue, path) {
                            log::debug!("[SlateRenderResources] Lazy load failed for '{}': {}", path, e);
                        }
                    }
                }
                DrawElement::Viewport { texture_name, .. } => {
                    if !texture_name.is_empty() && !self.textures.contains_key(texture_name.as_str()) {
                        log::debug!("[SlateRenderResources] Viewport texture '{}' not yet registered", texture_name);
                    }
                }
                _ => {}
            }
        }
    }

    /// 현재 리소스 버전 조회
    pub fn resource_version(&self) -> u64 {
        self.resource_version
    }

    /// 핫 리로드: 전체 텍스처 클리어 + 버전 범프
    pub fn invalidate_all_textures(&mut self) {
        self.textures.clear();
        self.resource_version += 1;
        log::info!("[SlateRenderResources] All textures invalidated (version={})", self.resource_version);
    }
}

// ============================================================================
// RSlateRenderer — 윈도우별 렌더러 (경량, ~2ms 생성)
// ============================================================================

/// RSlate 렌더러 (윈도우별)
///
/// 두 가지 모드로 동작:
/// - **Legacy** (`owned_resources` = Some): 공유 리소스를 내부 소유. `new()` 생성.
///   EditorUiState 등 단일 윈도우에서 사용.
/// - **Viewport** (`owned_resources` = None): 외부 `SlateRenderResources` 사용. `new_viewport()` 생성.
///   SlateApp의 멀티 윈도우에서 사용 (~2ms 생성).
pub struct RSlateRenderer {
    /// 소유 공유 리소스 (Some = legacy mode, None = viewport mode)
    owned_resources: Option<SlateRenderResources>,
    /// 유니폼 버퍼
    uniform_buffer: wgpu::Buffer,
    /// 유니폼 바인드 그룹
    uniform_bind_group: wgpu::BindGroup,
    /// 정점 버퍼
    vertex_buffer: wgpu::Buffer,
    /// 인덱스 버퍼
    index_buffer: wgpu::Buffer,
    /// 화면 크기
    screen_size: (f32, f32),
    /// 텍스트 뷰포트 (per-window text rendering state)
    text_viewport: TextViewport,
    /// 캐시된 DrawElementList (FastUpdate: idle 프레임 최적화)
    cached_draw_elements: DrawElementList,
    /// 캐시가 유효한지 (최소 1번 paint 완료)
    cache_valid: bool,
    /// 캐시된 정점 (CPU-side, 테셀레이션 결과 보존)
    cached_vertices: Vec<SlateVertex>,
    /// 캐시된 인덱스 (CPU-side, 테셀레이션 결과 보존)
    cached_indices: Vec<u32>,
    /// 캐시된 RoundedBox 정점
    cached_rounded_vertices: Vec<SlateRoundedVertex>,
    /// 캐시된 RoundedBox 인덱스
    cached_rounded_indices: Vec<u32>,
    /// RoundedBox 정점 버퍼
    rounded_vertex_buffer: wgpu::Buffer,
    /// RoundedBox 인덱스 버퍼
    rounded_index_buffer: wgpu::Buffer,
    /// 캐시된 배치 목록
    cached_batches: Vec<DrawBatch>,
    /// 캐시된 클리핑 상태 (tessellation 시 DrawElementList에서 복사)
    cached_clipping_states: Vec<SlateClippingState>,
    /// 캐시된 텍스트 정점 (text_renderer 독립적 보존)
    cached_text_vertices: Vec<SlateVertex>,
    /// 캐시된 텍스트 인덱스
    cached_text_indices: Vec<u32>,
    /// 테셀레이션 캐시 유효 여부
    tessellation_valid: bool,
    /// 통합 드로우 커맨드 리스트 (레이어 순서로 Geo·Text 인터리빙)
    cached_draw_commands: Vec<DrawCommand>,
    /// 2-phase 오버레이: Phase 2 시작 커맨드 인덱스 (None = 단일 phase)
    overlay_cmd_start: Option<usize>,
    /// 3-phase 드롭다운: Phase 3 시작 커맨드 인덱스 (None = 2-phase까지만)
    dropdown_cmd_start: Option<usize>,
    /// 스텐실 텍스처 (lazy 생성, 리사이즈 시 무효화)
    stencil_texture: Option<wgpu::Texture>,
    /// 스텐실 텍스처 뷰
    stencil_view: Option<wgpu::TextureView>,
    /// 스텐실 정점 버퍼
    stencil_vertex_buffer: wgpu::Buffer,
    /// 스텐실 인덱스 버퍼
    stencil_index_buffer: wgpu::Buffer,
    /// 캐시된 스텐실 write 엔트리
    cached_stencil_entries: Vec<StencilWriteEntry>,
    /// 캐시된 스텐실 정점
    cached_stencil_vertices: Vec<SlateVertex>,
    /// 캐시된 스텐실 인덱스
    cached_stencil_indices: Vec<u32>,
    /// 스텐실 참조값 스택 (StencilClipping 모듈 활용)
    stencil_ref_stack: crate::render::stencil_clipping::StencilRefStack,
}

impl RSlateRenderer {
    // ========================================================================
    // Constructors
    // ========================================================================

    /// 새 렌더러 생성 (legacy — 공유 리소스 내부 소유)
    ///
    /// 단일 윈도우 사용에 적합 (EditorUiState 등).
    /// 멀티 윈도우에서는 `SlateRenderResources::new()` + `new_viewport()` 사용.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        font_data: Vec<u8>,
    ) -> Self {
        let shared = SlateRenderResources::new(device, queue, format, font_data);
        let mut renderer = Self::new_viewport(device, &shared, width, height);
        renderer.owned_resources = Some(shared);
        renderer
    }

    /// 경량 뷰포트 생성 (공유 리소스 사용, ~2ms)
    ///
    /// SlateApp의 멀티 윈도우에서 사용.
    /// 렌더링 시 `render_with_shared()` / `render_elements_with_shared()`로 공유 리소스를 전달.
    pub fn new_viewport(
        device: &wgpu::Device,
        shared: &SlateRenderResources,
        width: u32,
        height: u32,
    ) -> Self {
        // 유니폼 버퍼
        let uniforms = SlateUniforms {
            screen_size: [width as f32, height as f32],
            _padding: [0.0, 0.0],
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("RSlate Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("RSlate Uniform Bind Group"),
            layout: &shared.uniform_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
            ],
        });

        // 정점/인덱스 버퍼
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("RSlate Vertex Buffer"),
            size: 1024 * 1024, // 1MB
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("RSlate Index Buffer"),
            size: 256 * 1024, // 256KB
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // RoundedBox 버퍼
        let rounded_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("RSlate RoundedBox Vertex Buffer"),
            size: 256 * 1024, // 256KB
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let rounded_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("RSlate RoundedBox Index Buffer"),
            size: 64 * 1024, // 64KB
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 텍스트 뷰포트
        let text_viewport = TextViewport::new(device, &shared.text, width, height);

        // 스텐실 버퍼
        let stencil_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("RSlate Stencil Vertex Buffer"),
            size: 64 * 1024, // 64KB
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let stencil_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("RSlate Stencil Index Buffer"),
            size: 16 * 1024, // 16KB
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            owned_resources: None,
            uniform_buffer,
            uniform_bind_group,
            vertex_buffer,
            index_buffer,
            screen_size: (width as f32, height as f32),
            text_viewport,
            cached_draw_elements: DrawElementList::new(),
            cache_valid: false,
            cached_vertices: Vec::new(),
            cached_indices: Vec::new(),
            cached_rounded_vertices: Vec::new(),
            cached_rounded_indices: Vec::new(),
            rounded_vertex_buffer,
            rounded_index_buffer,
            cached_batches: Vec::new(),
            cached_clipping_states: Vec::new(),
            cached_text_vertices: Vec::new(),
            cached_text_indices: Vec::new(),
            tessellation_valid: false,
            cached_draw_commands: Vec::new(),
            overlay_cmd_start: None,
            dropdown_cmd_start: None,
            stencil_texture: None,
            stencil_view: None,
            stencil_vertex_buffer,
            stencil_index_buffer,
            cached_stencil_entries: Vec::new(),
            cached_stencil_vertices: Vec::new(),
            cached_stencil_indices: Vec::new(),
            stencil_ref_stack: crate::render::stencil_clipping::StencilRefStack::new(),
        }
    }

    // ========================================================================
    // Backward-compat resource delegation (legacy mode only)
    // ========================================================================

    /// 내부 소유 리소스 참조 (legacy mode)
    fn resources(&self) -> &SlateRenderResources {
        self.owned_resources.as_ref()
            .expect("resources() called on viewport-mode renderer; use SlateRenderResources directly")
    }

    /// 내부 소유 리소스 가변 참조 (legacy mode)
    fn resources_mut(&mut self) -> &mut SlateRenderResources {
        self.owned_resources.as_mut()
            .expect("resources_mut() called on viewport-mode renderer; use SlateRenderResources directly")
    }

    /// 폰트 체인 설정 (패밀리별 폴백 체인)
    pub fn set_font_chain(&mut self, family: FontFamily, fonts: Vec<Vec<u8>>) {
        self.resources_mut().set_font_chain(family, fonts);
    }

    /// 에셋 기본 경로 설정
    pub fn set_asset_base_path(&mut self, path: impl Into<String>) {
        self.resources_mut().set_asset_base_path(path);
    }

    /// 텍스처 로드 (파일에서)
    pub fn load_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        name: &str,
    ) -> Result<(u32, u32), String> {
        self.resources_mut().load_texture(device, queue, name)
    }

    /// 텍스처 로드 (데이터에서)
    pub fn load_texture_from_data(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        name: &str,
        data: &[u8],
        width: u32,
        height: u32,
    ) {
        self.resources_mut().load_texture_from_data(device, queue, name, data, width, height);
    }

    /// 텍스처 언로드
    pub fn unload_texture(&mut self, name: &str) {
        self.resources_mut().unload_texture(name);
    }

    /// 텍스처 크기 조회
    pub fn get_texture_size(&self, name: &str) -> Option<(u32, u32)> {
        self.resources().get_texture_size(name)
    }

    /// 외부 TextureView 등록 (ViewportTexture 등)
    pub fn register_external_texture(
        &mut self,
        device: &wgpu::Device,
        name: &str,
        texture_view: &wgpu::TextureView,
        size: (u32, u32),
    ) {
        self.resources_mut().register_external_texture(device, name, texture_view, size);
    }

    /// 외부 TextureView 업데이트 (리사이즈 시)
    pub fn update_external_texture(
        &mut self,
        device: &wgpu::Device,
        name: &str,
        texture_view: &wgpu::TextureView,
        size: (u32, u32),
    ) {
        self.resources_mut().update_external_texture(device, name, texture_view, size);
    }

    /// 텍스처 바인드 그룹 레이아웃 참조 (외부에서 바인드 그룹 생성용)
    pub fn texture_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        self.resources().texture_bind_group_layout()
    }

    /// DrawElementList에서 참조하는 텍스처를 사전 로드 (lazy load)
    pub fn ensure_textures_loaded(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        draw_elements: &DrawElementList,
    ) {
        self.resources_mut().ensure_textures_loaded(device, queue, draw_elements);
    }

    // ========================================================================
    // Per-window methods
    // ========================================================================

    /// 현재 화면 크기 반환
    pub fn screen_size(&self) -> (f32, f32) {
        self.screen_size
    }

    /// 화면 크기 업데이트
    pub fn resize(&mut self, queue: &wgpu::Queue, width: u32, height: u32) {
        self.screen_size = (width as f32, height as f32);

        let uniforms = SlateUniforms {
            screen_size: [width as f32, height as f32],
            _padding: [0.0, 0.0],
        };

        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
        self.text_viewport.resize(width, height);
        // 리사이즈 시 캐시 무효화
        self.cache_valid = false;
        self.tessellation_valid = false;
        // 스텐실 텍스처 무효화 (lazy 재생성)
        self.stencil_texture = None;
        self.stencil_view = None;
    }

    /// DrawElementList 캐시 무효화 (외부 텍스처 변경 등)
    pub fn invalidate_cache(&mut self) {
        self.cache_valid = false;
        self.tessellation_valid = false;
    }

    /// 스텐실 텍스처 lazy 생성 (필요 시에만)
    fn ensure_stencil_texture(&mut self, device: &wgpu::Device) {
        if self.stencil_texture.is_some() {
            return;
        }
        let width = (self.screen_size.0 as u32).max(1);
        let height = (self.screen_size.1 as u32).max(1);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("RSlate Stencil Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24PlusStencil8,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.stencil_texture = Some(texture);
        self.stencil_view = Some(view);
    }

    // ========================================================================
    // Rendering — Legacy (backward compat, uses take/restore pattern)
    // ========================================================================

    /// 위젯 트리 렌더링 (legacy — 내부 소유 리소스 사용)
    ///
    /// 멀티 윈도우에서는 `render_with_shared()` 사용.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        root: &dyn Widget,
        scale: f32,
        current_time: f64,
        delta_time: f32,
    ) {
        let mut shared = self.owned_resources.take()
            .expect("render() requires owned resources; use render_with_shared() in viewport mode");
        self.render_with_shared(&mut shared, device, queue, encoder, view, root, scale, current_time, delta_time);
        self.owned_resources = Some(shared);
    }

    /// DrawElementList 직접 렌더링 (legacy — 내부 소유 리소스 사용)
    ///
    /// 멀티 윈도우에서는 `render_elements_with_shared()` 사용.
    pub fn render_elements(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        draw_elements: &DrawElementList,
    ) {
        let mut shared = self.owned_resources.take()
            .expect("render_elements() requires owned resources; use render_elements_with_shared() in viewport mode");
        self.render_elements_with_shared(&mut shared, device, queue, encoder, view, draw_elements);
        self.owned_resources = Some(shared);
    }

    // ========================================================================
    // Rendering — Shared (SlateApp multi-window)
    // ========================================================================

    /// 위젯 트리 렌더링 (공유 리소스 사용)
    pub fn render_with_shared(
        &mut self,
        shared: &mut SlateRenderResources,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        root: &dyn Widget,
        scale: f32,
        current_time: f64,
        delta_time: f32,
    ) {
        use crate::core::InvalidateWidgetReason;

        // FastUpdate: root가 clean이고 캐시가 유효하면 재paint 생략
        let root_dirty = root.dirty_flags();
        let needs_repaint = !self.cache_valid
            || root_dirty.contains(InvalidateWidgetReason::PAINT)
            || root_dirty.contains(InvalidateWidgetReason::LAYOUT)
            || root_dirty.contains(InvalidateWidgetReason::RENDER_TRANSFORM);

        if needs_repaint {
            // 루트 Geometry 생성
            let root_geometry = Geometry::make_root(
                glam::Vec2::new(self.screen_size.0, self.screen_size.1),
                scale,
            );

            // DrawElementList 수집 (캐시 갱신)
            self.cached_draw_elements.clear();
            let culling_rect = SlateRect::new(0.0, 0.0, self.screen_size.0, self.screen_size.1);
            let paint_args = PaintArgs {
                parent_enabled: true,
                current_time,
                delta_time,
                deferred_painting: false,
            };

            root.on_paint(&paint_args, &root_geometry, &culling_rect, &mut self.cached_draw_elements, 0, true);
            self.cache_valid = true;
            self.tessellation_valid = false;
        }

        // 테셀레이션 캐시: idle 프레임에서 vertex/index/batch 재생성 생략
        if !self.tessellation_valid {
            self.tessellate_elements(shared, queue);
            self.tessellation_valid = true;
        }

        // GPU 제출 (캐시된 vertices/indices/batches 사용)
        self.submit_render(shared, device, queue, encoder, view);
    }

    /// DrawElementList 직접 렌더링 (공유 리소스 사용)
    pub fn render_elements_with_shared(
        &mut self,
        shared: &mut SlateRenderResources,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        draw_elements: &DrawElementList,
    ) {
        // cached_* 에 테셀레이션 결과 저장 후 submit_render() 호출 (코드 중복 제거)
        self.cached_vertices.clear();
        self.cached_indices.clear();
        self.cached_rounded_vertices.clear();
        self.cached_rounded_indices.clear();
        self.cached_batches.clear();
        self.cached_draw_commands.clear();
        self.text_viewport.begin_frame();
        self.overlay_cmd_start = None;
        self.dropdown_cmd_start = None;

        let sorted_elements = draw_elements.sorted_with_clips();
        self.cached_clipping_states = draw_elements.clipping_manager().states().to_vec();

        let mut current_texture: Option<String> = None;
        let mut current_clip_idx: Option<usize> = None;
        let mut current_kind = BatchKind::Normal;
        #[allow(unused_assignments)]
        let mut batch_index_start: u32 = 0;
        #[allow(unused_assignments)]
        let mut rounded_batch_index_start: u32 = 0;

        macro_rules! flush_normal_batch {
            ($self:ident, $tex:ident, $clip:ident, $start:ident) => {{
                let index_count = $self.cached_indices.len() as u32 - $start;
                if index_count > 0 {
                    let batch_idx = $self.cached_batches.len();
                    $self.cached_batches.push(DrawBatch {
                        kind: BatchKind::Normal,
                        texture_name: $tex.take(),
                        clip_state_index: $clip,
                        index_start: $start,
                        index_count,
                    });
                    $self.cached_draw_commands.push(DrawCommand::GeometryBatch(batch_idx));
                    #[allow(unused_assignments)]
                    { $start = $self.cached_indices.len() as u32; }
                }
            }};
        }

        macro_rules! flush_rounded_batch {
            ($self:ident, $clip:ident, $start:ident) => {{
                let index_count = $self.cached_rounded_indices.len() as u32 - $start;
                if index_count > 0 {
                    let batch_idx = $self.cached_batches.len();
                    $self.cached_batches.push(DrawBatch {
                        kind: BatchKind::RoundedBox,
                        texture_name: None,
                        clip_state_index: $clip,
                        index_start: $start,
                        index_count,
                    });
                    $self.cached_draw_commands.push(DrawCommand::GeometryBatch(batch_idx));
                    #[allow(unused_assignments)]
                    { $start = $self.cached_rounded_indices.len() as u32; }
                }
            }};
        }

        for &(element, clip_state_index) in &sorted_elements {
            if clip_state_index != current_clip_idx {
                match current_kind {
                    BatchKind::Normal => flush_normal_batch!(self, current_texture, current_clip_idx, batch_index_start),
                    BatchKind::RoundedBox => flush_rounded_batch!(self, current_clip_idx, rounded_batch_index_start),
                }
                current_clip_idx = clip_state_index;
            }
            match element {
                DrawElement::Box { geometry, color } => {
                    if current_kind != BatchKind::Normal {
                        flush_rounded_batch!(self, current_clip_idx, rounded_batch_index_start);
                        current_kind = BatchKind::Normal;
                    } else if current_texture.is_some() {
                        flush_normal_batch!(self, current_texture, current_clip_idx, batch_index_start);
                        current_texture = None;
                    }
                    let c = [color.r, color.g, color.b, color.a];
                    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
                    emit_quad(&mut self.cached_vertices, &mut self.cached_indices, geometry, c, uvs);
                }
                DrawElement::Border { geometry, color, border_color, border_width } => {
                    if current_kind != BatchKind::Normal {
                        flush_rounded_batch!(self, current_clip_idx, rounded_batch_index_start);
                        current_kind = BatchKind::Normal;
                    } else if current_texture.is_some() {
                        flush_normal_batch!(self, current_texture, current_clip_idx, batch_index_start);
                        current_texture = None;
                    }
                    let c = [color.r, color.g, color.b, color.a];
                    let bc = [border_color.r, border_color.g, border_color.b, border_color.a];
                    emit_border(&mut self.cached_vertices, &mut self.cached_indices, geometry, c, bc, *border_width);
                }
                DrawElement::Text { geometry, text, color, font_size, font_family } => {
                    // 텍스트 전에 현재 지오메트리 배치 flush
                    match current_kind {
                        BatchKind::Normal => flush_normal_batch!(self, current_texture, current_clip_idx, batch_index_start),
                        BatchKind::RoundedBox => flush_rounded_batch!(self, current_clip_idx, rounded_batch_index_start),
                    }
                    current_kind = BatchKind::Normal;
                    current_texture = None;

                    let text_idx_before = self.text_viewport.indices_len() as u32;
                    let opacity = geometry.render_opacity();
                    let c = [color.r, color.g, color.b, color.a * opacity];
                    let (tx, ty) = if let Some(rt) = geometry.render_transform() {
                        let p = rt.transform_point2(Vec2::ZERO);
                        (p.x, p.y)
                    } else {
                        (geometry.position.x, geometry.position.y)
                    };
                    shared.text.add_text(&mut self.text_viewport, queue, text, tx, ty, *font_size, c, *font_family);
                    let text_idx_after = self.text_viewport.indices_len() as u32;
                    if text_idx_after > text_idx_before {
                        self.cached_draw_commands.push(DrawCommand::TextRange {
                            index_start: text_idx_before,
                            index_end: text_idx_after,
                            clip_state_index: clip_state_index,
                        });
                    }
                }
                DrawElement::StyledText { geometry, text, color, font_size, font_selector } => {
                    // 텍스트 전에 현재 지오메트리 배치 flush
                    match current_kind {
                        BatchKind::Normal => flush_normal_batch!(self, current_texture, current_clip_idx, batch_index_start),
                        BatchKind::RoundedBox => flush_rounded_batch!(self, current_clip_idx, rounded_batch_index_start),
                    }
                    current_kind = BatchKind::Normal;
                    current_texture = None;

                    let text_idx_before = self.text_viewport.indices_len() as u32;
                    let opacity = geometry.render_opacity();
                    let c = [color.r, color.g, color.b, color.a * opacity];
                    let (tx, ty) = if let Some(rt) = geometry.render_transform() {
                        let p = rt.transform_point2(Vec2::ZERO);
                        (p.x, p.y)
                    } else {
                        (geometry.position.x, geometry.position.y)
                    };
                    shared.text.add_text_with_selector(&mut self.text_viewport, queue, text, tx, ty, *font_size, c, *font_selector);
                    let text_idx_after = self.text_viewport.indices_len() as u32;
                    if text_idx_after > text_idx_before {
                        self.cached_draw_commands.push(DrawCommand::TextRange {
                            index_start: text_idx_before,
                            index_end: text_idx_after,
                            clip_state_index: clip_state_index,
                        });
                    }
                }
                DrawElement::Image { geometry, path, tint, scaling: _ } => {
                    if current_kind != BatchKind::Normal {
                        flush_rounded_batch!(self, current_clip_idx, rounded_batch_index_start);
                        current_kind = BatchKind::Normal;
                    }
                    let needs_new_batch = match &current_texture {
                        Some(current) => current != path,
                        None => true,
                    };
                    if needs_new_batch && !self.cached_indices.is_empty() {
                        flush_normal_batch!(self, current_texture, current_clip_idx, batch_index_start);
                    }
                    current_texture = Some(path.clone());
                    let c = [tint.r, tint.g, tint.b, tint.a];
                    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
                    emit_quad(&mut self.cached_vertices, &mut self.cached_indices, geometry, c, uvs);
                }
                DrawElement::Triangle { points, color } => {
                    if current_kind != BatchKind::Normal {
                        flush_rounded_batch!(self, current_clip_idx, rounded_batch_index_start);
                        current_kind = BatchKind::Normal;
                    } else if current_texture.is_some() {
                        flush_normal_batch!(self, current_texture, current_clip_idx, batch_index_start);
                        current_texture = None;
                    }
                    let c = [color.r, color.g, color.b, color.a];
                    let base_idx = self.cached_vertices.len() as u32;
                    for p in points {
                        self.cached_vertices.push(SlateVertex {
                            position: [p.x, p.y],
                            uv: [0.5, 0.5],
                            color: c,
                        });
                    }
                    self.cached_indices.extend_from_slice(&[base_idx, base_idx + 1, base_idx + 2]);
                }
                DrawElement::RoundedBox { geometry, fill_color, outline_color, outline_width, corner_radius } => {
                    if current_kind != BatchKind::RoundedBox {
                        flush_normal_batch!(self, current_texture, current_clip_idx, batch_index_start);
                        current_texture = None;
                        current_kind = BatchKind::RoundedBox;
                    }
                    let c = [fill_color.r, fill_color.g, fill_color.b, fill_color.a];
                    let oc = [outline_color.r, outline_color.g, outline_color.b, outline_color.a];
                    let cr = [corner_radius.top_left, corner_radius.top_right, corner_radius.bottom_right, corner_radius.bottom_left];
                    emit_rounded_box(&mut self.cached_rounded_vertices, &mut self.cached_rounded_indices, geometry, c, oc, *outline_width, cr);
                }
                DrawElement::Gradient { geometry, start_color, end_color, angle } => {
                    if current_kind != BatchKind::Normal {
                        flush_rounded_batch!(self, current_clip_idx, rounded_batch_index_start);
                        current_kind = BatchKind::Normal;
                    } else if current_texture.is_some() {
                        flush_normal_batch!(self, current_texture, current_clip_idx, batch_index_start);
                        current_texture = None;
                    }
                    let sc = [start_color.r, start_color.g, start_color.b, start_color.a];
                    let ec = [end_color.r, end_color.g, end_color.b, end_color.a];
                    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
                    if *angle >= 45.0 && *angle < 135.0 {
                        emit_quad_gradient(&mut self.cached_vertices, &mut self.cached_indices, geometry,
                            [sc, sc, ec, ec], uvs);
                    } else {
                        emit_quad_gradient(&mut self.cached_vertices, &mut self.cached_indices, geometry,
                            [sc, ec, ec, sc], uvs);
                    }
                }
                DrawElement::NineSlice { .. } => {}
                DrawElement::Brush { geometry, brush } => {
                    if current_kind != BatchKind::Normal {
                        flush_rounded_batch!(self, current_clip_idx, rounded_batch_index_start);
                        current_kind = BatchKind::Normal;
                    } else if current_texture.is_some() {
                        flush_normal_batch!(self, current_texture, current_clip_idx, batch_index_start);
                        current_texture = None;
                    }
                    let tint = brush.get_tint();
                    let c = [tint.r, tint.g, tint.b, tint.a];
                    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
                    emit_quad(&mut self.cached_vertices, &mut self.cached_indices, geometry, c, uvs);
                }
                DrawElement::Viewport { geometry, texture_name, tint } => {
                    if current_kind != BatchKind::Normal {
                        flush_rounded_batch!(self, current_clip_idx, rounded_batch_index_start);
                        current_kind = BatchKind::Normal;
                    }
                    let needs_new_batch = match &current_texture {
                        Some(current) => current != texture_name,
                        None => true,
                    };
                    if needs_new_batch && !self.cached_indices.is_empty() {
                        flush_normal_batch!(self, current_texture, current_clip_idx, batch_index_start);
                    }
                    current_texture = Some(texture_name.clone());
                    let c = [tint.r, tint.g, tint.b, tint.a];
                    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
                    emit_quad(&mut self.cached_vertices, &mut self.cached_indices, geometry, c, uvs);
                }
                DrawElement::Spline { .. } => {}
                DrawElement::CustomVerts { .. } => {}
                DrawElement::PostProcess { .. } => {}
            }
        }

        // 마지막 배치 저장
        match current_kind {
            BatchKind::Normal => flush_normal_batch!(self, current_texture, current_clip_idx, batch_index_start),
            BatchKind::RoundedBox => flush_rounded_batch!(self, current_clip_idx, rounded_batch_index_start),
        }

        // 스텐실 테셀레이션
        self.tessellate_stencil_quads();

        // ElementBatcher 통계 검증 (shadow 비교)
        #[cfg(feature = "slate_debugging")]
        {
            use crate::render::element_batcher::{ElementBatcher, BatchKey};
            let mut batcher = ElementBatcher::new();
            for (idx, &(ref element, clip_state)) in sorted_elements.iter().enumerate() {
                let key = BatchKey::from_draw_element(element, clip_state.unwrap_or(0) as u32);
                batcher.add_element(key, idx, 4, 6);
            }
            batcher.finish();
            log::trace!("[Batcher] inline_batches={} batcher_batches={}", self.cached_batches.len(), batcher.batch_count());
        }

        // 진단 로깅 (viewport mode — 2차 윈도우)
        if self.owned_resources.is_none() {
            log::info!("[RenderElements] sorted={}, vertices={}, indices={}, rounded_vertices={}, batches={}, cmds={}, screen={}x{}",
                sorted_elements.len(), self.cached_vertices.len(), self.cached_indices.len(),
                self.cached_rounded_vertices.len(), self.cached_batches.len(), self.cached_draw_commands.len(),
                self.screen_size.0, self.screen_size.1);
            for (bi, b) in self.cached_batches.iter().enumerate().take(5) {
                log::info!("[RenderElements] batch[{}]: kind={:?}, tex={:?}, clip={:?}, idx={}..+{}",
                    bi, b.kind, b.texture_name, b.clip_state_index, b.index_start, b.index_count);
            }
            if !self.cached_vertices.is_empty() {
                let v = &self.cached_vertices[0];
                log::info!("[RenderElements] v0: pos=[{:.1},{:.1}] color=[{:.2},{:.2},{:.2},{:.2}]",
                    v.position[0], v.position[1], v.color[0], v.color[1], v.color[2], v.color[3]);
            }
            if self.cached_vertices.is_empty() && self.cached_rounded_vertices.is_empty() {
                log::info!("[RenderElements] WARNING: zero vertices, only clear color will show");
            }
        }

        // 텍스트 데이터 스냅샷
        self.cached_text_vertices.clear();
        self.cached_text_vertices.extend_from_slice(self.text_viewport.text_vertices());
        self.cached_text_indices.clear();
        self.cached_text_indices.extend_from_slice(self.text_viewport.text_indices());

        // GPU 제출 (submit_render 재활용 — 스텐실 로직 포함)
        self.submit_render(shared, device, queue, encoder, view);
    }

    // ========================================================================
    // GT/RT 분리용 공개 메서드 (Step 3)
    // ========================================================================

    /// GT 전용: 위젯 트리 → DrawElementList 수집 (순수 CPU, GPU 의존 없음)
    ///
    /// `on_paint()`만 실행. tessellation/GPU submit 없음.
    /// 결과를 RT로 전송하여 `tessellate_and_submit()`으로 렌더링.
    pub fn paint_to_draw_list(
        &mut self,
        root: &dyn Widget,
        scale: f32,
        current_time: f64,
        delta_time: f32,
    ) -> &DrawElementList {
        use crate::core::InvalidateWidgetReason;

        let root_dirty = root.dirty_flags();
        let needs_repaint = !self.cache_valid
            || root_dirty.contains(InvalidateWidgetReason::PAINT)
            || root_dirty.contains(InvalidateWidgetReason::LAYOUT)
            || root_dirty.contains(InvalidateWidgetReason::RENDER_TRANSFORM);

        if needs_repaint {
            let root_geometry = Geometry::make_root(
                glam::Vec2::new(self.screen_size.0, self.screen_size.1),
                scale,
            );

            self.cached_draw_elements.clear();
            let culling_rect = SlateRect::new(0.0, 0.0, self.screen_size.0, self.screen_size.1);
            let paint_args = PaintArgs {
                parent_enabled: true,
                current_time,
                delta_time,
                deferred_painting: false,
            };

            root.on_paint(&paint_args, &root_geometry, &culling_rect, &mut self.cached_draw_elements, 0, true);
            self.cache_valid = true;
            self.tessellation_valid = false;
        }

        &self.cached_draw_elements
    }

    /// RT 전용: tessellation + GPU submit (DrawElementList가 이미 캐시에 있다고 가정)
    ///
    /// `paint_to_draw_list()` 또는 `set_draw_elements()` 후 호출.
    pub fn tessellate_and_submit(
        &mut self,
        shared: &mut SlateRenderResources,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
    ) {
        if !self.tessellation_valid {
            self.tessellate_elements(shared, queue);
            self.tessellation_valid = true;
        }
        self.submit_render(shared, device, queue, encoder, view);
    }

    /// RT 전용: 외부 DrawElementList로 캐시 교체 (GT에서 전송받은 데이터)
    pub fn set_draw_elements(&mut self, draw_elements: DrawElementList) {
        self.cached_draw_elements = draw_elements;
        self.cache_valid = true;
        self.tessellation_valid = false;
    }

    // ========================================================================
    // Internal: tessellation + GPU submit
    // ========================================================================

    /// 캐시된 DrawElementList → 정점/인덱스/배치 테셀레이션
    fn tessellate_elements(&mut self, shared: &mut SlateRenderResources, queue: &wgpu::Queue) {
        self.cached_vertices.clear();
        self.cached_indices.clear();
        self.cached_rounded_vertices.clear();
        self.cached_rounded_indices.clear();
        self.cached_batches.clear();
        self.cached_draw_commands.clear();
        self.text_viewport.begin_frame();
        self.overlay_cmd_start = None;
        self.dropdown_cmd_start = None;

        self.cached_draw_elements.ensure_sorted();
        let overlay_layer = self.cached_draw_elements.overlay_layer();
        let dropdown_layer = self.cached_draw_elements.dropdown_layer();

        // 클리핑 상태 캐시 (submit_render에서 사용)
        self.cached_clipping_states = self.cached_draw_elements
            .clipping_manager().states().to_vec();

        let mut current_texture: Option<String> = None;
        let mut current_clip_idx: Option<usize> = None;
        let mut current_kind = BatchKind::Normal;
        #[allow(unused_assignments)]
        let mut batch_index_start: u32 = 0;
        #[allow(unused_assignments)]
        let mut rounded_batch_index_start: u32 = 0;

        // Helper macros for batch flushing — also push DrawCommand::GeometryBatch
        macro_rules! flush_normal {
            ($self:ident, $tex:ident, $clip:ident, $start:ident) => {{
                let index_count = $self.cached_indices.len() as u32 - $start;
                if index_count > 0 {
                    let batch_idx = $self.cached_batches.len();
                    $self.cached_batches.push(DrawBatch {
                        kind: BatchKind::Normal,
                        texture_name: $tex.take(),
                        clip_state_index: $clip,
                        index_start: $start,
                        index_count,
                    });
                    $self.cached_draw_commands.push(DrawCommand::GeometryBatch(batch_idx));
                    #[allow(unused_assignments)]
                    { $start = $self.cached_indices.len() as u32; }
                }
            }};
        }

        macro_rules! flush_rounded {
            ($self:ident, $clip:ident, $start:ident) => {{
                let index_count = $self.cached_rounded_indices.len() as u32 - $start;
                if index_count > 0 {
                    let batch_idx = $self.cached_batches.len();
                    $self.cached_batches.push(DrawBatch {
                        kind: BatchKind::RoundedBox,
                        texture_name: None,
                        clip_state_index: $clip,
                        index_start: $start,
                        index_count,
                    });
                    $self.cached_draw_commands.push(DrawCommand::GeometryBatch(batch_idx));
                    #[allow(unused_assignments)]
                    { $start = $self.cached_rounded_indices.len() as u32; }
                }
            }};
        }

        let mut overlay_boundary_set = false;
        let mut dropdown_boundary_set = false;

        for (layer, element, clip_state_index) in self.cached_draw_elements.sorted_iter_with_layer() {
            // 2-phase 오버레이: overlay_layer 경계 감지
            if !overlay_boundary_set {
                if let Some(ol) = overlay_layer {
                    if layer >= ol {
                        // Phase 1 → Phase 2 경계: 현재 배치 flush 후 경계 기록
                        match current_kind {
                            BatchKind::Normal => flush_normal!(self, current_texture, current_clip_idx, batch_index_start),
                            BatchKind::RoundedBox => flush_rounded!(self, current_clip_idx, rounded_batch_index_start),
                        }
                        self.overlay_cmd_start = Some(self.cached_draw_commands.len());
                        overlay_boundary_set = true;
                    }
                }
            }

            // 3-phase 드롭다운: dropdown_layer 경계 감지
            if overlay_boundary_set && !dropdown_boundary_set {
                if let Some(dl) = dropdown_layer {
                    if layer >= dl {
                        // Phase 2 → Phase 3 경계: 현재 배치 flush 후 경계 기록
                        match current_kind {
                            BatchKind::Normal => flush_normal!(self, current_texture, current_clip_idx, batch_index_start),
                            BatchKind::RoundedBox => flush_rounded!(self, current_clip_idx, rounded_batch_index_start),
                        }
                        self.dropdown_cmd_start = Some(self.cached_draw_commands.len());
                        dropdown_boundary_set = true;
                    }
                }
            }

            // 클립 변경 체크
            if clip_state_index != current_clip_idx {
                match current_kind {
                    BatchKind::Normal => flush_normal!(self, current_texture, current_clip_idx, batch_index_start),
                    BatchKind::RoundedBox => flush_rounded!(self, current_clip_idx, rounded_batch_index_start),
                }
                current_clip_idx = clip_state_index;
            }
            match element {
                DrawElement::Box { geometry, color } => {
                    if current_kind != BatchKind::Normal {
                        flush_rounded!(self, current_clip_idx, rounded_batch_index_start);
                        current_kind = BatchKind::Normal;
                    } else if current_texture.is_some() {
                        flush_normal!(self, current_texture, current_clip_idx, batch_index_start);
                        current_texture = None;
                    }

                    let c = [color.r, color.g, color.b, color.a];
                    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
                    emit_quad(&mut self.cached_vertices, &mut self.cached_indices, geometry, c, uvs);
                }
                DrawElement::Border { geometry, color, border_color, border_width } => {
                    if current_kind != BatchKind::Normal {
                        flush_rounded!(self, current_clip_idx, rounded_batch_index_start);
                        current_kind = BatchKind::Normal;
                    } else if current_texture.is_some() {
                        flush_normal!(self, current_texture, current_clip_idx, batch_index_start);
                        current_texture = None;
                    }

                    let c = [color.r, color.g, color.b, color.a];
                    let bc = [border_color.r, border_color.g, border_color.b, border_color.a];
                    emit_border(&mut self.cached_vertices, &mut self.cached_indices, geometry, c, bc, *border_width);
                }
                DrawElement::Text { geometry, text, color, font_size, font_family } => {
                    // 텍스트 전에 현재 지오메트리 배치 flush → DrawCommand 인터리빙
                    match current_kind {
                        BatchKind::Normal => flush_normal!(self, current_texture, current_clip_idx, batch_index_start),
                        BatchKind::RoundedBox => flush_rounded!(self, current_clip_idx, rounded_batch_index_start),
                    }
                    current_kind = BatchKind::Normal;
                    current_texture = None;

                    let text_idx_before = self.text_viewport.indices_len() as u32;
                    let opacity = geometry.render_opacity();
                    let c = [color.r, color.g, color.b, color.a * opacity];
                    let (tx, ty) = if let Some(rt) = geometry.render_transform() {
                        let p = rt.transform_point2(Vec2::ZERO);
                        (p.x, p.y)
                    } else {
                        (geometry.position.x, geometry.position.y)
                    };
                    shared.text.add_text(&mut self.text_viewport, queue, text, tx, ty, *font_size, c, *font_family);
                    let text_idx_after = self.text_viewport.indices_len() as u32;
                    if text_idx_after > text_idx_before {
                        self.cached_draw_commands.push(DrawCommand::TextRange {
                            index_start: text_idx_before,
                            index_end: text_idx_after,
                            clip_state_index,
                        });
                    }
                }
                DrawElement::StyledText { geometry, text, color, font_size, font_selector } => {
                    // 텍스트 전에 현재 지오메트리 배치 flush → DrawCommand 인터리빙
                    match current_kind {
                        BatchKind::Normal => flush_normal!(self, current_texture, current_clip_idx, batch_index_start),
                        BatchKind::RoundedBox => flush_rounded!(self, current_clip_idx, rounded_batch_index_start),
                    }
                    current_kind = BatchKind::Normal;
                    current_texture = None;

                    let text_idx_before = self.text_viewport.indices_len() as u32;
                    let opacity = geometry.render_opacity();
                    let c = [color.r, color.g, color.b, color.a * opacity];
                    let (tx, ty) = if let Some(rt) = geometry.render_transform() {
                        let p = rt.transform_point2(Vec2::ZERO);
                        (p.x, p.y)
                    } else {
                        (geometry.position.x, geometry.position.y)
                    };
                    shared.text.add_text_with_selector(&mut self.text_viewport, queue, text, tx, ty, *font_size, c, *font_selector);
                    let text_idx_after = self.text_viewport.indices_len() as u32;
                    if text_idx_after > text_idx_before {
                        self.cached_draw_commands.push(DrawCommand::TextRange {
                            index_start: text_idx_before,
                            index_end: text_idx_after,
                            clip_state_index,
                        });
                    }
                }
                DrawElement::Image { geometry, path, tint, scaling: _ } => {
                    if current_kind != BatchKind::Normal {
                        flush_rounded!(self, current_clip_idx, rounded_batch_index_start);
                        current_kind = BatchKind::Normal;
                    }
                    let needs_new_batch = match &current_texture {
                        Some(current) => current != path,
                        None => true,
                    };

                    if needs_new_batch && !self.cached_indices.is_empty() {
                        flush_normal!(self, current_texture, current_clip_idx, batch_index_start);
                    }
                    current_texture = Some(path.clone());

                    let c = [tint.r, tint.g, tint.b, tint.a];
                    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
                    emit_quad(&mut self.cached_vertices, &mut self.cached_indices, geometry, c, uvs);
                }
                DrawElement::Triangle { points, color } => {
                    if current_kind != BatchKind::Normal {
                        flush_rounded!(self, current_clip_idx, rounded_batch_index_start);
                        current_kind = BatchKind::Normal;
                    } else if current_texture.is_some() {
                        flush_normal!(self, current_texture, current_clip_idx, batch_index_start);
                        current_texture = None;
                    }

                    let opacity = 1.0_f32;
                    let c = [color.r, color.g, color.b, color.a * opacity];
                    let base_idx = self.cached_vertices.len() as u32;
                    for p in points {
                        self.cached_vertices.push(SlateVertex {
                            position: [p.x, p.y],
                            uv: [0.5, 0.5],
                            color: c,
                        });
                    }
                    self.cached_indices.extend_from_slice(&[base_idx, base_idx + 1, base_idx + 2]);
                }
                DrawElement::RoundedBox { geometry, fill_color, outline_color, outline_width, corner_radius } => {
                    if current_kind != BatchKind::RoundedBox {
                        flush_normal!(self, current_texture, current_clip_idx, batch_index_start);
                        current_texture = None;
                        current_kind = BatchKind::RoundedBox;
                    }

                    let c = [fill_color.r, fill_color.g, fill_color.b, fill_color.a];
                    let oc = [outline_color.r, outline_color.g, outline_color.b, outline_color.a];
                    let cr = [corner_radius.top_left, corner_radius.top_right, corner_radius.bottom_right, corner_radius.bottom_left];
                    emit_rounded_box(&mut self.cached_rounded_vertices, &mut self.cached_rounded_indices, geometry, c, oc, *outline_width, cr);
                }
                DrawElement::Gradient { geometry, start_color, end_color, angle } => {
                    if current_kind != BatchKind::Normal {
                        flush_rounded!(self, current_clip_idx, rounded_batch_index_start);
                        current_kind = BatchKind::Normal;
                    } else if current_texture.is_some() {
                        flush_normal!(self, current_texture, current_clip_idx, batch_index_start);
                        current_texture = None;
                    }

                    let sc = [start_color.r, start_color.g, start_color.b, start_color.a];
                    let ec = [end_color.r, end_color.g, end_color.b, end_color.a];
                    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
                    if *angle >= 45.0 && *angle < 135.0 {
                        emit_quad_gradient(&mut self.cached_vertices, &mut self.cached_indices, geometry,
                            [sc, sc, ec, ec], uvs);
                    } else {
                        emit_quad_gradient(&mut self.cached_vertices, &mut self.cached_indices, geometry,
                            [sc, ec, ec, sc], uvs);
                    }
                }
                DrawElement::NineSlice { .. } => {
                    // TODO Phase 2.2: 9-Slice 텍스처 렌더링 구현
                }
                DrawElement::Brush { geometry, brush } => {
                    if current_kind != BatchKind::Normal {
                        flush_rounded!(self, current_clip_idx, rounded_batch_index_start);
                        current_kind = BatchKind::Normal;
                    } else if current_texture.is_some() {
                        flush_normal!(self, current_texture, current_clip_idx, batch_index_start);
                        current_texture = None;
                    }

                    let tint = brush.get_tint();
                    let c = [tint.r, tint.g, tint.b, tint.a];
                    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
                    emit_quad(&mut self.cached_vertices, &mut self.cached_indices, geometry, c, uvs);
                }
                DrawElement::Viewport { geometry, texture_name, tint } => {
                    if current_kind != BatchKind::Normal {
                        flush_rounded!(self, current_clip_idx, rounded_batch_index_start);
                        current_kind = BatchKind::Normal;
                    }
                    let needs_new_batch = match &current_texture {
                        Some(current) => current != texture_name,
                        None => true,
                    };

                    if needs_new_batch && !self.cached_indices.is_empty() {
                        flush_normal!(self, current_texture, current_clip_idx, batch_index_start);
                    }
                    current_texture = Some(texture_name.clone());

                    let c = [tint.r, tint.g, tint.b, tint.a];
                    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
                    emit_quad(&mut self.cached_vertices, &mut self.cached_indices, geometry, c, uvs);
                }
                DrawElement::Spline { .. } => {
                    // TODO: 스플라인 테셀레이션 → 삼각형 스트립
                }
                DrawElement::CustomVerts { .. } => {
                    // TODO: 커스텀 정점 직접 추가
                }
                DrawElement::PostProcess { .. } => {
                    // TODO: 후처리 패스 스케줄링
                }
            }
        }

        // 마지막 배치 저장
        match current_kind {
            BatchKind::Normal => flush_normal!(self, current_texture, current_clip_idx, batch_index_start),
            BatchKind::RoundedBox => flush_rounded!(self, current_clip_idx, rounded_batch_index_start),
        }

        // 커맨드 병합 (인접한 동일 종류 커맨드 합침, Phase 경계 보존)
        if self.cached_draw_commands.len() > 1 {
            let ocs = self.overlay_cmd_start;
            let dcs = self.dropdown_cmd_start;
            let mut new_ocs: Option<usize> = ocs.filter(|&o| o == 0).map(|_| 0);
            let mut new_dcs: Option<usize> = dcs.filter(|&d| d == 0).map(|_| 0);

            let mut write = 0;
            for read in 1..self.cached_draw_commands.len() {
                if ocs == Some(read) && new_ocs.is_none() {
                    new_ocs = Some(write + 1);
                }
                if dcs == Some(read) && new_dcs.is_none() {
                    new_dcs = Some(write + 1);
                }
                let at_boundary = ocs == Some(read) || dcs == Some(read);

                let can_merge = !at_boundary && match (&self.cached_draw_commands[write], &self.cached_draw_commands[read]) {
                    (DrawCommand::GeometryBatch(wi), DrawCommand::GeometryBatch(ri)) => {
                        let wb = &self.cached_batches[*wi];
                        let rb = &self.cached_batches[*ri];
                        wb.kind == rb.kind
                            && wb.texture_name == rb.texture_name
                            && wb.clip_state_index == rb.clip_state_index
                            && wb.index_start + wb.index_count == rb.index_start
                    }
                    (DrawCommand::TextRange { index_end: we, clip_state_index: wc, .. },
                     DrawCommand::TextRange { index_start: rs, clip_state_index: rc, .. }) => {
                        wc == rc && *we == *rs
                    }
                    _ => false,
                };

                if can_merge {
                    // 먼저 read 측 값을 복사해둬서 borrow 충돌 방지
                    let read_cmd = self.cached_draw_commands[read].clone();
                    match (&mut self.cached_draw_commands[write], &read_cmd) {
                        (DrawCommand::GeometryBatch(wi), DrawCommand::GeometryBatch(ri)) => {
                            self.cached_batches[*wi].index_count += self.cached_batches[*ri].index_count;
                        }
                        (DrawCommand::TextRange { index_end, .. }, DrawCommand::TextRange { index_end: re, .. }) => {
                            *index_end = *re;
                        }
                        _ => unreachable!(),
                    }
                } else {
                    write += 1;
                    if write != read {
                        self.cached_draw_commands.swap(write, read);
                    }
                }
            }
            self.cached_draw_commands.truncate(write + 1);
            let final_len = write + 1;

            if ocs.is_some() {
                self.overlay_cmd_start = Some(new_ocs.unwrap_or(final_len));
            }
            if dcs.is_some() {
                self.dropdown_cmd_start = Some(new_dcs.unwrap_or(final_len));
            }
        }

        // 텍스트 데이터 스냅샷 (submit_render에서 사용)
        self.cached_text_vertices.clear();
        self.cached_text_vertices.extend_from_slice(self.text_viewport.text_vertices());
        self.cached_text_indices.clear();
        self.cached_text_indices.extend_from_slice(self.text_viewport.text_indices());

        // 스텐실 쿼드 테셀레이션
        self.tessellate_stencil_quads();
    }

    /// 캐시된 클리핑 상태에서 스텐실 쿼드 테셀레이션 (내부 헬퍼)
    fn tessellate_stencil_quads(&mut self) {
        use crate::core::EClippingMethod;

        self.cached_stencil_entries.clear();
        self.cached_stencil_vertices.clear();
        self.cached_stencil_indices.clear();

        // 스텐실 상태가 있는지 확인
        let has_stencil = self.cached_clipping_states.iter()
            .any(|s| s.clipping_method() == EClippingMethod::Stencil);
        if !has_stencil {
            return;
        }

        let screen_w = self.screen_size.0;
        let screen_h = self.screen_size.1;

        // 풀스크린 클리어 쿼드 (index 0..5, stencil_ref=0 → Clear)
        let base = self.cached_stencil_vertices.len() as u32;
        let white = [1.0f32, 1.0, 1.0, 0.0]; // 투명 — 색상 출력 없음
        self.cached_stencil_vertices.push(SlateVertex { position: [0.0, 0.0], uv: [0.0, 0.0], color: white });
        self.cached_stencil_vertices.push(SlateVertex { position: [screen_w, 0.0], uv: [1.0, 0.0], color: white });
        self.cached_stencil_vertices.push(SlateVertex { position: [screen_w, screen_h], uv: [1.0, 1.0], color: white });
        self.cached_stencil_vertices.push(SlateVertex { position: [0.0, screen_h], uv: [0.0, 1.0], color: white });
        self.cached_stencil_indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);

        // 각 스텐실 메서드 클리핑 상태별 쿼드 emit
        self.stencil_ref_stack.clear();
        for (state_idx, state) in self.cached_clipping_states.iter().enumerate() {
            if state.clipping_method() != EClippingMethod::Stencil {
                continue;
            }
            let stencil_ref = self.stencil_ref_stack.push().unwrap_or(0) as u32;

            let entry_index_start = self.cached_stencil_indices.len() as u32;

            for quad in &state.stencil_quads {
                let qbase = self.cached_stencil_vertices.len() as u32;
                self.cached_stencil_vertices.push(SlateVertex {
                    position: [quad.top_left.x, quad.top_left.y], uv: [0.0, 0.0], color: white,
                });
                self.cached_stencil_vertices.push(SlateVertex {
                    position: [quad.top_right.x, quad.top_right.y], uv: [1.0, 0.0], color: white,
                });
                self.cached_stencil_vertices.push(SlateVertex {
                    position: [quad.bottom_right.x, quad.bottom_right.y], uv: [1.0, 1.0], color: white,
                });
                self.cached_stencil_vertices.push(SlateVertex {
                    position: [quad.bottom_left.x, quad.bottom_left.y], uv: [0.0, 1.0], color: white,
                });
                self.cached_stencil_indices.extend_from_slice(&[
                    qbase, qbase + 1, qbase + 2,
                    qbase, qbase + 2, qbase + 3,
                ]);
            }

            let entry_index_count = self.cached_stencil_indices.len() as u32 - entry_index_start;
            if entry_index_count > 0 {
                self.cached_stencil_entries.push(StencilWriteEntry {
                    clip_state_index: state_idx,
                    stencil_ref,
                    index_start: entry_index_start,
                    index_count: entry_index_count,
                });
            }
        }
    }

    /// 커맨드 범위 내 스텐실 상태 존재 확인
    fn phase_needs_stencil(&self, cmd_start: usize, cmd_end: usize) -> bool {
        use crate::core::EClippingMethod;
        for cmd in &self.cached_draw_commands[cmd_start..cmd_end] {
            let clip_idx = match cmd {
                DrawCommand::GeometryBatch(bi) => self.cached_batches[*bi].clip_state_index,
                DrawCommand::TextRange { clip_state_index, .. } => *clip_state_index,
            };
            if let Some(ci) = clip_idx {
                if let Some(state) = self.cached_clipping_states.get(ci) {
                    if state.clipping_method() == EClippingMethod::Stencil {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// 캐시된 정점/인덱스/배치 + 텍스트를 GPU에 제출 (내부용)
    ///
    /// UE5 FSlateElementBatcher 패턴: 지오메트리와 텍스트를 DrawCommand 리스트 순서로
    /// 인터리빙하여 같은 렌더 패스 내에서 파이프라인 스위칭으로 처리.
    fn submit_render(
        &mut self,
        shared: &SlateRenderResources,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
    ) {
        if self.cached_draw_commands.is_empty() {
            return;
        }

        // GPU 버퍼 업로드 (지오메트리 + 텍스트 — 한 번만)
        if !self.cached_vertices.is_empty() {
            queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&self.cached_vertices));
            queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&self.cached_indices));
        }
        if !self.cached_rounded_vertices.is_empty() {
            queue.write_buffer(&self.rounded_vertex_buffer, 0, bytemuck::cast_slice(&self.cached_rounded_vertices));
            queue.write_buffer(&self.rounded_index_buffer, 0, bytemuck::cast_slice(&self.cached_rounded_indices));
        }

        // 텍스트 버퍼 업로드 (한 번만 — 기존 Phase별 3회 → 1회로 감소)
        let has_text = !self.cached_text_vertices.is_empty();
        if has_text {
            // TextUniforms: [screen_size: f32x2, _padding: f32x2]
            let text_uniforms: [f32; 4] = [
                self.text_viewport.screen_size.0, self.text_viewport.screen_size.1, 0.0, 0.0,
            ];
            queue.write_buffer(
                &self.text_viewport.uniform_buffer, 0,
                bytemuck::cast_slice(&text_uniforms),
            );
            queue.write_buffer(&self.text_viewport.vertex_buffer, 0, bytemuck::cast_slice(&self.cached_text_vertices));
            queue.write_buffer(&self.text_viewport.index_buffer, 0, bytemuck::cast_slice(&self.cached_text_indices));
        }

        // Phase별 커맨드 범위 계산
        let total_cmds = self.cached_draw_commands.len();
        let phase1_end = self.overlay_cmd_start.unwrap_or(total_cmds).min(total_cmds);
        let phase2_end = self.dropdown_cmd_start.unwrap_or(total_cmds).min(total_cmds);

        // Phase 1: 콘텐츠
        if phase1_end > 0 {
            self.render_interleaved_phase(shared, device, queue, encoder, view, 0, phase1_end);
        }

        // Phase 2: 오버레이/헤더
        if phase1_end < phase2_end {
            self.render_interleaved_phase(shared, device, queue, encoder, view, phase1_end, phase2_end);
        }

        // Phase 3: 드롭다운
        if phase2_end < total_cmds {
            self.render_interleaved_phase(shared, device, queue, encoder, view, phase2_end, total_cmds);
        }
    }

    /// 인터리빙된 Phase 렌더링 (단일 렌더 패스 내에서 지오메트리·텍스트 파이프라인 스위칭)
    fn render_interleaved_phase(
        &mut self,
        shared: &SlateRenderResources,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        cmd_start: usize,
        cmd_end: usize,
    ) {
        use crate::core::EClippingMethod;

        let needs_stencil = self.phase_needs_stencil(cmd_start, cmd_end);

        // 스텐실 필요 시: 텍스처 생성 + 버퍼 업로드
        if needs_stencil {
            self.ensure_stencil_texture(device);
            if !self.cached_stencil_vertices.is_empty() {
                queue.write_buffer(
                    &self.stencil_vertex_buffer, 0,
                    bytemuck::cast_slice(&self.cached_stencil_vertices),
                );
                queue.write_buffer(
                    &self.stencil_index_buffer, 0,
                    bytemuck::cast_slice(&self.cached_stencil_indices),
                );
            }
        }

        // 렌더패스: 스텐실 필요 시 depth_stencil_attachment 설정
        let depth_stencil_attachment = if needs_stencil {
            self.stencil_view.as_ref().map(|sv| wgpu::RenderPassDepthStencilAttachment {
                view: sv,
                depth_ops: None,
                stencil_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0),
                    store: wgpu::StoreOp::Store,
                }),
            })
        } else {
            None
        };

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("RSlate Interleaved Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        let screen_w = self.screen_size.0 as u32;
        let screen_h = self.screen_size.1 as u32;

        // 지오메트리 유니폼은 bind_group 0
        render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);

        // 배치별 활성 스텐실 상태 추적
        let mut active_stencil_clip: Option<usize> = None;
        // 마지막 커맨드가 텍스트였는지 추적 (bind_group 0 복구 판별용)
        let mut last_was_text = false;

        for cmd in &self.cached_draw_commands[cmd_start..cmd_end] {
            match cmd {
                DrawCommand::GeometryBatch(batch_idx) => {
                    let batch = &self.cached_batches[*batch_idx];

                    // 이 배치가 스텐실 클리핑을 사용하는지 판별
                    let batch_uses_stencil = if let Some(clip_idx) = batch.clip_state_index {
                        self.cached_clipping_states.get(clip_idx)
                            .map(|s| s.clipping_method() == EClippingMethod::Stencil)
                            .unwrap_or(false)
                    } else {
                        false
                    };

                    // 텍스트 후 → 지오메트리 전환 시에만 bind_group 0 복구
                    if last_was_text {
                        render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                    }
                    last_was_text = false;

                    if needs_stencil && batch_uses_stencil {
                        let clip_idx = batch.clip_state_index.unwrap();

                        if active_stencil_clip != Some(clip_idx) {
                            active_stencil_clip = Some(clip_idx);

                            render_pass.set_pipeline(&shared.stencil_write_pipeline);
                            render_pass.set_vertex_buffer(0, self.stencil_vertex_buffer.slice(..));
                            render_pass.set_index_buffer(self.stencil_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                            render_pass.set_bind_group(1, &shared.white_texture.bind_group, &[]);
                            render_pass.set_scissor_rect(0, 0, screen_w, screen_h);
                            render_pass.set_stencil_reference(0);
                            render_pass.draw_indexed(0..6, 0, 0..1);

                            if let Some(entry) = self.cached_stencil_entries.iter()
                                .find(|e| e.clip_state_index == clip_idx)
                            {
                                render_pass.set_stencil_reference(entry.stencil_ref);
                                render_pass.draw_indexed(
                                    entry.index_start..(entry.index_start + entry.index_count),
                                    0, 0..1,
                                );
                            }
                        }

                        let stencil_ref = self.cached_stencil_entries.iter()
                            .find(|e| e.clip_state_index == clip_idx)
                            .map(|e| e.stencil_ref)
                            .unwrap_or(1);

                        match batch.kind {
                            BatchKind::Normal => {
                                render_pass.set_pipeline(&shared.stencil_test_pipeline);
                                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                                render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                            }
                            BatchKind::RoundedBox => {
                                render_pass.set_pipeline(&shared.stencil_test_rounded_box_pipeline);
                                render_pass.set_vertex_buffer(0, self.rounded_vertex_buffer.slice(..));
                                render_pass.set_index_buffer(self.rounded_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                            }
                        }
                        render_pass.set_stencil_reference(stencil_ref);

                        if let Some(state) = self.cached_clipping_states.get(clip_idx) {
                            let [cx, cy, cw, ch] = resolve_scissor_rect(state);
                            let sx = (cx.max(0.0)) as u32;
                            let sy = (cy.max(0.0)) as u32;
                            let sw = (cw as u32).min(screen_w.saturating_sub(sx));
                            let sh = (ch as u32).min(screen_h.saturating_sub(sy));
                            render_pass.set_scissor_rect(sx, sy, sw.max(1), sh.max(1));
                        }
                    } else if needs_stencil {
                        active_stencil_clip = None;
                        match batch.kind {
                            BatchKind::Normal => {
                                render_pass.set_pipeline(&shared.stencil_passthrough_pipeline);
                                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                                render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                            }
                            BatchKind::RoundedBox => {
                                render_pass.set_pipeline(&shared.stencil_passthrough_rounded_box_pipeline);
                                render_pass.set_vertex_buffer(0, self.rounded_vertex_buffer.slice(..));
                                render_pass.set_index_buffer(self.rounded_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                            }
                        }

                        if let Some(clip_idx) = batch.clip_state_index {
                            if let Some(state) = self.cached_clipping_states.get(clip_idx) {
                                let [cx, cy, cw, ch] = resolve_scissor_rect(state);
                                let sx = (cx.max(0.0)) as u32;
                                let sy = (cy.max(0.0)) as u32;
                                let sw = (cw as u32).min(screen_w.saturating_sub(sx));
                                let sh = (ch as u32).min(screen_h.saturating_sub(sy));
                                render_pass.set_scissor_rect(sx, sy, sw.max(1), sh.max(1));
                            } else {
                                render_pass.set_scissor_rect(0, 0, screen_w, screen_h);
                            }
                        } else {
                            render_pass.set_scissor_rect(0, 0, screen_w, screen_h);
                        }
                    } else {
                        match batch.kind {
                            BatchKind::Normal => {
                                render_pass.set_pipeline(&shared.pipeline);
                                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                                render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                            }
                            BatchKind::RoundedBox => {
                                render_pass.set_pipeline(&shared.rounded_box_pipeline);
                                render_pass.set_vertex_buffer(0, self.rounded_vertex_buffer.slice(..));
                                render_pass.set_index_buffer(self.rounded_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                            }
                        }

                        if let Some(clip_idx) = batch.clip_state_index {
                            if let Some(state) = self.cached_clipping_states.get(clip_idx) {
                                let [cx, cy, cw, ch] = resolve_scissor_rect(state);
                                let sx = (cx.max(0.0)) as u32;
                                let sy = (cy.max(0.0)) as u32;
                                let sw = (cw as u32).min(screen_w.saturating_sub(sx));
                                let sh = (ch as u32).min(screen_h.saturating_sub(sy));
                                render_pass.set_scissor_rect(sx, sy, sw.max(1), sh.max(1));
                            } else {
                                render_pass.set_scissor_rect(0, 0, screen_w, screen_h);
                            }
                        } else {
                            render_pass.set_scissor_rect(0, 0, screen_w, screen_h);
                        }
                    }

                    let bind_group = if let Some(ref tex_name) = batch.texture_name {
                        if let Some(tex) = shared.textures.get(tex_name) {
                            &tex.bind_group
                        } else {
                            log::warn!("[RSlateRenderer] Texture '{}' not found, using white texture", tex_name);
                            &shared.white_texture.bind_group
                        }
                    } else {
                        &shared.white_texture.bind_group
                    };

                    render_pass.set_bind_group(1, bind_group, &[]);
                    render_pass.draw_indexed(
                        batch.index_start..(batch.index_start + batch.index_count),
                        0,
                        0..1,
                    );
                }

                DrawCommand::TextRange { index_start, index_end, clip_state_index } => {
                    if !self.cached_text_vertices.is_empty() && index_end > index_start {
                        // 텍스트의 스텐실 클리핑 판별
                        let text_uses_stencil = if let Some(ci) = clip_state_index {
                            self.cached_clipping_states.get(*ci)
                                .map(|s| s.clipping_method() == EClippingMethod::Stencil)
                                .unwrap_or(false)
                        } else {
                            false
                        };

                        // 스텐실 모드에 따른 텍스트 파이프라인 선택
                        if needs_stencil && text_uses_stencil {
                            let clip_idx = clip_state_index.unwrap();

                            // 스텐실 write (지오메트리와 동일 패턴)
                            if active_stencil_clip != Some(clip_idx) {
                                active_stencil_clip = Some(clip_idx);

                                render_pass.set_pipeline(&shared.stencil_write_pipeline);
                                render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                                render_pass.set_vertex_buffer(0, self.stencil_vertex_buffer.slice(..));
                                render_pass.set_index_buffer(self.stencil_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                                render_pass.set_bind_group(1, &shared.white_texture.bind_group, &[]);
                                render_pass.set_scissor_rect(0, 0, screen_w, screen_h);
                                render_pass.set_stencil_reference(0);
                                render_pass.draw_indexed(0..6, 0, 0..1);

                                if let Some(entry) = self.cached_stencil_entries.iter()
                                    .find(|e| e.clip_state_index == clip_idx)
                                {
                                    render_pass.set_stencil_reference(entry.stencil_ref);
                                    render_pass.draw_indexed(
                                        entry.index_start..(entry.index_start + entry.index_count),
                                        0, 0..1,
                                    );
                                }
                            }

                            let stencil_ref = self.cached_stencil_entries.iter()
                                .find(|e| e.clip_state_index == clip_idx)
                                .map(|e| e.stencil_ref)
                                .unwrap_or(1);

                            render_pass.set_pipeline(&shared.text.stencil_test_pipeline);
                            render_pass.set_stencil_reference(stencil_ref);

                            if let Some(state) = self.cached_clipping_states.get(clip_idx) {
                                let [cx, cy, cw, ch] = resolve_scissor_rect(state);
                                let sx = (cx.max(0.0)) as u32;
                                let sy = (cy.max(0.0)) as u32;
                                let sw = (cw as u32).min(screen_w.saturating_sub(sx));
                                let sh = (ch as u32).min(screen_h.saturating_sub(sy));
                                render_pass.set_scissor_rect(sx, sy, sw.max(1), sh.max(1));
                            }
                        } else if needs_stencil {
                            active_stencil_clip = None;
                            render_pass.set_pipeline(&shared.text.stencil_passthrough_pipeline);

                            // scissor 설정
                            if let Some(ci) = clip_state_index {
                                if let Some(state) = self.cached_clipping_states.get(*ci) {
                                    let [cx, cy, cw, ch] = resolve_scissor_rect(state);
                                    let sx = (cx.max(0.0)) as u32;
                                    let sy = (cy.max(0.0)) as u32;
                                    let sw = (cw as u32).min(screen_w.saturating_sub(sx));
                                    let sh = (ch as u32).min(screen_h.saturating_sub(sy));
                                    render_pass.set_scissor_rect(sx, sy, sw.max(1), sh.max(1));
                                } else {
                                    render_pass.set_scissor_rect(0, 0, screen_w, screen_h);
                                }
                            } else {
                                render_pass.set_scissor_rect(0, 0, screen_w, screen_h);
                            }
                        } else {
                            render_pass.set_pipeline(&shared.text.pipeline);

                            // scissor 설정
                            if let Some(ci) = clip_state_index {
                                if let Some(state) = self.cached_clipping_states.get(*ci) {
                                    let [cx, cy, cw, ch] = resolve_scissor_rect(state);
                                    let sx = (cx.max(0.0)) as u32;
                                    let sy = (cy.max(0.0)) as u32;
                                    let sw = (cw as u32).min(screen_w.saturating_sub(sx));
                                    let sh = (ch as u32).min(screen_h.saturating_sub(sy));
                                    render_pass.set_scissor_rect(sx, sy, sw.max(1), sh.max(1));
                                } else {
                                    render_pass.set_scissor_rect(0, 0, screen_w, screen_h);
                                }
                            } else {
                                render_pass.set_scissor_rect(0, 0, screen_w, screen_h);
                            }
                        }

                        // 텍스트 바인딩
                        render_pass.set_bind_group(0, &self.text_viewport.uniform_bind_group, &[]);
                        render_pass.set_bind_group(1, &shared.text.atlas_bind_group, &[]);
                        render_pass.set_vertex_buffer(0, self.text_viewport.vertex_buffer.slice(..));
                        render_pass.set_index_buffer(self.text_viewport.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                        render_pass.draw_indexed(*index_start..*index_end, 0, 0..1);
                        last_was_text = true;
                    }
                }
            }
        }
    }
}
