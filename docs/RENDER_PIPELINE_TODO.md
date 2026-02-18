# SKOPE Render Pipeline TODO

> **Last Updated:** 2026-02-08 (Sprint 10 — GPU Scene Incremental + POM + Clear Coat + IBL HDR — v5.0)
> **Branch:** dodam-windows
> **Goal:** UE5-class V-Buffer Deferred Rendering Pipeline

---

## Current Pipeline Overview

```
Frame Start
  |
  v
[Phase 0]    Blue Noise + Sky LUT Precompute
  |
  v
[Phase 1]    GPU Scene Upload + Instance Culling
  |
  v
[Phase 2]    Visibility Pass (V-Buffer: TriangleID + Bary + Depth)
  |
  v
[Phase 2.1]  Nanite Cull + HW/SW Rasterize + V-Buffer Resolve
  |
  v
[Phase 2.5]  Shadow Maps (CSM / VSM + Shadow Depth + SMRT)
  |
  v
[Phase 2.7]  DBuffer Decals → Material Eval 연결
  |
  v
[Phase 3]    Material Eval (Compute) + MegaLights Tile Classify
  |
  v
[Phase 4]    Motion Vectors + HZB Generation
  |
  v
[Phase 4.5]  Sky View LUT
[Phase 4.6]  DF Shadows / DF AO
[Phase 4.7]  VRS Classify
  |
  v
[Phase 5]    GTAO
[Phase 6]    Contact Shadows
[Phase 7]    SSR
  |
  v
[Phase 8]    DDGI Update (previous frame radiance)
[Phase 8.5]  Lumen Screen Probes (Place → Gather → Filter → Composite)
[Phase 8.5.5] Lumen Radiance Cache SH Update
[Phase 8.5.6] Lumen Reflections (Trace → Temporal Filter → History Swap)
  |
  v
[Phase 8.7]  Outline Edge Detection + Composite (compute-only path)
  |
  v
[Phase 9]    Volumetric Fog + SS Composite
[Phase 9.6]  OIT Resolve (transparent geometry composite)
[Phase 9.7]  MegaLights Denoise
[Phase 9.8]  Aerial Perspective
  |
  v
[Phase 10]   TAA / TSR Resolve
  |
  v
[Phase 11]   SSS
[Phase 12]   DoF
  |
  v
[Phase 12.5] OIT Composite (투명 오브젝트, forward pass)
  |
  v
[Phase 13]   Post Processing (Bloom + Auto Exposure + Tonemapping + Color Grading + Film)
  |
  v
[Phase 14]   Debug View / Blit to Screen
```

> **Note:**
> - Phase 번호는 `render_vbuffer()` 실행 순서 기준. MegaLights는 Phase 3 (classify)과 Phase 9.7 (denoise)에 분리 실행됨.
> - Phase 4.5~4.7은 **파이프라인 실행 단계** 번호. Task 4.1~4.5는 **Lumen GI 작업** 번호. 번호 체계가 다르므로 혼동 주의.

---

## Completed Systems (Reference Only)

아래 시스템들의 구현 현황. 100% 항목은 완성, 그 외는 부분 구현 (관련 Task 참조).

| System | Module | Status | Notes |
|--------|--------|--------|-------|
| V-Buffer | `vbuffer.rs` | 100% | Triangle ID encoding/decoding, 12 bytes/pixel |
| V-Buffer Resolve | `vbuffer_resolve.rs` | 100% | Standard geometry resolve + NANITE_FLAG 인프라 |
| Material Eval | `material_eval.rs` | 100% | Cook-Torrance PBR, bindless 4096 slots, 4 bind groups, Nanite/Standard 분기, DBuffer 합성, POM + Clear Coat (Sprint 10) |
| Z-Prepass | `zprepass.rs` | 100% | LESS depth test, dynamic offset |
| TAA | `taa.rs` | 100% | 16-sample Halton, variance clipping |
| TSR | `skope_endgame/tsr.rs` | 100% | 9-phase upscaling, 32-sample Halton |
| Motion Vectors | `motion_vectors.rs` | 100% | View-proj 기반 |
| HZB | `hzb.rs` | 100% | Mip chain construction + prev-frame ping-pong |
| SSR | `ssr.rs` | 100% | Hi-Z ray march + temporal |
| Contact Shadows | `contact_shadows.rs` | 100% | Depth trace |
| GTAO | `gtao.rs` | 100% | Temporal filtering |
| Volumetric Fog | `volumetric.rs` | 100% | Froxel grid 256x128x64 |
| SSS | `sss.rs` | 100% | 17-sample kernel |
| DoF | `dof.rs` | 100% | CoC + tile blur |
| SS Composite | `ss_composite.rs` | 100% | GTAO + Contact + SSR merge |
| CSM | `skope_lighting/shadows.rs` | 100% | 4-cascade, 2048px, PCSS 활성화 (blocker search + variable PCF) |
| Clustered Lighting | `skope_lighting/clustered.rs` | 100% | 16x16x24 grid, CPU fallback |
| BRDF | `skope_lighting/brdf.rs` | 100% | Cook-Torrance + BRDF LUT 512x512 |
| IBL | `skope_lighting/ibl.rs` | 100% | Split-sum (prefiltered + irradiance + BRDF LUT), Material Eval Group 2 bindings 22-25 연결 완료 (Sprint 8). GPU prefilter dispatch 완성. HDR 환경맵 로딩 완료 (Sprint 10) |
| Light Manager | `skope_lighting/lights.rs` | 100% | Point/Spot/Rect/Disk + attenuation |
| Light Probes | `skope_lighting/light_probes.rs` | 100% | SH9 + trilinear interpolation |
| Character Lighting | `skope_lighting/character_lighting.rs` | 100% | Fill/Rim/Face shadow/SSS/Hair |
| LOD Selector | `lod.rs` | 100% | Bounding sphere, screen-space error |
| HLOD | `hlod.rs` | 100% | Cluster organization |
| Shadow Atlas | `shadow_atlas.rs` | 100% | Tile allocator for local lights |
| OIT | `oit.rs` | 90% | Per-pixel linked list build + resolve. Renderer Phase 9.6 통합 (Sprint 9). Transparent mesh source 대기 |
| Stochastic Transparency | `stochastic_transparency.rs` | 100% | TAA noise reduction |
| Animation | `animation.rs` | 100% | Skeletal + morph weights |
| Animation Blend | `animation_blend.rs` | 100% | Crossfade transitions |
| State Machine | `state_machine.rs` | 100% | Layers, blend trees (1D/2D), transitions |
| Skinned Mesh | `skinned_mesh.rs` | 100% | MAX_JOINTS=256 |
| Morph Targets | `morph_target.rs` | 100% | Weight channels |
| Texture Array | `texture_array.rs` | 100% | Bindless heap 4096 slots |
| Blue Noise | `blue_noise.rs` | 100% | Generator + GPU texture |
| Eye Rendering | `eye.rs` | 100% | Parallax iris, cornea refraction |
| Magic Circle | `magic_circle.rs` | 100% | SDF rune rendering |
| GPU Profiler | `profiler.rs` | 100% | Timestamp queries (when supported) |
| Velocity Viz | `velocity_viz.rs` | 100% | Debug overlay |
| Frustum Culling | `frustum.rs` | 100% | Plane extraction |
| Viewport Texture | `viewport_texture.rs` | 100% | Editor viewport |
| Resolution Config | `resolution.rs` | 100% | Native/Quality/Performance |
| Debug Visualization | `debug_viz.rs` | 100% | 15+ 오버레이 모드 (depth, normals, VSM, DF, etc.) |
| DBuffer Decals | `decals.rs` | 100% | Box projection + DBuffer 생성 + dispatch + Material Eval 합성 연결 완료 |
| Instance Culling | `instance_culling.rs` | 85% | Two-pass 인프라 완성 (Pass 0/1 + occluded buffer). Indirect draw 소비는 Sprint 3 |
| Nanite Cull | `skope_virtual_geometry/cull.rs` | 90% | 2D dispatch, normal cone, mesh_ranges 매핑 완성. Renderer 통합 완료 |
| Nanite HW Raster | `skope_virtual_geometry/rasterize.rs` | 90% | Mesh shader pipeline 완성 + Renderer 통합 |
| Nanite SW Raster | `skope_virtual_geometry/rasterize.rs` | 90% | Compute rasterizer + atomicMin depth. Renderer 통합 완료 |
| Nanite V-Buffer | `skope_virtual_geometry/visibility.rs` | 100% | HW/SW merge via vbuffer_resolve. Material Eval Nanite 분기 연결 완료 |
| Distance Field | `distance_field.rs` | 70% | GDF volume + voxelization compute (bounding sphere SDF). Mesh SDF = Phase 2 |
| VRS Classify | `vrs.rs` | 50% | Pipeline 완성, 셰이더 stub |
| DDGI | `ddgi.rs` + `ddgi/` | 60% | 3-level probe cascade (2m/8m/32m) 완성. Normal binding 버그 수정 (Sprint 4). Ray tracing/irradiance update pipeline stub (Phase 15.6) |
| Bloom | `skope_endgame/bloom.rs` | 100% | 13-tap Karis, 7 mips |
| Tonemapping | `skope_endgame/tonemapping.rs` | 100% | ACES/Reinhard/Hable/AgX/Hejl |
| Auto Exposure | `skope_endgame/auto_exposure.rs` | 100% | Histogram (256-bin) + weighted average + temporal adaptation. Pipeline 연결 완료 (Sprint 5) |
| Color Grading | `skope_endgame/color_grading.rs` | 100% | 3D LUT + Lift/Gamma/Gain. execute() + identity LUT 업로드 + pipeline 연결 완료 (Sprint 5) |
| Film Effects | `skope_endgame/film_effects.rs` | 100% | Grain + Vignette |
| Post-Process Pipeline | `skope_endgame/pipeline.rs` | 100% | Full chain: Bloom → Auto Exposure → Tonemapping → Color Grading → Film Effects |
| Shader Preprocessor | `shaders/preprocessor.rs` | 100% | #include with cycle detection + #define/#ifdef/#ifndef/#else/#endif (Sprint 6) |
| Shader Manager | `shaders/manager.rs` | 100% | Hot-reload (Debug), embedded (Release) |
| Pipeline Manager | `shaders/pipeline_manager.rs` | 100% | Auto-rebuild on shader change |
| GPU Resource Pool | `skope_resource` | 100% | PipelineCache, BufferPool, StagingBelt, BindGroupLayoutCache |

