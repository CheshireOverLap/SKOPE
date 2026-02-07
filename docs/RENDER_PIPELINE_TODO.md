# SKOPE Render Pipeline TODO

> **Last Updated:** 2026-02-07 (Sprint 4 — Lumen Phase 1 — v3.5.1)
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
[Phase 2.5]  Shadow Maps (CSM / VSM)
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
  |
  v
[Phase 9]    Volumetric Fog + SS Composite + Aerial Perspective
[Phase 9.7]  MegaLights Denoise
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
[Phase 13]   Post Processing (Bloom + Tonemapping + Film)
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
| Material Eval | `material_eval.rs` | 100% | Cook-Torrance PBR, bindless 4096 slots, 4 bind groups, Nanite/Standard 분기, DBuffer 합성 |
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
| CSM | `skope_blitz/shadows.rs` | 100% | 4-cascade, 2048px, PCSS 활성화 (blocker search + variable PCF) |
| Clustered Lighting | `skope_blitz/clustered.rs` | 100% | 16x16x24 grid, CPU fallback |
| BRDF | `skope_blitz/brdf.rs` | 100% | Cook-Torrance + BRDF LUT 512x512 |
| Light Manager | `skope_blitz/lights.rs` | 100% | Point/Spot/Rect/Disk + attenuation |
| Light Probes | `skope_blitz/light_probes.rs` | 100% | SH9 + trilinear interpolation |
| Character Lighting | `skope_blitz/character_lighting.rs` | 100% | Fill/Rim/Face shadow/SSS/Hair |
| LOD Selector | `lod.rs` | 100% | Bounding sphere, screen-space error |
| HLOD | `hlod.rs` | 100% | Cluster organization |
| Shadow Atlas | `shadow_atlas.rs` | 100% | Tile allocator for local lights |
| OIT | `oit.rs` | 85% | Per-pixel linked list build + resolve |
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
| Nanite Cull | `skope_gambit/cull.rs` | 90% | 2D dispatch, normal cone, mesh_ranges 매핑 완성. Renderer 통합 완료 |
| Nanite HW Raster | `skope_gambit/rasterize.rs` | 90% | Mesh shader pipeline 완성 + Renderer 통합 |
| Nanite SW Raster | `skope_gambit/rasterize.rs` | 90% | Compute rasterizer + atomicMin depth. Renderer 통합 완료 |
| Nanite V-Buffer | `skope_gambit/visibility.rs` | 100% | HW/SW merge via vbuffer_resolve. Material Eval Nanite 분기 연결 완료 |
| Distance Field | `distance_field.rs` | 70% | GDF volume + voxelization compute (bounding sphere SDF). Mesh SDF = Phase 2 |
| VRS Classify | `vrs.rs` | 50% | Pipeline 완성, 셰이더 stub |
| DDGI | `ddgi.rs` + `ddgi/` | 60% | 3-level probe cascade (2m/8m/32m) 완성. Normal binding 버그 수정 (Sprint 4). Ray tracing/irradiance update pipeline stub (Phase 15.6) |
| Bloom | `skope_endgame/bloom.rs` | 100% | 13-tap Karis, 7 mips |
| Tonemapping | `skope_endgame/tonemapping.rs` | 100% | ACES/Reinhard/Hable/AgX/Hejl |
| Color Grading | `skope_endgame/color_grading.rs` | 100% | 3D LUT + Lift/Gamma/Gain |
| Film Effects | `skope_endgame/film_effects.rs` | 100% | Grain + Vignette |
| Post-Process Pipeline | `skope_endgame/pipeline.rs` | 100% | Full chain (see 0.1 note) |
| Shader Preprocessor | `shaders/preprocessor.rs` | 100% | #include with cycle detection |
| Shader Manager | `shaders/manager.rs` | 100% | Hot-reload (Debug), embedded (Release) |
| Pipeline Manager | `shaders/pipeline_manager.rs` | 100% | Auto-rebuild on shader change |
| GPU Resource Pool | `skope_resource` | 100% | PipelineCache, BufferPool, StagingBelt, BindGroupLayoutCache |

---

## Phase 0: Cleanup & Correctness