---

## Phase 0: Cleanup & Correctness

### 0.1 ~~Dead Blit Bind Group 제거~~ [DONE — Sprint 8]
- **Completed:** Sprint 8. `self.blit_bind_group` 필드, deprecated `render()` 함수, `new()`/`resize()` 초기화 코드 모두 제거.
- `blit_bind_group_layout`, `blit_pipeline`, `blit_sampler`, `blit_params_buffer`, `create_blit_bind_group()` 보존 (활성 경로 `render_blit_with_source()` 사용).

### 0.2 ~~Decal System Dispatch 미연결~~ [DONE]
- **Completed:** Sprint 1. `render_phase_decals()`에서 Phase 2.7로 dispatch 연결. `update_decals()` public API 추가. Shadow (Phase 2.5) 이후, Material Eval (Phase 3) 이전에 실행.

### 0.3 ~~HZB Previous Frame 이슈~~ [DONE]
- **Completed:** Sprint 1. `hzb.rs`에 `prev_hzb_texture`/`prev_hzb_view` 추가. `swap_history()` 메서드로 매 프레임 현재 HZB → prev HZB 복사. Instance culling은 `prev_hzb_view` 사용. `resize()`에서 prev HZB도 재생성.

### 0.4 ~~Instance Culling Dispatch 미연결~~ [DONE]
- **Completed:** Sprint 1. `render_phase_instance_culling()`에서 `instance_culling.cull()` dispatch 연결. `gpu_scene.live_count() > 0` guard. 이전 프레임 HZB (`prev_hzb_view`) 사용. GPU Scene populate (Task 2.1)와 연동하여 자동 활성화.

---

## Phase 1: Renderer Architecture Refactoring

### 1.1 ~~render_vbuffer() 분리~~ [DONE]
- **Completed:** Sprint 1. ~700줄을 10개 phase 함수로 분리:
  ```
  render_phase_precompute()          // Phase 0: Blue Noise + Sky LUT
  render_phase_instance_culling()    // Phase 0.5: GPU Instance Culling
  render_phase_visibility()          // Phase 1-2: V-Buffer + Z-Prepass
  render_phase_nanite()              // Phase 2.1: Nanite Cull + HW/SW Raster + V-Buffer Resolve
  render_phase_shadows()             // Phase 2.5: CSM/VSM
  render_phase_decals()              // Phase 2.7: DBuffer Decals
  render_phase_material_eval()       // Phase 3: Material Eval + MegaLights
  render_phase_motion_hzb()          // Phase 3-4: Motion Vectors + HZB
  render_phase_auxiliary()           // Phase 4.5-4.7: Sky, DF, VRS
  render_phase_screen_space()        // Phase 5-7: GTAO, Contact, SSR
  render_phase_gi_to_final()         // Phase 8-14: GI → Post → Blit
  ```
  `render_vbuffer()`는 orchestrator 역할. Phase 8-14는 borrow checker 제약으로 단일 함수.

### 1.2 Pass-Based Rendering 도입 [MEDIUM]
- **Problem:** 패스가 하드코딩된 순서로 실행. 새 패스 추가/제거 어려움.
- **UE Reference:**
  - `reference/UE_RenderPipeline/Renderer/MeshPassProcessor.cpp` - EMeshPass::Type, FMeshPassProcessor
  - `reference/UE_RenderPipeline/Renderer/DepthRendering.h` - FDepthPassMeshProcessor
- **Approach:**
  1. `RenderPass` trait 정의 (setup, execute, teardown)
  2. Pass registry로 동적 패스 관리
  3. 의존성 기반 자동 정렬 (RDG compile 활용)

### 1.3 ~~View 시스템 도입~~ [DONE — Sprint 6]
- **Completed:** Sprint 6. `FrameView` 구조체 도입.
  - `renderer/types.rs`: `FrameView` struct (view, proj, jittered_proj, view_proj, inv_view_proj, camera_pos, sun_direction, sun_color)
  - `FrameView::new()`: view_proj, inv_view_proj, camera_pos 자동 계산 (view inverse에서 추출)
  - `render_vbuffer()`: FrameView 생성 후 모든 phase 함수에 `&FrameView` 전달
  - 9개 phase 함수 시그니처 통일 (instance_culling, visibility, nanite, shadows, decals, motion_hzb, auxiliary, screen_space, gi_to_final)
  - `camera_pos` 추출 중복 5회 → `frame_view.camera_pos` 직접 참조로 제거
  - `render_phase_precompute`, `render_phase_material_eval`은 카메라 불필요 → 변경 없음

---

## Phase 2: GPU Scene & Culling 완성

### 2.1 GPU Scene 인스턴스 매핑 완성 [PARTIAL — Sprint 1 기초 완료]
- **Current:** ~85%. Instance lifecycle, dirty tracking, free-list, **앱 레이어 연동** 완료.
- **Sprint 1 완료:**
  - `gpu_scene.clear_all()` 메서드 추가 (per-frame rebuild)
  - `state/render.rs`에서 매 프레임 GPU Scene clear + glTF mesh 인스턴스 등록 + upload 연동
  - `gpu_scene_begin_frame()` → `gpu_scene_clear()` → `add_instance()` loop → `gpu_scene_upload()` 흐름 구축
  - **Phase 1 접근:** 매 프레임 clear + 재구축 (단순하지만 비효율적). Phase 2에서 incremental update로 전환 필요.
- **Remaining:**
  1. Incremental update (매 프레임 재구축 → dirty tracking 활용)
  2. ~~Nanite meshlet offset 테이블 연동~~ (3.2 ✅ Sprint 2)
  3. Previous frame transform → motion vector 연결 검증
- **UE Reference:**
  - `reference/UE_RenderPipeline/Renderer/GPUScene.h` - FGPUScene

### 2.2 ~~Instance Culling Two-Pass 인프라~~ [DONE — Sprint 2]
- **Completed:** Sprint 2. Two-pass 인프라 구축 완료.
  - `instance_culling.wgsl`: `pass_index` 분기 — Pass 0에서 occluded → `occluded_indices` 버퍼 기록, Pass 1에서 re-test
  - `instance_culling.rs`: `occluded_indices_buffer`, `occluded_count_buffer` 추가. `cull_pass1()` 메서드 추가. counters 3개 (visible/draw_cmd/occluded).
  - WGSL binding 6: `occluded_indices` storage buffer 추가
  - **Note:** 실제 two-pass 전환은 indirect draw 도입 시 (Sprint 3) 활성화. 현재 visibility pass는 direct draw → culling 결과 미반영.
- **Remaining (Sprint 3):**
  1. Indirect draw argument 소비 → visibility pass 연결
  2. Stats readback (culled count)

---

## Phase 3: Nanite (skope_virtual_geometry) 완성

### 3.1 ~~Normal Cone 계산 구현~~ [DONE — Sprint 2]
- **Completed:** Sprint 2. `meshlet.rs`에 `compute_normal_cone()` 구현.
  - Face normal 계산 (cross product) → 평균 normal → cone axis (정규화)
  - 각 normal과 axis 사이 dot 최솟값 → `cos(half_angle)`
  - Degenerate (zero area tri, 반구 이상 spread) → `cos = -1.0` (backface cull 비활성화)
  - 4개 단위 테스트 추가 (`cargo test -p skope_virtual_geometry` 통과)

### 3.2 ~~Instance-to-Meshlet 매핑 테이블~~ [DONE — Sprint 2]
- **Completed:** Sprint 2. 2D Dispatch 방식으로 해결.
  - `types.rs`: `MeshMeshletRange { meshlet_offset, meshlet_count }` struct 추가
  - `cull.rs`: G1 layout에 `mesh_ranges` binding 추가, `dispatch()` 2D dispatch (gid.x = local meshlet, gid.y = instance)
  - `nanite_cull.wgsl`: `mesh_ranges` 바인딩 + instance_id = gid.y 계산 + `mesh_ranges[mesh_id]` 룩업

### 3.3 ~~Nanite HW/SW V-Buffer Merge + Renderer 통합~~ [DONE — Sprint 2]
- **Completed:** Sprint 2. `renderer.rs`에 전체 Nanite 파이프라인 통합.
  - `render_phase_nanite()` Phase 2.1로 삽입 (Visibility 직후, Shadows 이전)
  - Nanite Cull Pipeline → HW Mesh Shader Rasterization → SW Compute Rasterization → V-Buffer Resolve
  - `vbuffer_resolve.rs`의 기존 인프라 활용: Standard + Nanite depth 비교 머지
  - `upload_nanite_meshes()` / `update_nanite_instances()` public API
  - Nanite 데이터 없으면 Phase 스킵 (no-op)
  - resize() 시 nanite_vbuffer, sw_raster, vbuffer_resolve 재생성

### 3.4 ~~Nanite → Material Eval 연결~~ [DONE — Sprint 3]
- **Completed:** Sprint 3. Material Eval에 Nanite 전체 파이프라인 연결.
  - `NaniteFullVertex` 64-byte struct 추가 (position, normal, tangent, UV — GpuVertex와 동일 레이아웃)
  - `NaniteMesh.vertex_data` 필드 추가 + `upload_nanite_meshes()`에서 full vertex buffer 업로드 (fallback 생성 포함)
  - Group 0 binding 2: `texture_depth_2d` → `texture_2d<f32>` (merged resolve R32Float 대응)
  - Group 1 bindings 3-6: Nanite vertices/triangles/meshlets/instances storage 바인딩 추가
  - `create_merged_vbuffer_bind_group()` + `create_geometry_bind_group_with_nanite()` 메서드 추가
  - `material_eval.wgsl`: NANITE_FLAG(bit 31) 분기 — Nanite path에서 cluster_id(20) | tri_id(7) | mat_id(5) 디코딩, meshlet vertex fetch, barycentric interpolation
  - Standard path 기존 동작 유지

### 3.5 통계 Readback [LOW]
- **Problem:** `NaniteFrameResult` 정의만 있고 데이터 수집 없음.
- **Tasks:**
  1. Atomic counter readback (visible clusters, HW/SW 비율)
  2. Debug overlay 표시 (DebugView::NaniteClusterLod)

---

## Phase 4: Lumen GI (skope_bishop) 완성

### 4.1 ~~SDF Volume Population~~ [DONE — Sprint 4]
- **Completed:** Sprint 4. Bounding sphere SDF voxelization.
  - `sdf_voxelize.wgsl`: GPU Scene 인스턴스 bounding sphere 기반 signed distance 계산 (workgroup 4x4x4)
  - `distance_field.rs`: `VoxelizeParams` struct + `voxelize_pipeline` + `voxelize()` 메서드 추가
  - `renderer.rs`: `render_phase_auxiliary()`에서 `enable_df_shadows || enable_df_ao || enable_lumen_gi` 시 voxelization dispatch
  - **Phase 1:** Bounding sphere SDF (근사). Phase 2에서 정확한 mesh SDF로 교체 예정.
- **Remaining (Phase 2):**
  1. 정확한 mesh → SDF voxelization
  2. 점진적 업데이트 (dirty region만)
  3. Camera-centered volume repositioning 시 데이터 재사용

### 4.2 ~~Screen Probe Gather에 실제 Radiance 연결~~ [DONE — Sprint 4]
- **Completed:** Sprint 4. Placeholder radiance → prev-frame HDR fetch.
  - `lumen_screen_probe_gather.wgsl`: binding(5) `prev_hdr` + binding(6) `hdr_sampler` 추가
  - Screen-space hit: `view_proj` 투영 → prev HDR `textureSampleLevel()` fetch
  - SDF hit: 동일 방식 + on-screen check, off-screen → sky fallback
  - Sky fallback: 기존 gradient 유지 (sky atmosphere LUT 연결은 Phase 2)
  - `screen_probe.rs`: `gather_input_layout` binding 5-6 추가
  - `renderer.rs`: gather bind group에 `material_eval.output_view` + `lumen_linear_sampler` 바인딩
  - **전체 pipeline renderer 통합 완료:** Place → Gather → Filter → Composite 4-pass dispatch
  - DDGI normal binding 버그 수정 (depth_view 중복 → normal_roughness_view)
  - Placement depth binding type: `Depth` → `Float { filterable: true }` (merged R32Float 대응)
- **Remaining (Phase 2):**
  1. Surface cache radiance lookup (4.4 완료 후)
  2. Sky atmosphere LUT 샘플링 연결
  3. Albedo GBuffer 도입 → Composite에서 `irradiance * albedo / PI` 복원

### 4.3 ~~Radiance Cache SH Update Shader~~ [DONE — Sprint 5]
- **Completed:** Sprint 5. World-space radiance cache SH encoding.
  - `lumen_radiance_cache_sh_update.wgsl` (NEW): @workgroup_size(64) compute shader — cache probe → screen 투영 → 2x2 bilinear screen probe 샘플링 → L2 SH basis (9 coefficients) → temporal blend
  - `SHUpdateParams` struct (types.rs): view_proj, cache_origin, probe_spacing, grid_size, update range 등 16 fields
  - `SHUpdatePipeline` struct (radiance_cache.rs): 3 bind group layouts (params, screen_data, cache_data)
  - `renderer.rs`: Phase 8.5.5에서 `update_origin()` → `update_range()` → params upload → dispatch
  - Round-robin update: ~6%/frame, off-screen probes validity 감쇠 (0.95)

### 4.4 Surface Cache / Mesh Card 시스템 [MEDIUM]
- **Problem:** `SurfaceCard` 타입만 정의. capture/atlas 미구현.
- **UE Reference:**
  - `reference/UE_RenderPipeline/Renderer/Lumen/LumenMeshCards.h`
  - `reference/UE_RenderPipeline/Renderer/Lumen/LumenSurfaceCacheFeedback.h`
  - `reference/UE_RenderPipeline/Renderer/DeferredShadingRenderer.h` - FLumenCardRenderer
- **Tasks:**
  1. Mesh → Surface card 생성 (6방향 직교 투영)
  2. Surface cache atlas 관리 (할당/해제)
  3. Radiance capture (light injection into cards)
  4. Gather 셰이더에서 card lookup 연결

### 4.5 ~~Reflection Radiance Cache Fallback~~ [DONE — Sprint 6]
- **Completed:** Sprint 6. Screen trace miss 시 radiance cache SH fallback.
  - `reflections.rs`: `ReflectionParams` 확장 — `grid_size`, `probe_spacing`, `cache_origin` 추가 (256 bytes)
  - `lumen_reflections_trace.wgsl`: 전면 리팩터
    - `RadianceCacheProbe` struct 수정 — Rust layout과 일치 (world_pos, validity, sh_coefficients[9], last_update_frame, _pad)
    - Binding type: `array<vec4<f32>>` → `array<RadianceCacheProbe>`
    - `sh_basis()`: L2 SH 9-coefficient basis evaluation
    - `evaluate_sh()`: probe SH → RGB 색상 평가
    - `world_to_grid()`: world pos → grid 좌표 변환 (clamped)
    - `sample_radiance_cache()`: 8-probe trilinear interpolation (validity-weighted)
    - Miss case: `vec4(0.0)` → `cache_color * attenuation * 0.5`, w=0.5 (cache hit marker)
  - SH basis coefficients: SH update shader와 동일 (0.282095, 0.488603, 1.092548, 0.315392, 0.546274)
  - **Sprint 7:** Renderer dispatch 연결 완료 → Phase 8.5.6
    - `lumen_reflections: Option<LumenReflectionsPipeline>` field + init (enable_lumen_gi guard)
    - `trace()` → `temporal_filter()` → `swap_history()` dispatch chain
    - Bindings: depth_view, normal_roughness_view, hzb_view, material_eval output, radiance cache probes
    - resize() 연결

---

## Phase 5: Lighting System 강화

### 5.1 ~~PCSS (Percentage Closer Soft Shadows) 활성화~~ [DONE — Sprint 3]
- **Completed:** Sprint 3. `material_eval.wgsl`에 PCSS 구현.
  - `pcss_blocker_search()`: 8 Poisson sample로 blocker 평균 depth 검색 (search_radius = light_size * texel_size * 20)
  - `pcss_shadow()`: blocker depth → penumbra 크기 계산 → variable-radius PCF (clamp 1~8)
  - `sample_csm_shadow()`: `shadow_uniforms.pcss_enabled` 분기 — PCSS 활성 시 `pcss_shadow()`, 비활성 시 기존 `pcf_shadow()`
  - Rust 변경 불필요 — `pcss_enabled`/`pcss_light_size`는 이미 `ShadowUniforms`에 업로드됨

### 5.2 VSM WGSL Shader 검증 + Shadow Depth + SMRT Dispatch [VERIFIED — Sprint 9]
- **Current:** VERIFIED — fully integrated, shader sampling implemented. CPU-side 파이프라인 완성 + **Shadow Depth rendering + SMRT dispatch 연결 (Sprint 7)**.
- **Sprint 7 추가:**
  - `vsm_shadow_depth.wgsl` (NEW): depth-only vertex shader for physical pool rendering
  - `VsmShadowUniforms` struct + `shadow_depth_pipeline` + `render_shadow_depth()` in VirtualShadowMap
  - `SmrtPipeline` renderer dispatch: scene depth + page table + physical pool → soft shadow factor
  - Renderer `render_phase_shadows()`: mark → allocate → shadow depth → SMRT trace → material_eval 연결
- **UE Reference:**
  - `reference/UE_RenderPipeline/Renderer/VirtualShadowMaps/VirtualShadowMapArray.h`
  - `reference/UE_RenderPipeline/Renderer/VirtualShadowMaps/VirtualShadowMapClipmap.h`
  - `reference/UE_RenderPipeline/Shaders/VirtualShadowMaps/`
- **Remaining Tasks:**
  1. ~~Page marking compute shader 검증~~ ✅ (mark_pages dispatch 연결됨)
  2. ~~Page allocation compute shader 검증~~ ✅ (allocate_pages dispatch 연결됨)
  3. ~~Shadow depth rendering 검증~~ ✅ (render_shadow_depth() 연결됨 — Sprint 7)
  4. ~~SMRT 셰이더 검증~~ ✅ (SmrtPipeline.trace() dispatch 연결됨 — Sprint 7)
  5. ~~Material eval VSM 샘플링 연결 검증~~ ✅ (set_vsm_resources 연결됨, vsm_sample_shadow() 셰이더 검증 완료 — Sprint 9)

### 5.3 ~~MegaLights WGSL Shader 검증~~ [VERIFIED — Sprint 8]
- **Status:** VERIFIED — working as designed, gaps documented.
- **Pipeline 연결 상태 (모두 정상):**
  - Classify (tile): `update_megalights()` → `classify()` ✅
  - Sample (RIS): `update_megalights()` → `sample()` ✅
  - Denoise (temporal): Phase 9.7 → `denoise()` ✅
  - Material Eval 연결: Group 2 bindings 17-18, `set_megalights_resources()` ✅
  - Shader 읽기: `megalights_params.max_lights > 0u` → `textureLoad(megalights_output)` ✅
- **알려진 갭 (향후 Sprint 대상):**
  1. `VisibleLightHash` — sampling shader에 선언만 됨, 실제 해시 계산 미구현
  2. `prev_reservoir_buffer` — 할당됨, temporal reuse 미구현 (현재 single-bounce RIS만)
  3. Shadow evaluation on RIS winner — 선택된 light에 대한 shadow 쿼리 없음

### 5.4 ~~IBL Integration~~ [DONE — Sprint 8]
- **Completed:** Sprint 8. IBLEnvironment → MaterialEval Group 2 (bindings 22-25) 연결 + split-sum 셰이더.
  - `material_eval.rs`: Group 2 bindings 22-25 (IBL cubemaps + BRDF LUT + sampler) + dummy/active 리소스 패턴 + `set_ibl_resources()` + `rebuild_group2()`
  - `material_eval/types.rs`: `ibl_intensity: f32` 추가 (default 0.3)
  - `material_eval.wgsl`: `@group(2) @binding(22-25)` 바인딩 + `sample_ibl()` split-sum + main lighting 연결
  - `ibl.rs`: `prefiltered_view()`, `irradiance_view()`, `brdf_lut_view()`, `sampler()` getter 추가
  - `renderer.rs`: `IBLEnvironment` field + `new()` 초기화 + `set_ibl_resources()` 연결
  - Ambient fallback: DDGI 또는 IBL 활성 시 0.3으로 감소
  - Note: 초기 Group 4 접근 → `max_bind_groups=4` 제한으로 Group 2 통합