### 0.1 Dead Blit Bind Group 제거 [LOW]
- **Problem:** `renderer.rs:495-504`와 `resize():858-867`에서 `self.blit_bind_group`을 `material_eval.output_view`로 초기화하면서 TODO 주석이 남아있음.
- **Reality:** 실제 렌더 경로에서는 `render_blit_with_source()` (line 1721)가 **매 프레임 동적 bind group을 생성**하여 `post_output`을 사용. `self.blit_bind_group`은 deprecated `render()` 함수(line 1036)에서만 사용되며 **active 렌더 경로에서는 dead code**.
- **Evidence:**
  - `render_vbuffer():1636` → `post_process.execute()` 호출 (정상)
  - `render_vbuffer():1659` → `render_blit_with_source(device, encoder, output_view, post_output)` (정상)
  - `render_blit_with_source():1729` → `Self::create_blit_bind_group()` 동적 생성 (정상)
  - `render():1036` → deprecated 함수에서 `self.blit_bind_group` 사용 (레거시)
- **Fix:** `self.blit_bind_group` 필드, deprecated `render()` 함수, TODO 주석 모두 제거.
- **Impact:** 기능적 영향 없음. 코드 정리 수준.

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

### 1.3 View 시스템 도입 [MEDIUM]
- **Problem:** 카메라/뷰 정보가 함수 매개변수로 흩어져 있음
- **UE Reference:**
  - `reference/UE_RenderPipeline/Renderer/SceneRendering.h` - FViewInfo 클래스
- **Approach:**
  1. `ViewInfo` 구조체 (view/proj, jitter, visibility map, near/far 등)
  2. 프레임당 ViewInfo 생성 → 모든 패스에 전달
  3. 다중 뷰 지원 준비 (split-screen, VR)

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

## Phase 3: Nanite (skope_gambit) 완성

### 3.1 ~~Normal Cone 계산 구현~~ [DONE — Sprint 2]
- **Completed:** Sprint 2. `meshlet.rs`에 `compute_normal_cone()` 구현.
  - Face normal 계산 (cross product) → 평균 normal → cone axis (정규화)
  - 각 normal과 axis 사이 dot 최솟값 → `cos(half_angle)`
  - Degenerate (zero area tri, 반구 이상 spread) → `cos = -1.0` (backface cull 비활성화)
  - 4개 단위 테스트 추가 (`cargo test -p skope_gambit` 통과)

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

### 4.3 Radiance Cache SH Update Shader [HIGH]
- **Current:** ~35%. RadianceCacheProbe storage buffer + GPU 버퍼 관리 있음. SH 업데이트 compute shader **완전 미구현**.
- **UE Reference:**
  - `reference/UE_RenderPipeline/Renderer/Lumen/LumenRadianceCache.h`
  - `reference/UE_RenderPipeline/Shaders/Lumen/LumenRadianceCacheUpdate.usf`
- **Tasks:**
  1. Per-probe radiance sampling → SH encoding compute shader
  2. L2 SH 계수 (9 bands x RGB) 누적
  3. Temporal blending (이전 프레임 SH와 혼합)
  4. Validity/convergence tracking

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

### 4.5 Reflection Radiance Cache Fallback [LOW]
- **Problem:** `lumen_reflections_trace.wgsl:144` - radiance cache fallback이 zero.
- **Depends on:** 4.3 (SH update)
- **Tasks:**
  1. Radiance cache SH evaluation (반사 방향으로 SH 샘플링)
  2. Temporal blend factor 적응형으로 변경

---

## Phase 5: Lighting System 강화

### 5.1 ~~PCSS (Percentage Closer Soft Shadows) 활성화~~ [DONE — Sprint 3]
- **Completed:** Sprint 3. `material_eval.wgsl`에 PCSS 구현.
  - `pcss_blocker_search()`: 8 Poisson sample로 blocker 평균 depth 검색 (search_radius = light_size * texel_size * 20)
  - `pcss_shadow()`: blocker depth → penumbra 크기 계산 → variable-radius PCF (clamp 1~8)
  - `sample_csm_shadow()`: `shadow_uniforms.pcss_enabled` 분기 — PCSS 활성 시 `pcss_shadow()`, 비활성 시 기존 `pcf_shadow()`
  - Rust 변경 불필요 — `pcss_enabled`/`pcss_light_size`는 이미 `ShadowUniforms`에 업로드됨

### 5.2 VSM WGSL Shader 검증 [HIGH]
- **Current:** ~85%. CPU-side 파이프라인 완성 (page table, physical pool, clipmap, cache, SMRT). 셰이더 존재 (`vsm_mark_pages.wgsl`, `vsm_allocate.wgsl`, `vsm_sampling.wgsl`, `smrt.wgsl`). 통합 검증 필요.
- **UE Reference:**
  - `reference/UE_RenderPipeline/Renderer/VirtualShadowMaps/VirtualShadowMapArray.h`
  - `reference/UE_RenderPipeline/Renderer/VirtualShadowMaps/VirtualShadowMapClipmap.h`
  - `reference/UE_RenderPipeline/Shaders/VirtualShadowMaps/`