- **Remaining (향후 Sprint):**
  1. Specular prefilter 검증 (roughness mip별 결과 확인)
  2. Irradiance convolution compute 검증 (cosine-weighted hemisphere)
  3. HDR 환경맵 로딩 → equirectangular → cubemap 변환 런타임 파이프라인

### 5.5 Area Light Evaluation [LOW]
- **Problem:** RectAreaLight/DiskAreaLight 타입 정의됨. 셰이더 미구현.
- **UE Reference:**
  - `reference/UE_RenderPipeline/Shaders/AreaLightCommon.ush`
  - `reference/UE_RenderPipeline/Shaders/RectLight.ush`
  - `reference/UE_RenderPipeline/Shaders/CapsuleLight.ush`
- **Tasks:**
  1. LTC 기반 rect area light
  2. Representative point disk light
  3. Material eval clustered lighting 루프에 통합

---

## Phase 6: Material System 강화

### 6.1 Parallax Occlusion Mapping (POM) [MEDIUM]
- **Problem:** Normal mapping만 구현. Height map 기반 parallax 없음.
- **UE Reference:**
  - `reference/UE_RenderPipeline/Shaders/BasePassPixelShader.usf` - POM 관련 로직
- **Tasks:**
  1. Height texture handle을 Material 구조체에 추가
  2. material_eval.wgsl에 POM ray march
  3. Self-shadow 옵션

### 6.2 Clear Coat Material [MEDIUM]
- **Problem:** 단일 레이어 PBR만 지원.
- **UE Reference:**
  - `reference/UE_RenderPipeline/Shaders/ClearCoatCommon.ush`
  - `reference/UE_RenderPipeline/Shaders/BRDF.ush` - ClearCoatRoughness
- **Tasks:**
  1. Material에 clear_coat, clear_coat_roughness 추가
  2. Dual-lobe specular (base + coat)
  3. Fresnel 층간 에너지 보존

### 6.3 Anisotropic BRDF [LOW]
- **Problem:** Isotropic GGX만 사용.
- **UE Reference:**
  - `reference/UE_RenderPipeline/Renderer/AnisotropyRendering.h`
  - `reference/UE_RenderPipeline/Shaders/BRDF.ush` - BxDFContext XoV, YoV
- **Tasks:**
  1. GGX anisotropic NDF (roughness_x, roughness_y)
  2. Material에 anisotropy 파라미터

### 6.4 Shading Energy Conservation [LOW]
- **UE Reference:** `reference/UE_RenderPipeline/Renderer/ShadingEnergyConservation.h`
- **Tasks:**
  1. Energy compensation LUT 생성
  2. Specular compensation 적용
  3. Diffuse 에너지 복구

### 6.5 Subsurface Profile [LOW]
- **Problem:** SSS blur 있지만 per-material 프로필 없음. Dummy white mask.
- **UE Reference:**
  - `reference/UE_RenderPipeline/Shaders/BurleyNormalizedSSSCommon.ush`
  - `reference/UE_RenderPipeline/Shaders/SubsurfaceProfileCommon.ush`
- **Tasks:**
  1. Material eval에서 SSS mask 텍스처 생성
  2. Per-material SSS 프로필 (scatter radius, color)
  3. Burley diffusion profile

---

## Phase 7: Screen-Space Effects 개선

### 7.1 ~~GTAO → Contact Shadows 순서 변경~~ [DONE]
- **Completed:** Sprint 1. GTAO → Phase 5, Contact Shadows → Phase 6으로 교환. Pipeline Overview 다이어그램도 갱신됨. AO-modulated contact shadow (ss_composite 연동)는 향후 개선 가능.

### 7.2 SSR Importance Sampling 개선 [MEDIUM]
- **UE Reference:**
  - `reference/UE_RenderPipeline/Renderer/ScreenSpaceRayTracing.h`
  - `reference/UE_RenderPipeline/Shaders/SSRT/SSRTReflections.usf`
  - `reference/UE_RenderPipeline/Shaders/ScreenSpaceReflectionTileCommons.ush`
- **Tasks:**
  1. GGX importance sampling 반사 방향 분산
  2. Roughness 기반 cone tracing (mip selection)
  3. Temporal accumulation 개선

### 7.3 GTAO Multi-Bounce 근사 [LOW]
- **Tasks:**
  1. Jimenez et al. multi-bounce AO
  2. Base color 기반 AO 색상 틴팅

---

## Phase 8: Transparency & Special Rendering

### 8.1 OIT Resolve 셰이더 연결 검증 [PARTIAL — Sprint 9]
- **Current:** ~90% (445줄). Build pipeline, resolve pipeline, bind groups, clear/resize 모두 구현 완료. **Renderer 통합 완료 (Sprint 9)**.
- **Sprint 9 추가:**
  - `RenderSettings`: `enable_oit: bool`, `enable_stochastic_vfx: bool` 플래그 추가
  - `renderer.rs`: Phase 9.6에 OIT clear + resolve skeleton 삽입 (transparent mesh 없이 pass-through)
  - `renderer.rs`: `render_vbuffer()` 시작에 `stochastic.begin_frame()` 호출
  - Pipeline Overview에 Phase 9.6 추가
- **Remaining:** transparent mesh submission API 구현 후 실제 투명 오브젝트 렌더링 검증.
- **UE Reference:**
  - `reference/UE_RenderPipeline/Renderer/TranslucentRendering.h`
  - `reference/UE_RenderPipeline/Renderer/TranslucentLighting.h`
- **Tasks:**
  1. Transparent mesh submission API (render_transparent_meshes)
  2. OIT build pass dispatch (create_build_bind_group → transparent render pass)
  3. Linked list resolve 동작 검증 (depth sort + blend)
  4. Max node 초과 시 fallback (closest N fragments)
  5. 반투명 오브젝트 라이팅 (clustered forward)

### 8.2 ~~Decal DBuffer Material Eval 연결~~ [DONE — Sprint 3]
- **Completed:** Sprint 3. DBuffer → Material Eval 전체 연결.
  - Group 2 bindings 19-21: `dbuffer_albedo` (Rgba8Unorm), `dbuffer_normal` (Rgba8Snorm), `dbuffer_roughness` (Rgba8Unorm) 추가
  - Dummy DBuffer 텍스처 + active resource tracking 패턴 적용
  - `set_dbuffer_resources()` 메서드: `render_phase_material_eval()`에서 DBuffer view 전달
  - `material_eval.wgsl`: material sampling 후 DBuffer 합성 — alpha 기반 albedo/normal/roughness mix
  - `render_settings.enable_decals` guard로 조건부 활성화

### 8.3 ~~Outline Rendering (skope_check) Dispatch 연결~~ [DONE — Sprint 7, compute-only path]
- **Completed:** Sprint 7. Compute-only edge detection + composite dispatch 연결 (hull pass skip).
- **Sprint 7 구현:**
  - `enable_outline: bool` in `RenderSettings`
  - `OutlinePipeline` + `OutlineBuffers` + `dummy_r32float_view` (model_id placeholder) fields
  - Phase 8.7 dispatch: hull clear → edge detection (depth/normal Sobel, 8x8) → composite (8x8)
  - Bindings: `merged_depth_view` (R32Float) + `normal_roughness_view` + `edge_mask_view` (storage) + `outline_view` (storage)
  - `edge_use_object_id = 0` (no model ID in compute-only path)
  - resize() 연결
- **Remaining (future sprints):**
  1. Hull expansion 셰이더 활성화 (per-mesh smooth normals 필요)
  2. Model ID 텍스처 연결 (object-level edge detection)
  3. 4 preset 런타임 전환 검증

### 8.4 Hair Rendering (skope_fianchetto) 통합 검증 [LOW]
- **Current:** ~85%. Card + Flyaway strand + Silhouette strand hybrid architecture. LOD system (4 levels). Marschner BRDF 모듈 참조. Deep shadow map 구조. **WGSL 셰이더 7개 모두 구현됨** (`hair_card.wgsl` 125+줄, `hair_strand_rasterize.wgsl` 200+줄, `hair_flyaway_generate.wgsl`, `hair_strand_spawn.wgsl`, `hair_composite.wgsl`, `hair_deep_shadow.wgsl`, `hair_env_lighting.wgsl`).
- **Missing:** Marschner R/TT/TRT lobe evaluation 셰이더 내 구현 검증, strand physics compute, main renderer dispatch 연결.
- **UE Reference:**
  - `reference/UE_RenderPipeline/Renderer/HairStrands/` - Hair pipeline
  - `reference/UE_RenderPipeline/Shaders/HairStrands/` - Hair shading
- **Tasks:**
  1. Card shader 검증 (alpha test + Marschner specular 연결)
  2. Strand rasterize shader 검증 (line ribbon tube expansion)
  3. Flyaway compute shader 검증 (procedural generation)
  4. Marschner R/TT/TRT lobe evaluation 완성
  5. Strand physics simulation compute shader
  6. Main renderer에 hair pass 연결

---

## Phase 9: Post-Processing 강화

### 9.1 ~~Auto Exposure 연결~~ [DONE — Sprint 5]
- **Completed:** Sprint 5. Full histogram-based auto exposure.
  - `auto_exposure.rs`: `execute()` 메서드 추가 — histogram pass (8x8 workgroups) + average pass (1 workgroup, 256 threads)
  - `pipeline.rs`: `PostProcessConfig.auto_exposure_enabled` (default: false), `auto_exposure` 필드 추가
  - `tonemapping.rs`: binding 5 (ExposureResult storage buffer), `default_exposure_buffer` (zero-init), `execute()` 시그니처에 `exposure_buffer: Option<&wgpu::Buffer>` 추가
  - `tonemapping.wgsl`: `auto_exposure.current_exposure > 0.0` 시 자동 노출, 아니면 manual exposure
  - `execute_internal()` / `execute()` / `execute_with_gbuffer()`: `queue: &wgpu::Queue` 파라미터 추가
  - Renderer 호출부 수정 완료

### 9.2 ~~Color Grading LUT 연결~~ [DONE — Sprint 5]
- **Completed:** Sprint 5. Pipeline 연결 + identity LUT 업로드.
  - `color_grading.rs`: `execute()` 메서드 추가 — 3D LUT 적용 compute dispatch
  - `pipeline.rs`: `PostProcessPipeline::new()`에서 identity LUT 32x32x32 업로드
  - `execute_internal()`: tonemapping 이후 color grading, film effects 입력 분기, `get_final_output_view()` 업데이트
  - **Remaining:** .cube 파일 로딩, 런타임 LUT 전환, UI 연결

### 9.3 Chromatic Aberration 활성화 [LOW]
- **Tasks:** 활성화 + 테스트 + UI 연결

---

## Phase 10: Virtual Texture Streaming (skope_promotion)

### 10.1 VT Streaming 검증 + Disk I/O [MEDIUM]
- **Current:** ~85%. Page table, physical pool (4096x4096 atlas), LRU cache, feedback pipeline 모두 CPU-side 완성. 128-texel pages, 4-texel border. **WGSL 셰이더 구현됨** (`vt_feedback.wgsl`, `vt_page_table.wgsl`).
- **Missing:** Disk I/O layer, streaming task 스케줄링, material eval VT 샘플링 연결.
- **UE Reference:**
  - `reference/UE_RenderPipeline/Shaders/VirtualTextureCommon.ush`
  - `reference/UE_RenderPipeline/Shaders/VirtualTextureMaterial.usf`
- **Tasks:**
  1. Feedback generation shader 검증 (material eval에서 page request 기록)
  2. Page table update shader 검증 (indirection 텍스처 갱신)
  3. Material eval에서 VT 샘플링 (page table → physical atlas UV 변환)
  4. Disk I/O layer (page 데이터 로딩)
  5. Background streaming task spawning

### 10.2 VT Material Eval 통합 [MEDIUM]
- **Depends on:** 10.1
- **Tasks:**
  1. Material 구조체에 VT handle 추가
  2. Bindless texture와 VT의 통합 샘플링 경로
  3. Fallback mip (page 미로드 시)

---

## Phase 11: RDG Migration (점진적)

### 11.1 RDG 패스 등록 시작 [HIGH]
- **Problem:** `skope_zugzwang` RDG 완전 구현이지만 **패스 0개 등록**. Dead code.
- **UE Reference:** `reference/UE_RenderPipeline/RenderCore/RenderGraphBuilder.h`
- **Approach:** 한 번에 전부 마이그레이션하지 말고 점진적으로
  1. Phase 0-2 (Blue Noise, Visibility, Shadows) 먼저 RDG 등록
  2. 검증 후 Phase 3-9 확장
  3. 마지막으로 Phase 10-14

### 11.2 RDG Intra-Frame Aliasing [LOW]
- **Problem:** 현재 transient 리소스 동시 할당. Intra-frame 재사용 불가.
- **UE Reference:** `reference/UE_RenderPipeline/RenderCore/RenderGraphBuilder.h` - Resource lifetime
- **Tasks:**
  1. Pass별 first-write / last-read 추적
  2. 비겹침 lifetime 리소스 동일 메모리 매핑
  3. 메모리 사용량 감소

---

## Phase 12: Shader Infrastructure 강화

### 12.1 ~~Shader Variant / Conditional Compilation~~ [DONE — Sprint 6]
- **Completed:** Sprint 6. 전처리기에 조건부 컴파일 지원 추가.
  - `preprocessor.rs`:
    - `defines: HashMap<String, Option<String>>` 필드 추가
    - `with_defines()` 생성자, `define()`, `undefine()` API
    - `#define NAME` / `#define NAME VALUE` — 매크로 정의
    - `#ifdef NAME` / `#ifndef NAME` — 조건부 블록
    - `#else` / `#endif` — 블록 제어
    - `Vec<bool>` 조건 스택으로 중첩 ifdef 지원
    - `#else` 구현: 부모 active 상태 보존 (중첩 안전)
    - 매크로 값 치환: `#define NAME VALUE` → 소스 내 단어 경계 기준 치환 (`replace_word_boundary()`)
    - `PreprocessError` 확장: `UnmatchedEndif`, `UnmatchedElse`, `UnclosedIfdef`
    - 13개 신규 단위 테스트 (모두 통과)
  - `manager.rs`:
    - `global_defines` 필드 + `set_define()` / `remove_define()` API
    - Preprocessor에 global defines 자동 전파
  - **Remaining:** Permutation 키 시스템 (런타임 변형 캐싱)은 Phase 2

### 12.2 Common Shader 동기화 자동화 [LOW]
- **Problem:** `src/shaders/common/`과 `engine/shaders/common/`이 수동 동기화. 발산 위험.
- **Tasks:**
  1. build.rs에서 자동 동기화 (한 쪽을 source of truth로)
  2. 또는 심볼릭 링크 사용

### 12.3 Common Shader 유틸 확장 [LOW]
- **Missing vs UE5:**
  - Packed normal/roughness helpers
  - Anisotropic BRDF utils (6.3과 연계)
  - SH evaluation functions (Lumen용)
  - Color space 변환 (ACEScg, LMS)
- **UE Reference:**
  - `reference/UE_RenderPipeline/Shaders/Common.ush`
  - `reference/UE_RenderPipeline/Shaders/BRDF.ush`
  - `reference/UE_RenderPipeline/Shaders/SHCommon.ush`

---

## Phase 13: Sky & Atmosphere 완성

### 13.1 ~~Aerial Perspective HDR 체인 연결~~ [DONE — Sprint 9]
- **Completed:** Sprint 9. Aerial perspective 출력을 HDR 체인에 연결.
  - `renderer.rs`: Phase 10 (TAA/TSR), Phase 11 (SSS), Phase 12 (DoF), Phase 13 (Post Processing) HDR 입력 체인에 `sky_atmosphere.aerial_output_view` 분기 추가
  - `enable_sky_atmosphere = true` 시 aerial perspective가 최종 출력에 반영됨
  - 이전: `apply_aerial_perspective()` 계산 결과가 `aerial_output_view`에 쓰이지만 후속 체인에서 참조하지 않아 매 프레임 버려짐
  - 이후: 모든 HDR 입력 분기에서 TAA/TSR 이전 단계로 aerial output 우선 참조
- **Remaining:**
  1. LUT 정확도 검증 (ground truth 비교)
  2. Sun disk 렌더링

### 13.2 Volumetric Cloud [FUTURE]
- **UE Reference:**
  - `reference/UE_RenderPipeline/Renderer/VolumetricCloudRendering.h`
  - `reference/UE_RenderPipeline/Shaders/VolumetricCloud.usf`
  - `reference/UE_RenderPipeline/Shaders/VolumetricCloudCommon.ush`

---

## Phase 14: Debug & Profiling

### 14.1 GPU Profiler Overlay [MEDIUM]
- **Tasks:**
  1. Per-pass timing 수집
  2. 타이밍 바 그래프 렌더링
  3. VRAM 사용량 추적

### 14.2 Debug View 완성 [LOW]
- **Note:** `debug_viz.rs`에 15+ 오버레이 모드 이미 구현됨. 아래는 미완성 뷰.
- **Tasks:**
  1. Nanite triangle source 뷰 (3.3 완료 후)
  2. Lumen screen probe 뷰 (4.2 완료 후)
  3. GPU Scene bounds 뷰
  4. Instance culling visualization (0.4 완료 후)

---

## Phase 15: Future Features

### 15.1 Texture Streaming [FUTURE]
### 15.2 Water Rendering [FUTURE]
- **UE Reference:** `reference/UE_RenderPipeline/Renderer/SingleLayerWaterRendering.h`
### 15.3 VR Support [FUTURE]
### 15.4 Ray Tracing [FUTURE]
- wgpu RT extensions 대기
### 15.5 VRS Shader 완성 [FUTURE]
- `vrs.rs` 파이프라인 완성됨. `vrs_classify.wgsl` 셰이더 로직이 stub.
### 15.6 DDGI Ray Tracing Pipeline 완성 [FUTURE]
- `ddgi.rs` probe cascade 완성. Ray tracing / irradiance update pipeline 미구현.

---

## Implementation Order (Recommended)