- **Tasks:**
  1. Page marking compute shader 검증
  2. Page allocation compute shader 검증
  3. Shadow depth rendering 검증
  4. SMRT 셰이더 검증
  5. Material eval VSM 샘플링 연결 검증

### 5.3 MegaLights WGSL Shader 검증 [MEDIUM]
- **Current:** ~75%. 파이프라인 구조 완성. 셰이더 존재 (`megalights_classify.wgsl`, `megalights_sample.wgsl`, `megalights_denoise.wgsl`). RIS 정확성 + denoiser 품질 미검증.
- **UE Reference:**
  - `reference/UE_RenderPipeline/Renderer/MegaLights/` (있다면)
  - `reference/UE_RenderPipeline/Shaders/MegaLights/`
- **Tasks:**
  1. Tile classification compute 셰이더 검증
  2. RIS reservoir sampling 셰이더 검증
  3. Spatiotemporal denoiser 셰이더 검증
  4. Visible light hash 업로드 검증

### 5.4 IBL Irradiance Convolution + 통합 검증 [MEDIUM]
- **Current:** ~85%. CPU equirectangular 변환 구현. GPU prefilter 셰이더 **구현됨** (`ibl_prefilter.wgsl`, 182줄 — importance sampling GGX, Hammersley 시퀀스, cube direction mapping). Irradiance convolution 통합 미검증.
- **UE Reference:**
  - `reference/UE_RenderPipeline/Renderer/ReflectionEnvironmentCapture.h`
  - `reference/UE_RenderPipeline/Shaders/ReflectionEnvironmentShaders.usf`
- **Tasks:**
  1. Specular prefilter 검증 (roughness mip별 결과 확인)
  2. Irradiance convolution compute 검증 (cosine-weighted hemisphere)
  3. Material eval에서 IBL 샘플링 연결 확인

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

### 8.1 OIT Resolve 셰이더 연결 검증 [MEDIUM]
- **Current:** ~85% (445줄). Build pipeline, resolve pipeline, bind groups, clear/resize **모두 구현 완료**.
- **Remaining:** 실제 씬에서 투명 오브젝트 렌더링 시 resolve 정확도 검증.
- **UE Reference:**
  - `reference/UE_RenderPipeline/Renderer/TranslucentRendering.h`
  - `reference/UE_RenderPipeline/Renderer/TranslucentLighting.h`
- **Tasks:**
  1. Linked list resolve 동작 검증 (depth sort + blend)
  2. Max node 초과 시 fallback (closest N fragments)
  3. 반투명 오브젝트 라이팅 (clustered forward)

### 8.2 ~~Decal DBuffer Material Eval 연결~~ [DONE — Sprint 3]
- **Completed:** Sprint 3. DBuffer → Material Eval 전체 연결.
  - Group 2 bindings 19-21: `dbuffer_albedo` (Rgba8Unorm), `dbuffer_normal` (Rgba8Snorm), `dbuffer_roughness` (Rgba8Unorm) 추가
  - Dummy DBuffer 텍스처 + active resource tracking 패턴 적용
  - `set_dbuffer_resources()` 메서드: `render_phase_material_eval()`에서 DBuffer view 전달
  - `material_eval.wgsl`: material sampling 후 DBuffer 합성 — alpha 기반 albedo/normal/roughness mix
  - `render_settings.enable_decals` guard로 조건부 활성화

### 8.3 Outline Rendering (skope_check) Dispatch 연결 [MEDIUM]
- **Current:** ~90%. Hull pipeline + Edge detection pipeline + Composite pipeline 구조 완성. 4 presets (Default/Cute/Serious/Boss). Normal smoothing, per-part controls 구현. **WGSL 셰이더 3개 모두 구현됨** (`outline_hull.wgsl` 105+줄, `outline_edge_detect.wgsl`, `outline_composite.wgsl`).
- **Missing:** Main renderer에서 dispatch 연결 + 실제 렌더링 검증.
- **UE Reference:**
  - `reference/UE_RenderPipeline/Renderer/DebugViewModeRendering.h` - Wireframe/outline
- **Tasks:**
  1. Hull expansion 셰이더 정확도 검증 (screen-space clamping, distance fade)
  2. Edge detection 셰이더 정확도 검증 (Sobel on depth/normal/ID)
  3. Composite 셰이더 정확도 검증
  4. Render dispatch 메서드 구현
  5. Main renderer에 outline pass 연결

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

### 9.1 Auto Exposure 연결 [HIGH]
- **Problem:** `skope_endgame/auto_exposure.rs` 존재. 파이프라인에 미연결.
- **UE Reference:**
  - `reference/UE_RenderPipeline/Shaders/PostProcessHistogramCommon.ush`
  - `reference/UE_RenderPipeline/Shaders/PostProcessEyeAdaptation.usf`
- **Tasks:**
  1. Histogram compute shader (HDR luminance 분포)
  2. Average luminance (가중 평균)
  3. Temporal smoothing
  4. Exposure → tonemapping 전달

### 9.2 Color Grading LUT 연결 검증 [MEDIUM]
- **Problem:** 구현 완료 (3D LUT, Lift/Gamma/Gain). 실사용 검증 필요.
- **UE Reference:** `reference/UE_RenderPipeline/Shaders/PostProcessCombineLUTs.usf`
- **Tasks:**
  1. .cube 파일 로딩 테스트
  2. 런타임 LUT 전환
  3. UI에서 Lift/Gamma/Gain 연결

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

### 12.1 Shader Variant / Conditional Compilation [MEDIUM]
- **Problem:** 현재 #include만 지원. `#ifdef`, `#define` 매크로 없음. 모든 변형이 별도 셰이더 파일.
- **UE Reference:**
  - `reference/UE_RenderPipeline/RenderCore/GlobalShader.h` - Permutation system
- **Tasks:**
  1. `preprocessor.rs`에 `#define`, `#ifdef`, `#ifndef`, `#else`, `#endif` 추가
  2. Permutation 키 시스템 (e.g., `ENABLE_DDGI=1`, `USE_NANITE=1`)
  3. 런타임 셰이더 변형 캐싱

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

### 13.1 Aerial Perspective 검증 [MEDIUM]
- **Current:** ~80%. Transmittance, multi-scatter, sky view LUTs 구현.
- **UE Reference:**
  - `reference/UE_RenderPipeline/Renderer/SkyAtmosphereRendering.h`
  - `reference/UE_RenderPipeline/Shaders/SkyAtmosphere.usf`
  - `reference/UE_RenderPipeline/Shaders/SkyAtmosphereCommon.ush`
- **Tasks:**
  1. LUT 정확도 검증 (ground truth 비교)
  2. Aerial perspective fog 적용 검증
  3. Sun disk 렌더링

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

Sprint 5 (Week 9-10): Lumen Phase 2 + Post
  [4.3] Radiance Cache SH Update
  [9.1] Auto Exposure 연결
  [9.2] Color Grading 연결

Sprint 6 (Week 11-12): Architecture + Infrastructure
  [11.1] RDG 패스 등록 시작
  [12.1] Shader variant system
  [1.2] Pass-Based Rendering 기초
  [1.3] View 시스템 도입

Sprint 7 (Week 13-14): Special Rendering
  [8.1] OIT 검증
  [8.3] Outline rendering dispatch 연결
  [8.4] Hair rendering 통합 검증

Sprint 8+ (Ongoing): Polish & Advanced
  [10.x] Virtual Texture Streaming (셰이더 검증 + Disk I/O)
  [6.x] Material 기능 추가 (POM, Clear Coat)
  [4.4] Surface Cache
  [5.3] MegaLights 셰이더 검증
  [5.4] IBL 통합 검증
  [3.5] Nanite 통계 Readback
  [14.x] Debug/Profiling
  [0.1] Dead code 정리
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
                                        4.3 Radiance Cache SH
                                               |
                                               v
                                        4.4 Surface Cache
                                               |
                                               v
                                        4.5 Reflection Fallback

[Decals]
  0.2 Decal Dispatch ✅ ──> 8.2 DBuffer Material Eval ✅

[Infra]
  12.1 Shader Variants ──> 셰이더 기반 task 전반에 도움
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
- Cargo.toml: `skope_gambit` `gpu` feature 활성화
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

---

## Notes

- 모든 UE Reference base: `C:\Users\Cheshire\Documents\GitHub\SKOPE\reference\UE_RenderPipeline\`
- wgpu 28 기준. Mesh shader: EXPERIMENTAL_MESH_SHADER
- DDGI chicken-and-egg: DDGI는 이전 프레임 material_eval output을 읽으므로 정상. Temporal lag 1 frame은 의도된 설계.
- 각 Task 완료 후 이 문서 Status 업데이트할 것.
- Sprint 순서는 의존성 기반. Dependency Graph 참조.
- `skope_resource` 크레이트는 렌더 인프라 (BufferPool, PipelineCache, StagingBelt). 직접적 TODO task 없지만 RDG migration (11.1) 시 활용됨.