```
Sprint 1 (Week 1-2): Foundation ✅ COMPLETED
  [0.4] Instance Culling dispatch 연결 ✅
  [0.2] Decal dispatch 연결 ✅
  [0.3] HZB previous frame ping-pong ✅
  [1.1] render_vbuffer() 함수 분리 ✅
  [7.1] GTAO/Contact Shadow 순서 수정 ✅
  [2.1] GPU Scene 인스턴스 매핑 (기초 연동 완료, incremental update 잔여) ✅

Sprint 2 (Week 3-4): Nanite Core + Culling ✅ COMPLETED
  [2.2] Instance Culling Two-Pass 인프라 ✅
  [3.1] Normal Cone 계산 ✅
  [3.2] Instance-to-Meshlet 매핑 ✅
  [3.3] Nanite HW/SW V-Buffer Merge + Renderer 통합 ✅

Sprint 3 (Week 5-6): Lighting & Shadows + Nanite Integration ✅ COMPLETED
  [3.4] Nanite → Material Eval 연결 ✅
  [5.1] PCSS 활성화 ✅
  [5.2] VSM 셰이더 검증 (Sprint 4+로 이월)
  [8.2] Decal DBuffer material eval 연결 ✅

Sprint 4 (Week 7-8): Lumen Phase 1 ✅ COMPLETED
  [4.1] SDF Volume Population ✅
  [4.2] Screen Probe Gather 실제 radiance + Renderer 통합 ✅
  + DDGI normal binding 버그 수정 ✅
  + Placement depth binding type 수정 ✅
  + Composite pipeline dispatch 연결 ✅

Sprint 5 (Week 9-10): Lumen Phase 2 + Post ✅ COMPLETED
  [4.3] Radiance Cache SH Update ✅
  [9.1] Auto Exposure 연결 ✅
  [9.2] Color Grading 연결 ✅

Sprint 6 (Week 11-12): Shader Infra + Reflection + View ✅ COMPLETED
  [12.1] Shader variant system ✅
  [4.5] Reflection Radiance Cache Fallback ✅
  [1.3] View 시스템 도입 ✅

Sprint 7 (Week 13-14): Renderer Dispatch Closure + Quality ✅ COMPLETED
  [4.5+] Lumen Reflections dispatch 연결 ✅ (Sprint 6 closure)
  [8.3] Outline rendering dispatch 연결 ✅ (compute-only path)
  [5.2+] VSM Shadow Depth + SMRT dispatch 연결 ✅

Sprint 8 (Week 15-16): Code Quality + Verification + IBL ✅ COMPLETED
  [0.1] Dead code cleanup (blit_bind_group + deprecated render()) ✅
  [5.3] MegaLights verification (VERIFIED — gaps documented) ✅
  [5.4] IBL Integration (Group 2 bindings 22-25 + split-sum + material_eval 연결) ✅

Sprint 9 (Week 17-18): Aerial Perspective Wiring + VSM Verification + OIT Integration ✅ COMPLETED
  [13.1] Aerial Perspective HDR 체인 연결 ✅ (Phase 10/11/12/13 HDR 입력에 aerial_output_view 분기 추가)
  [5.2] VSM 전체 파이프라인 검증 ✅ (VERIFIED — mark → allocate → shadow depth → SMRT → material eval 샘플링 완전 연결)
  [8.1] OIT settings + frame init + resolve skeleton ✅ (PARTIAL — transparent mesh source 대기)
  + Stochastic Transparency begin_frame 호출 연결 ✅

Sprint 10 (Week 19-20): GPU Scene Incremental + Material Quality + IBL HDR ✅ COMPLETED
  [2.1] GPU Scene incremental update ✅ (Entity→InstanceId persistent mapping, delta add/remove/update_transform)
  [6.1] Parallax Occlusion Mapping ✅ (GpuMaterial 64→96 bytes, POM ray march with adaptive layers, height_tex_handle)
  [6.2] Clear Coat Material ✅ (Dual-lobe BRDF: base attenuation + coat specular on sun/clustered/IBL)
  [5.4] IBL HDR 환경맵 로딩 ✅ (image crate, load_hdr(), IBLPrefilter.prefilter(), Renderer API)
  Note: [11.1] RDG Pass Registration → Sprint 11 단독 전담 (closure ownership 구조적 문제)

Sprint 11 (Week 21-22): RDG + Advanced Systems
  [11.1] RDG Pass Registration (Phase 0-2 마이그레이션) — 단독 전담 (closure ownership 리팩터)
  [10.1-2] Virtual Texture Streaming + Material Eval 연결
  [4.4] Surface Cache / Mesh Card
  [8.4] Hair Rendering (Marschner + strand physics)
  [14.1] GPU Profiler Overlay
  [3.5] Nanite 통계 Readback
  [6.3-5] Anisotropic BRDF, Energy Conservation, SSS Profile

Future:
  [15.1] Texture Streaming
  [15.2] Water Rendering
  [15.3] VR Support
  [15.4] Ray Tracing
  [15.5] VRS Shader
  [15.6] DDGI RT Pipeline
```

---

## Dependency Graph (Critical Path)

```
[Foundation] ✅
  0.4 Instance Culling Dispatch ✅
    |
    v
  0.3 HZB Ping-Pong ✅
    |
    v
  2.2 Instance Culling Two-Pass ✅ (depends on 0.3 + 2.1)

[Nanite]
  2.1 GPU Scene ────┐
                     ├──> 3.2 Instance-to-Meshlet (needs mesh_id)
  3.1 Normal Cone ──┘
                     |
                     v
              3.3 Nanite HW/SW Merge
                     |
                     v
              3.4 Nanite → Material Eval ✅

[Lumen]
  4.1 SDF Population ✅ ─── (optional) ──> 4.2 Screen Probe Gather ✅
                                               |
                                               v
                                        4.3 Radiance Cache SH ✅
                                               |
                                               v
                                        4.4 Surface Cache
                                               |
                                               v
                                        4.5 Reflection Fallback ✅
                                               |
                                               v
                                        4.5+ Reflections Dispatch ✅

[Outline]
  8.3 Outline Edge+Composite Dispatch ✅ (compute-only path)

[VSM/SMRT]
  5.2+ VSM Shadow Depth ✅ ──> SMRT Dispatch ✅

[Decals]
  0.2 Decal Dispatch ✅ ──> 8.2 DBuffer Material Eval ✅

[Infra]
  12.1 Shader Variants ✅ ──> 셰이더 기반 task 전반에 도움
```

> **Note:**
> - 화살표 방향 = "완료해야 다음 진행 가능". `0.4 → 0.3`은 0.4 완료 후 0.3 착수 가능.
> - 4.1 → 4.2는 **optional dependency**. 4.2는 screen-space trace만으로 독립 시작 가능. SDF trace는 4.1 필요.

---

## UE Reference Quick Lookup

| SKOPE Feature | UE Reference Path |
|---|---|
| Render Loop | `Renderer/SceneRendering.h`, `Renderer/DeferredShadingRenderer.h` |
| RDG | `RenderCore/RenderGraphBuilder.h` |
| Pass System | `Renderer/MeshPassProcessor.cpp`, `Renderer/DepthRendering.h` |
| GPU Scene | `Renderer/GPUScene.h` |
| Base Pass | `Renderer/BasePassRendering.h` |
| Depth Prepass | `Renderer/DepthRendering.h` (EDepthDrawingMode) |
| Nanite | `Renderer/Nanite/` — `NaniteCullRaster.h`, `NaniteVisibility.h` |
| Lumen GI | `Renderer/Lumen/` — `LumenScreenProbeGather.h`, `LumenRadianceCache.h`, `LumenMeshCards.h` |
| Lumen Shaders | `Shaders/Lumen/` — `LumenScreenProbeGather.usf`, `LumenRadianceCacheUpdate.usf` |
| VSM | `Renderer/VirtualShadowMaps/` (9 files) |
| Shadows | `Renderer/ShadowRendering.h`, `Shaders/ShadowProjectionCommon.ush` |
| Clustered | `Renderer/LightRendering.h` |
| BRDF | `Shaders/BRDF.ush`, `Shaders/BxDF.ush` |
| GBuffer | `Shaders/DeferredShadingCommon.ush` |
| SSR | `Renderer/ScreenSpaceRayTracing.h`, `Shaders/SSRT/SSRTReflections.usf` |
| Translucency | `Renderer/TranslucentRendering.h` |
| Decals | `Renderer/DBufferTextures.h`, `Renderer/DecalRenderingCommon.h` |
| Sky | `Renderer/SkyAtmosphereRendering.h`, `Shaders/SkyAtmosphere.usf` |
| Distance Field | `Renderer/GlobalDistanceField.h`, `Renderer/DistanceFieldLightingShared.h` |
| Volumetric Fog | `Renderer/VolumetricFog.h` |
| Volumetric Cloud | `Shaders/VolumetricCloud.usf`, `Shaders/VolumetricCloudCommon.ush` |
| Hair | `Renderer/HairStrands/` |
| Outline | `Renderer/DebugViewModeRendering.h` |
| VT Streaming | `Shaders/VirtualTextureCommon.ush`, `Shaders/VirtualTextureMaterial.usf` |
| Post-Process | `Renderer/PostProcess/` |
| Auto Exposure | `Shaders/PostProcessEyeAdaptation.usf` |
| Tonemapping | `Shaders/PostProcessTonemap.usf` |
| TAA | `Shaders/TAA.ush` |
| SSS | `Shaders/BurleyNormalizedSSSCommon.ush`, `Shaders/SubsurfaceProfileCommon.ush` |
| Blue Noise | `Shaders/BlueNoise.ush` |
| Area Lights | `Shaders/AreaLightCommon.ush`, `Shaders/RectLight.ush` |
| SH | `Shaders/SHCommon.ush` |
| Color Space | `Shaders/ColorSpace.ush` |
| Clear Coat | `Shaders/ClearCoatCommon.ush` |
| Debug | `Renderer/DebugViewModeRendering.h`, `Renderer/ShaderPrint.h` |
| Shader System | `RenderCore/GlobalShader.h` |
| RHI | `RHI/RHI.h`, `RHI/DynamicRHI.h` |

---

## v3 변경 이력

v2 → v3에서 수정된 18건:

**완성도 수정 (6건):**
- bishop Screen Probes: 85% → 95%
- bishop Radiance Cache: 60% → 35%
- bishop SDF: 70% → 40%
- blitz MegaLights: 95% → 75%
- blitz VSM: 95% → 85%
- fianchetto Hair: 80% → 85%

**셰이더 "missing" → "구현됨" 정정 (3건):**
- skope_check: 셰이더 3개 구현됨 → Task를 "dispatch 연결 + 검증"으로 변경
- skope_promotion: 셰이더 2개 구현됨 → Task를 "검증 + Disk I/O"로 변경
- skope_fianchetto: 셰이더 7개 구현됨 → Task를 "통합 검증 + Marschner + physics"로 변경

**UE Reference 경로 수정 (14건):**
- `NaniteCull.h` → `NaniteCullRaster.h`
- `NaniteVisibility.ush` → `NaniteVisibility.h`
- `LumenScreenProbeGathering.h` → `LumenScreenProbeGather.h`
- `LumenScreenProbeGather.ush` → `LumenScreenProbeGather.usf`
- `LumenSurfaceCache.h` → `LumenSurfaceCacheFeedback.h`
- `SubsurfaceProfile.ush` → `SubsurfaceProfileCommon.ush`
- `ScreenSpaceReflections.ush` → `SSRT/SSRTReflections.usf` + `ScreenSpaceReflectionTileCommons.ush`
- `ParallaxOcclusionMapping.ush` → 삭제 (미존재), `BasePassPixelShader.usf`로 대체
- `Platform.ush` → 삭제 (미존재)
- `VirtualTexturing/` (Renderer) → 삭제 (미존재)
- `Shaders/VirtualTexturing/` → `VirtualTextureCommon.ush` + `VirtualTextureMaterial.usf`
- `Shaders/SkyAtmosphere/` → `SkyAtmosphere.usf` + `SkyAtmosphereCommon.ush`
- `Shaders/VolumetricCloud/` → `VolumetricCloud.usf` + `VolumetricCloudCommon.ush`
- `MeshPassProcessor.h` → `MeshPassProcessor.cpp`

**누락 모듈 추가 (7건):**
- Completed Systems에 추가: `debug_viz.rs`, `decals.rs`, `distance_field.rs`, `vrs.rs`, `ddgi.rs`, `vbuffer_resolve.rs`, `skope_resource`
- 새 Task 0.4: Instance Culling Dispatch 미연결 [CRITICAL]

**Sprint 계획 수정 (4건):**
- Sprint 1에 [0.4] Instance Culling dispatch + [2.1] GPU Scene 추가
- Task 0.3에 "Depends on: 0.4" 추가
- Dependency Graph에서 4.1→4.2를 optional dependency로 명시 (모순 해소)
- Phase 번호 정리: MegaLights classify → Phase 3 내, denoise → Phase 9.7

**기타 수정 (2건):**
- IBL Task 5.4: "GPU prefilter 미구현" → "구현됨 (182줄), irradiance convolution 검증 필요"
- Pipeline Overview 다이어그램: Phase 번호 정리, MegaLights 위치 명시

**v3.1 → v3.2 Sprint 1 완료 + 검토 수정 (6+2건):**
- [0.2] Decal Dispatch → DONE (Phase 2.7에서 dispatch, `update_decals()` API 추가)
- [0.3] HZB Ping-Pong → DONE (`prev_hzb_texture`/`swap_history()` 추가)
- [0.4] Instance Culling Dispatch → DONE (`render_phase_instance_culling()`에서 `cull()` 호출)
- [1.1] render_vbuffer() 분리 → DONE (10개 phase 함수로 분리)
- [7.1] GTAO/Contact Shadow 순서 → DONE (Phase 5↔6 교환)
- [2.1] GPU Scene 인스턴스 매핑 → PARTIAL (per-frame rebuild 연동. incremental update 잔여)
- Pipeline Overview: Phase 5/6 순서 갱신 (GTAO first, Contact second)
- Completed Systems: HZB notes 업데이트, DBuffer Decals "미dispatch" 제거
- Dependency Graph: Foundation 블록 ✅ 마킹, 다음 sprint 방향 표시

**검토 후 버그 수정 (2건):**
- `hzb.rs` `swap_history()`: mip 0만 복사 → 전체 mip chain 복사로 수정 (coarse occlusion test에 필요)
- `gpu_scene.rs` `upload()`: `None` 슬롯 skip → zeroed GpuInstance write로 수정 (stale data 방지)

**v3.2 → v3.3 Sprint 2 완료:**
- [3.1] Normal Cone 계산 → DONE (`meshlet.rs`에 `compute_normal_cone()` 구현 + 4 unit tests)
- [3.2] Instance-to-Meshlet 매핑 → DONE (`MeshMeshletRange` + 2D dispatch + WGSL 연동)
- [3.3] Nanite HW/SW V-Buffer Merge + Renderer 통합 → DONE (`render_phase_nanite()` Phase 2.1)
- [2.2] Instance Culling Two-Pass 인프라 → DONE (`cull_pass1()` + `occluded_indices` buffer + WGSL pass_index 분기)
- Cargo.toml: `skope_virtual_geometry` `gpu` feature 활성화
- Pipeline Overview: Phase 2.1 (Nanite) 추가
- render_vbuffer 분리 목록: `render_phase_nanite()` 추가
- Completed Systems 테이블: Instance Culling, Nanite Cull/HW/SW/V-Buffer 항목 추가
- wgpu 28 mesh shader fix: `MESH_SHADER` → `MESH`, `TASK_SHADER` → `TASK` (rasterize.rs)

**v3.3 → v3.4 Sprint 3 완료:**
- [3.4] Nanite → Material Eval 연결 → DONE
  - `NaniteFullVertex` 64-byte struct (types.rs) + `NaniteMesh.vertex_data` 필드 (meshlet.rs)
  - `upload_nanite_meshes()` full vertex buffer 업로드 (fallback 생성 포함)
  - Group 0 depth: `texture_depth_2d` → `texture_2d<f32>` (merged resolve R32Float)
  - Group 1 bindings 3-6: Nanite vertices/triangles/meshlets/instances
  - `material_eval.wgsl` NANITE_FLAG(bit 31) 분기 + meshlet vertex fetch + barycentric interpolation
- [5.1] PCSS 활성화 → DONE
  - `pcss_blocker_search()` 8-sample Poisson + `pcss_shadow()` variable-radius PCF
  - `sample_csm_shadow()` pcss_enabled 분기
- [8.2] DBuffer Material Eval 연결 → DONE
  - Group 2 bindings 19-21: DBuffer albedo/normal/roughness
  - `set_dbuffer_resources()` + alpha-based compositing in WGSL
- Completed Systems 업데이트: Material Eval 95→100%, Nanite V-Buffer 90→100%, CSM PCSS 활성화, DBuffer Material Eval 합성
- Pipeline Overview: Phase 2.7 DBuffer → Material Eval 연결 명시

**v3.4 → v3.5 Sprint 4 완료 — Lumen Phase 1:**
- [4.1] SDF Volume Population → DONE
  - `sdf_voxelize.wgsl` (NEW): GPU Scene bounding sphere → signed distance, workgroup(4,4,4)
  - `distance_field.rs`: `VoxelizeParams` + `voxelize_pipeline` + `voxelize()` method
  - `renderer.rs` `render_phase_auxiliary()`: `enable_lumen_gi` guard 추가, voxelize dispatch
- [4.2] Screen Probe Gather 실제 radiance + 전체 Renderer 통합 → DONE
  - `lumen_screen_probe_gather.wgsl`: prev_hdr(5) + hdr_sampler(6) binding, screen/SDF hit → prev HDR fetch
  - `screen_probe.rs`: gather_input_layout binding 5-6 추가, place depth `Depth` → `Float { filterable: true }`
  - `types.rs`: 6개 새 Rust struct (PlaceCameraData, GatherCameraData, GatherParams, FilterParams, LumenCompositeParams)
  - `renderer.rs`: Lumen fields (15개) 추가, `new()` 초기화, `resize()` grid 갱신, Phase 8.5 dispatch (Place→Gather→Filter→Composite)
  - Composite pipeline: `lumen_composite.wgsl` bind group 동적 생성 + dispatch
- Bug fixes:
  - DDGI normal binding: `depth_view` 중복 전달 → `normal_roughness_view` 수정
  - Placement depth type: `Depth` sample type → `Float { filterable: true }` (merged R32Float)
- `Cargo.toml`: `skope_bishop` `gpu` feature 활성화
- Pipeline Overview: Phase 8.5 (Lumen Screen Probes) 추가
- Completed Systems: Distance Field 40% → 70%, DDGI normal fix 기록

**v3.5 검토 후 버그 수정 (3건):**
- ScreenProbe 구조체 정렬 수정 (`types.rs`):
  - WGSL `vec3<f32>` 16바이트 정렬 vs Rust `[f32; 3]` 4바이트 정렬 → 40 bytes vs 64 bytes 불일치
  - Rust `ScreenProbe`에 `_align0: [u32; 2]`, `_align1: u32`, `_align2: [u32; 3]` 패딩 추가 → 64 bytes로 일치
  - 버퍼 크기 `size_of::<ScreenProbe>()` 기반이므로 자동 반영
- Composite 텍스처 충돌 수정 (`renderer.rs`, `lumen_composite.wgsl`):
  - G2 (StorageTexture ReadWrite) + G3 (Sampled Texture)에 동일한 `material_eval.output_view` 바인딩 → wgpu validation error
  - Albedo GBuffer 미존재 → G3 (albedo) bind group 삭제, 셰이더를 3 bind group으로 단순화
  - `irradiance * albedo / PI` → `irradiance / PI * gi_intensity`로 변경 (albedo 모듈레이션은 Phase 2에서 albedo GBuffer 도입 시)
- Gather 셰이더 NaN 가드 추가 (`lumen_screen_probe_gather.wgsl`):
  - Placement가 sky 픽셀 건너뛰기 → 미배치 probe의 `normal = [0,0,0]` → `hemisphere_direction()`에서 `normalize(cross(up, [0,0,0]))` = NaN
  - `if all(probe.normal == vec3(0.0)) { return; }` 가드 추가

**v3 → v3.1 내부 일관성 수정 (9건):**
- Completed Systems 테이블 헤더: "이미 완성되어 작동 중" → "구현 현황" (부분 구현 항목 포함이므로)
- `distance_field.rs` 테이블: 70% → 40% (Task 4.1과 일치시킴)
- DDGI 테이블 Notes: probe cascade 명확화 + Phase 15.6 참조 추가
- Dependency Graph: ASCII 아트 전면 재작성 (0.4→0.3 방향 명확화, 카테고리 분리)
- Sprint 2에 [2.2] Instance Culling Two-Pass 추가
- Sprint 3에 [3.4] Nanite → Material Eval 연결 추가
- Sprint 6에 [1.3] View 시스템 도입 추가
- Sprint 8+에 [3.5] Nanite 통계 Readback 추가
- Quick Lookup에 Distance Field 항목 추가
- Pipeline Overview에 [Phase 12.5] OIT Composite 추가
- Phase 번호 체계 충돌 주석 추가 (Phase 4.x pipeline vs Task 4.x Lumen)

**v3.5.1 → v3.6 Sprint 5 완료 — Post-Processing + Lumen Phase 2:**
- [9.2] Color Grading 연결 → DONE
  - `color_grading.rs`: `execute()` 메서드 추가 (compute dispatch, bind group 동적 생성)
  - `pipeline.rs`: `PostProcessPipeline::new()`에서 identity LUT 32³ 업로드
  - `execute_internal()`: tonemapping → color grading → film effects 체인 연결
  - `get_final_output_view()`: color grading 분기 추가
- [9.1] Auto Exposure 연결 → DONE
  - `auto_exposure.rs`: `execute()` 메서드 추가 (histogram + average 2-pass dispatch)
  - `pipeline.rs`: `auto_exposure` 필드 + `auto_exposure_enabled` config 추가 (lib.rs + pipeline.rs 양쪽)
  - `tonemapping.rs`: binding 5 (ExposureResult storage), `default_exposure_buffer` (mapped_at_creation zero-init), `execute()` 시그니처에 `exposure_buffer: Option<&Buffer>` 추가
  - `tonemapping.wgsl`: `ExposureResult` struct + `auto_exposure.current_exposure > 0` 시 자동 노출 사용
  - `execute()` / `execute_with_gbuffer()` / `execute_internal()`: `queue: &wgpu::Queue` 파라미터 추가
  - `renderer.rs`: post_process 호출부 `queue` 전달
- [4.3] Radiance Cache SH Update → DONE
  - `lumen_radiance_cache_sh_update.wgsl` (NEW 218줄): L2 SH 인코딩 compute shader
    - @workgroup_size(64), cache probe → screen 투영 → 2x2 bilinear screen probe 샘플링
    - 9 SH basis functions (Y_0^0 ~ Y_2^2) → irradiance × basis × weight 누적
    - Temporal blend: `mix(old_sh, new_sh, temporal_speed)`, validity tracking
    - Off-screen probes: validity × 0.95 감쇠
  - `types.rs`: `SHUpdateParams` struct (128 bytes, 16 fields)
  - `radiance_cache.rs`: `SHUpdatePipeline` struct (3 bind group layouts: params/screen_data/cache_data)
  - `lib.rs`: `SHUpdatePipeline` export 추가
  - `renderer.rs`: `lumen_radiance_cache` / `lumen_radiance_cache_gpu` / `lumen_sh_update_pipeline` / `lumen_sh_update_params_buf` 필드 추가 + Phase 8.5.5 dispatch
  - Round-robin: `update_range()` ~6%/frame (total/16, min 64)
- Pipeline Overview: Phase 8.5.5 + Phase 13 chain 업데이트
- Completed Systems: Auto Exposure 100%, Color Grading pipeline 연결, Post-Process chain 업데이트
- 버그 수정: `default_exposure_buffer` mapped_at_creation: false → true + fill(0) + unmap() (garbage 값 방지)

**v3.6 → v3.7 Sprint 6 완료 — Shader Variants + Reflection Fallback + View System:**
- [12.1] Shader Variant System → DONE
  - `preprocessor.rs`: `defines` 필드, `with_defines()`, `define()`, `undefine()` API
  - `#define`/`#ifdef`/`#ifndef`/`#else`/`#endif` 전처리 지시문 (중첩 지원, 부모 active 보존)
  - 매크로 값 치환: 단어 경계 체크 (`replace_word_boundary()`)
  - `PreprocessError` 확장: `UnmatchedEndif`, `UnmatchedElse`, `UnclosedIfdef`
  - `manager.rs`: `global_defines` + `set_define()`/`remove_define()` API
  - 13개 신규 단위 테스트 (16 total, all pass)
- [4.5] Reflection Radiance Cache Fallback → DONE
  - `reflections.rs`: `ReflectionParams` 확장 (`grid_size`, `probe_spacing`, `cache_origin`, `_pad: u32`)
  - `lumen_reflections_trace.wgsl`: 전면 리팩터
    - `RadianceCacheProbe` struct → Rust layout 일치 (was: separate sh_r/sh_g/sh_b arrays)
    - Binding 5: `array<vec4<f32>>` → `array<RadianceCacheProbe>`
    - L2 SH evaluation (`sh_basis`, `evaluate_sh`) + trilinear interpolation (`world_to_grid`, `sample_radiance_cache`)
    - Miss case: `vec4(0.0)` → radiance cache SH 평가, w=0.5 (cache hit marker)
- [1.3] View System 도입 → DONE
  - `renderer/types.rs`: `FrameView` struct + `FrameView::new()`
  - `render_vbuffer()`: FrameView 생성, 9개 phase 함수에 `&FrameView` 전달
  - `camera_pos` 추출 중복 제거 (5회 → 0회)
- Pipeline Overview: 변경 없음 (phase 구조 동일, 내부 시그니처만 변경)
- Completed Systems: Shader Preprocessor 100% notes 업데이트
- Dependency Graph: 12.1 ✅, 4.5 ✅ 마킹

**v3.7 → v3.8 Sprint 7 완료 — Lumen Reflections Dispatch + Outline + VSM Shadow Depth + SMRT:**
- [4.5+] Lumen Reflections Dispatch → DONE (Sprint 6 closure)
  - `lumen_reflections: Option<LumenReflectionsPipeline>` field in Renderer
  - `new()`: conditional init (enable_lumen_gi guard)
  - Phase 8.5.6 dispatch: `trace()` → `temporal_filter()` → `swap_history()`
  - Bindings: depth_view (Depth), normal_roughness_view (Float), hzb_view (Float), output_view (Float), probe_buffer (Storage)
  - `resize()` 연결
- [8.3] Outline Edge Detection + Composite → DONE (compute-only path)
  - `enable_outline: bool` in `RenderSettings` + Default (false)
  - `OutlinePipeline` + `OutlineBuffers` + `dummy_r32float_view` (1x1 R32Float placeholder) fields
  - Phase 8.7 dispatch: hull clear → edge detection (8x8) → composite (8x8)
  - Edge detect bindings: merged_depth_view + normal_roughness_view + dummy_r32float (model_id) + edge_mask_view (storage) + edge_params
  - Composite bindings: output_view + hull_view + edge_mask_view + dummy_r32float + outline_view (storage) + composite_params
  - `edge_use_object_id = 0` (no model ID in compute-only path)
  - `resize()` 연결
- [5.2+] VSM Shadow Depth + SMRT Dispatch → DONE
  - `vsm_shadow_depth.wgsl` (NEW): depth-only vertex shader, `VsmShadowUniforms` (light_view_proj)
  - `VirtualShadowMap`: `shadow_depth_pipeline` + `shadow_depth_bind_group_layout` + `shadow_depth_uniform_buffer` fields
  - `render_shadow_depth()`: depth-only render pass into `physical_pool_depth_view`, front-face cull, depth bias
  - Vertex layout: stride 64 (GpuVertex), position at offset 0 (Float32x3)
  - `smrt: Option<SmrtPipeline>` field in Renderer, conditional init (enable_vsm guard)
  - Dispatch in `render_phase_shadows()`: mark → allocate → shadow depth → SMRT trace → material_eval
  - `resize()` 연결
- Pipeline Overview: Phase 2.5 업데이트 (VSM + Shadow Depth + SMRT), Phase 8.5.6 + Phase 8.7 추가
- Sprint roadmap: Sprint 7 ✅ COMPLETED 마킹

**v3.8 → v3.9 Sprint 8 완료 — Dead Code Cleanup + MegaLights Verification + IBL Integration:**
- [0.1] Dead Code Cleanup → DONE
  - `self.blit_bind_group` 필드 제거
  - deprecated `render()` 함수 삭제
  - `new()` 초기화 코드 제거 (create_blit_bind_group 호출 + Self block field)
  - `resize()` 재생성 코드 제거
  - 보존: `blit_bind_group_layout`, `blit_pipeline`, `blit_sampler`, `blit_params_buffer`, `create_blit_bind_group()` — 활성 경로 사용
- [5.3] MegaLights Verification → VERIFIED
  - 전체 파이프라인 연결 확인: Classify → Sample → Denoise → Material Eval (Group 2 bindings 17-18)
  - 3개 갭 문서화: VisibleLightHash 미구현, temporal reuse 미구현, RIS winner shadow 미구현
- [5.4] IBL Integration → DONE
  - `material_eval.rs`: Group 2 bindings 22-25 (prefiltered cube, irradiance cube, BRDF LUT, sampler)
  - dummy/active 리소스 패턴 (기존 clustered/shadows/DDGI/VSM/MegaLights 패턴 준수)
  - `set_ibl_resources()` + `rebuild_group2()` 연동
  - Pipeline layout: 4 bind groups 유지 (Group 2 통합)
  - `material_eval/types.rs`: `ibl_intensity: f32` 추가, `_pad2` 7→6 (default: 0.3)
  - `material_eval.wgsl`: `@group(2) @binding(22-25)` + `sample_ibl()` split-sum + ambient fallback 조정
  - `ibl.rs`: `prefiltered_view()`, `irradiance_view()`, `brdf_lut_view()`, `sampler()` getter 추가
  - `renderer.rs`: `IBLEnvironment` import + field + `new()` 초기화 (256 cube_size) + `set_ibl_resources()` 연결
  - 2개 `update_lighting` 호출부: `ibl_intensity: 0.3` + `_pad2: [0; 6]` 수정
  - Note: 초기 Group 4 접근 → `max_bind_groups=4` 제한으로 Group 2 통합 결정
- Completed Systems: IBL 항목 추가 (90%)
- Sprint roadmap: Sprint 8 ✅ COMPLETED 마킹, Sprint 9+ 업데이트
- **Runtime Bug Fixes (Sprint 8 추가):**
  - `tonemapping.rs`: `MAP_WRITE | STORAGE` 불가 → `MAP_WRITE` 제거 (`mapped_at_creation` 유지)
  - `pipeline.rs`: `ColorGradingParams` 버퍼 미초기화 → `default()` 값 write 추가 (검정색 뷰포트 원인)
  - `material_eval.rs`: Dummy IBL 텍스처 `COPY_DST` 추가 + zero-fill (NaN 방지)

---

## Notes

- 모든 UE Reference base: `C:\Users\Cheshire\Documents\GitHub\SKOPE\reference\UE_RenderPipeline\`
- wgpu 28 기준. Mesh shader: EXPERIMENTAL_MESH_SHADER
- DDGI chicken-and-egg: DDGI는 이전 프레임 material_eval output을 읽으므로 정상. Temporal lag 1 frame은 의도된 설계.
- 각 Task 완료 후 이 문서 Status 업데이트할 것.
- Sprint 순서는 의존성 기반. Dependency Graph 참조.
- `skope_resource` 크레이트는 렌더 인프라 (BufferPool, PipelineCache, StagingBelt). 직접적 TODO task 없지만 RDG migration (11.1) 시 활용됨.
