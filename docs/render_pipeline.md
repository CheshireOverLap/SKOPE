# SKOPE Engine — Render Pipeline Architecture

> wgpu 28.0 (Vulkan) | Deferred V-Buffer + Forward Hybrid | 606 Tests

---

## Pipeline Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        Frame Begin                              │
├─────────────────────────────────────────────────────────────────┤
│  Transform Propagation → Animation Update → Camera Setup        │
│  Frustum Culling (multi-threaded) → Light Manager Update        │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌───────────────────── Deferred Path ────────────────────────┐ │
│  │                                                            │ │
│  │  1. Visibility Pass (V-Buffer)                             │ │
│  │       ↓                                                    │ │
│  │  2. Cascaded Shadow Maps (CSM)                             │ │
│  │       ↓                                                    │ │
│  │  3. Material Evaluation (Compute)                          │ │
│  │       ↓                                                    │ │
│  │  4. Motion Vectors                                         │ │
│  │       ↓                                                    │ │
│  │  5. HZB Generation                                         │ │
│  │       ↓                                                    │ │
│  │  6. Screen-Space Effects                                   │ │
│  │     ├─ Contact Shadows                                     │ │
│  │     ├─ GTAO (Ambient Occlusion)                            │ │
│  │     ├─ SSR (Screen-Space Reflections)                      │ │
│  │     └─ Volumetric Fog                                      │ │
│  │       ↓                                                    │ │
│  │  7. SS Composite                                           │ │
│  │       ↓                                                    │ │
│  │  8. DDGI Update (Lumen GI)                                 │ │
│  │                                                            │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                 │
│  ┌───────────────────── Forward Path ─────────────────────────┐ │
│  │                                                            │ │
│  │  9.  Skinned Mesh Pass                                     │ │
│  │  10. Hair Rendering (Marschner BRDF)                       │ │
│  │  11. Particle / Effect Rendering                           │ │
│  │  12. Magic Circle SDF Rendering                            │ │
│  │  13. Transparency (OIT / Stochastic)                       │ │
│  │                                                            │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                 │
│  ┌───────────────────── Post-Processing ──────────────────────┐ │
│  │                                                            │ │
│  │  14. TAA Resolve                                           │ │
│  │  15. Subsurface Scattering (SSS)                           │ │
│  │  16. Depth of Field (DoF)                                  │ │
│  │  17. Bloom + Auto Exposure                                 │ │
│  │  18. Tonemapping + Color Grading                           │ │
│  │                                                            │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                 │
│  ┌───────────────────── Overlay ──────────────────────────────┐ │
│  │                                                            │ │
│  │  19. Debug Draw (bounds, lights, colliders)                │ │
│  │  20. Editor Grid + Gizmo                                   │ │
│  │  21. Game UI (Lua-integrated)                              │ │
│  │  22. Final Blit → Swapchain                                │ │
│  │                                                            │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│                        Frame End                                │
└─────────────────────────────────────────────────────────────────┘
```

---

## 1. Visibility Pass (V-Buffer)

> `src/renderer/vbuffer.rs` · `src/renderer.rs`

단일 패스로 모든 불투명 지오메트리의 가시성을 결정한다.
전통적인 G-Buffer 대신 V-Buffer를 사용하여 메모리 대역폭을 절감한다.

| 항목 | 값 |
|------|-----|
| Pipeline | Render (Vertex + Fragment) |
| Depth Compare | `Less` |
| Render Targets | 3 |

**출력 텍스처:**

| Attachment | Format | 내용 |
|------------|--------|------|
| Color 0 | `R32Uint` | Triangle ID (mesh_index:16 \| primitive_index:16) |
| Color 1 | `RG16Float` | Barycentric coordinates (u, v) |
| Depth | `Depth32Float` | Depth buffer |

**Bind Groups:**

| Group | Binding | 내용 |
|-------|---------|------|
| 0 | 0 | Camera Uniform (view_proj, position, screen_size) |
| 0 | 1 | Model Uniform (world_matrix) |
| 1 | 0 | Visibility Params (vertex/index offset, material index) |

**V-Buffer vs G-Buffer 비교:**

```
V-Buffer: 8 bytes/pixel  (Triangle ID 4B + Barycentric 4B)
G-Buffer: 24-32 bytes/pixel (Albedo 8B + Normal 8B + PBR 8B + Emissive 8B)
```

---

## 2. Cascaded Shadow Maps (CSM)

> `crates/skope_lighting/src/shadows.rs` · `src/renderer/shadow_atlas.rs`

디렉셔널 라이트를 위한 4-캐스케이드 그림자 맵.

| 항목 | 값 |
|------|-----|
| Pipeline | Render (Depth-Only) |
| Cascades | 4 |
| Resolution | 2048 × 2048 (per cascade) |
| Format | `Depth32Float` |
| Features | PCSS, depth bias, normal bias |

```
Cascade 0: Near (0-10m)    ← 높은 해상도
Cascade 1: Mid  (10-30m)
Cascade 2: Far  (30-100m)
Cascade 3: Very Far (100m+) ← 낮은 해상도
```

---

## 3. Material Evaluation (Compute)

> `src/renderer/material_eval.rs`

V-Buffer의 Triangle ID + Barycentric으로부터 PBR 머티리얼을 평가하는 컴퓨트 셰이더.
Bindless 텍스처 배열을 사용하여 모든 머티리얼을 단일 디스패치로 평가한다.

| 항목 | 값 |
|------|-----|
| Pipeline | Compute |
| Output | `RGBA16Float` (HDR) |
| Also outputs | `RGBA16Float` (Normal/Roughness G-Buffer for SSR) |

**Bind Groups (4 — wgpu 최대):**

| Group | 내용 |
|-------|------|
| G0 | V-Buffer: Triangle ID, Barycentric, Depth, Sampler |
| G1 | Geometry: Vertices, Indices, Mesh infos |
| G2 | Material + Lighting: Materials[], Bindless textures[], Clustered lighting, CSM, DDGI |
| G3 | Output: HDR storage texture |

**처리 흐름:**

```
1. Triangle ID → mesh index + primitive index
2. Barycentric + Geometry → world position, normal, UV
3. UV + Bindless textures → albedo, normal map, metallic/roughness
4. Clustered lighting → direct illumination
5. CSM → shadow factor
6. DDGI → indirect illumination (GI)
7. → HDR color output
```

---

## 4. Motion Vectors

> `src/renderer/motion_vectors.rs`

프레임 간 픽셀 이동 벡터를 계산한다. TAA와 모션 블러에 사용된다.

| 항목 | 값 |
|------|-----|
| Pipeline | Render |
| Input | Depth, current/previous VP matrices, camera jitter |
| Output | `RG16Float` (velocity UV) |

---

## 5. HZB (Hierarchical Z-Buffer)

> `src/renderer/hzb.rs`

깊이 버퍼의 밉 체인을 생성하여 빠른 레이 마칭에 사용한다.

| 항목 | 값 |
|------|-----|
| Pipeline | Compute (per mip level) |
| Mip Levels | Up to 12 (max 4096×4096) |
| Format | `R32Float` |
| Used by | SSR, DDGI, Contact Shadows, Nanite Occlusion Culling |

---

## 6. Screen-Space Effects

### 6-A. Contact Shadows

> `src/renderer/contact_shadows.rs`

| 항목 | 값 |
|------|-----|
| Pipeline | Compute |
| Input | HZB, sun direction, view-projection |
| Output | `RG16Float` (shadow) |
| Method | Hi-Z ray march + early termination |

### 6-B. GTAO (Ground Truth Ambient Occlusion)

> `src/renderer/gtao.rs`

| 항목 | 값 |
|------|-----|
| Pipeline | Compute (3-pass: AO → Spatial → Temporal) |
| Input | Depth, Normal/Roughness, previous AO |
| Output | `R16Float` (AO) |
| Method | Horizon-based sampling + temporal filtering |
| Parameters | radius, falloff, intensity, power, direction_count, step_count |

### 6-C. SSR (Screen-Space Reflections)

> `src/renderer/ssr.rs`

| 항목 | 값 |
|------|-----|
| Pipeline | Compute (2-pass: Trace → Resolve) |
| Input | HZB, Normal/Roughness, Depth, view-projection |
| Output | `RG16Float` (reflection + history) |
| Method | Hi-Z ray marching + temporal accumulation |
| Parameters | max_distance, thickness, max_steps, roughness_threshold |

### 6-D. Volumetric Fog

> `src/renderer/volumetric.rs`

| 항목 | 값 |
|------|-----|
| Pipeline | Compute (3-pass: Inject → Scatter → Apply) |
| Grid | Froxel 3D (width × height × depth) |
| Input | Depth, shadows, lights |
| Output | 3D volumetric texture |
| Method | Froxel + Henyey-Greenstein scattering |

---

## 7. SS Composite

> `src/renderer/ss_composite.rs`

모든 스크린-스페이스 이펙트를 HDR 버퍼에 합성한다.

| 항목 | 값 |
|------|-----|
| Pipeline | Compute |
| Input | HDR color, GTAO, Contact Shadows, SSR |
| Output | Composite HDR texture |

---

## 8. DDGI (Lumen GI)

> `crates/skope_lumen/src/` · `src/renderer/ddgi/`

UE5 Lumen에서 영감을 받은 동적 글로벌 일루미네이션 시스템.

| 항목 | 값 |
|------|-----|
| Pipeline | Compute |
| Cascades | 3 (2m, 8m, 32m spacing) |
| Probe Irradiance | 8×8 octahedral encoding |
| Probe Visibility | 16×16 octahedral encoding |
| Format | `RGBA16Float` (irradiance + visibility atlas) |
| Method | Screen-space + SDF hybrid ray tracing |
| Features | Ray reuse, hysteresis blending, frame-based updates |

```
Cascade 0 (2m):  실내/근접 GI
Cascade 1 (8m):  중거리 GI
Cascade 2 (32m): 원거리/실외 GI
```

---

## 9-13. Forward Passes

### 9. Skinned Mesh

> `src/renderer/skinned_mesh.rs`

본 매트릭스 기반 스키닝 메시의 포워드 렌더링.

### 10. Hair Rendering

> `crates/skope_hair/src/pipeline.rs`

| 항목 | 값 |
|------|-----|
| Pipeline | Render + Compute (flyaway generation) |
| Methods | Card renderer + Strand renderer |
| BRDF | Marschner (realistic hair scattering) |
| Blending | Alpha |

### 11. Particle / Effect Rendering

> `crates/skope_effects/src/`

| System | 설명 |
|--------|------|
| Flipbook | Sprite sheet animation (CPU-driven) |
| VAT | Vertex Animation Textures (Houdini/JangaFX) |
| GPU Particles | Compute shader simulation + rendering |
| EffectRenderer | Unified orchestrator for all three |

### 12. Magic Circle SDF

> `crates/skope_magic/src/pipeline/sdf_renderer.rs`

SDF 기반 프로시저럴 마법진 렌더링. 노드 그래프로 정의된 애니메이션 SDF.

### 13. Transparency

| System | Source | Method |
|--------|--------|--------|
| OIT | `src/renderer/oit.rs` | Order-Independent Transparency |
| Stochastic | `src/renderer/stochastic_transparency.rs` | Stochastic alpha |

---

## 14-18. Post-Processing

### 14. TAA (Temporal Anti-Aliasing)

> `src/renderer/taa.rs`

| 항목 | 값 |
|------|-----|
| Pipeline | Render (2-stage: Resolve → History Copy) |
| Jitter | Halton base-2,3 (16 samples/cycle) |
| History | Ping-pong `RGBA16Float` |
| Blend Factor | 0.9 |
| Variance Clip Gamma | 1.25 |
| Method | Variance-clipped temporal resolve |

### 15. Subsurface Scattering

> `src/renderer/sss.rs`

| 항목 | 값 |
|------|-----|
| Pipeline | Compute |
| Method | Separable blur, depth-aware |
| Input | HDR, depth, SSS mask |

### 16. Depth of Field

> `src/renderer/dof.rs`

| 항목 | 값 |
|------|-----|
| Pipeline | Compute (4-pass: CoC → Downsample → Blur → Composite) |
| Parameters | focus distance, focus range, aperture (f-stop), focal length |
| Method | Physically-based bokeh |

### 17. Bloom + Auto Exposure

> `crates/skope_post/src/bloom.rs` · `crates/skope_post/src/auto_exposure.rs`

```
Bloom:  Threshold → Multi-scale Blur → Composite
Auto Exposure: Luminance histogram → EV adaptation
```

### 18. Tonemapping

> `crates/skope_post/src/tonemapping.rs`

HDR → LDR 변환 + 컬러 그레이딩 + 필름 그레인 + 비네팅.

---

## Nanite Virtual Geometry Pipeline

> `crates/skope_nanite/`

GPU-Driven 클러스터 기반 메시 렌더링. Task + Mesh Shader 활용 (wgpu 28).

### Architecture

```
┌──────────────────────────────────────────────────────────┐
│                   CPU: Per Frame                          │
│  Upload instances, camera, update CullParams             │
│  Reset counters/indirect buffers                         │
└──────────────┬───────────────────────────────────────────┘
               ↓
┌──────────────────────────────────────────────────────────┐
│             GPU: Cull Compute Shader                      │
│  @compute @workgroup_size(64)                            │
│                                                          │
│  Per meshlet:                                            │
│    1. Transform bounding sphere → world                  │
│    2. Frustum test (6 planes)                            │
│    3. LOD selection (screen-space error)                  │
│    4. HZB occlusion test                                 │
│    5. Normal cone backface cull                          │
│    6. Classify: HW (>32px) vs SW (<32px)                 │
│    7. Append to visible_clusters[]                       │
│    8. Increment counters[1] (HW) or counters[2] (SW)    │
│                                                          │
│  Bind Groups:                                            │
│    G0: CullParams + Instances                            │
│    G1: Meshlets                                          │
│    G2: HZB texture + sampler                             │
│    G3: Outputs (visible_clusters, indirect, counters)    │
└──────────────┬───────────────────────────────────────────┘
               ↓
       ┌───────┴───────┐
       ↓               ↓
┌──────────────┐ ┌──────────────────────────────────────────┐
│  SW Raster   │ │  Mesh Shader Raster (HW Path)            │
│  (Compute)   │ │                                          │
│              │ │  Task Shader @workgroup_size(1):          │
│  Per cluster:│ │    Read counters[1] = HW cluster count   │
│  scan-convert│ │    Dispatch up to 32 mesh workgroups     │
│  triangles   │ │                                          │
│  atomicMin   │ │  Mesh Shader @workgroup_size(64):        │
│  vis buffer  │ │    1 workgroup = 1 meshlet               │
│              │ │    Emit ≤64 verts, ≤124 tris             │
│  Output:     │ │    World transform + clip projection     │
│  u32 storage │ │    V-Buffer encoding per primitive       │
│  (depth|     │ │                                          │
│   payload)   │ │  Fragment Shader:                        │
│              │ │    Output triangle_id (R32Uint)          │
│              │ │    Output barycentrics (RG16Float)       │
│              │ │                                          │
│              │ │  Bind Groups:                            │
│              │ │    G0: Camera uniform                    │
│              │ │    G1: Vertices, Triangles, Meshlets     │
│              │ │    G2: Instances, VisibleClusters,       │
│              │ │        Counters                          │
└──────┬───────┘ └──────────────────┬───────────────────────┘
       ↓                            ↓
┌──────────────────────────────────────────────────────────┐
│              Visibility Resolve                           │
│  Merge HW (render target) + SW (storage) → V-Buffer     │
└──────────────────────────────────────────────────────────┘
```

### Meshlet Specification

| 항목 | 값 |
|------|-----|
| Max Vertices/meshlet | 64 |
| Max Triangles/meshlet | 124 |
| Max LOD Levels | 25 |
| Max Visible Meshlets/frame | 1,000,000 |

### V-Buffer Encoding (per pixel)

```
payload (u32):
  ┌─────────────────┬──────────────┬──────────────┐
  │ cluster_id (20) │ tri_id (7)   │ mat_id (5)   │
  │ bits 31..12     │ bits 11..5   │ bits 4..0    │
  └─────────────────┴──────────────┴──────────────┘
```

### Key Types

```rust
// crates/skope_nanite/src/types.rs

struct Meshlet {
    vertex_offset: u32,      // Global vertex buffer offset
    vertex_count: u32,       // ≤ 64
    triangle_offset: u32,    // Meshlet triangle index offset
    triangle_count: u32,     // ≤ 124
    bounding_sphere: [f32;4],// xyz=center, w=radius
    normal_cone: [f32;4],    // xyz=axis, w=cos(half_angle)
    lod_error: f32,          // Screen-space error threshold
    parent_error: f32,       // Parent's error threshold
    group_id: u32,           // LOD group ID
    lod_level: u32,          // 0 = finest
}

struct NaniteInstance {
    world_matrix: [[f32;4];4],
    prev_world_matrix: [[f32;4];4],
    mesh_id: u32,
    material_id: u32,
    lod_bias: f32,
    flags: u32,              // VISIBLE(1) | SHADOW_CASTER(2) | MOVABLE(4)
}

struct MeshTaskIndirectArgs {
    group_count_x: u32,
    group_count_y: u32,
    group_count_z: u32,
    _pad: u32,
}
```

---

## Texture Format Summary

| Texture | Format | Size/pixel | Usage |
|---------|--------|-----------|-------|
| V-Buffer Triangle ID | `R32Uint` | 4B | Mesh + primitive index |
| V-Buffer Barycentric | `RG16Float` | 4B | UV interpolation |
| Depth | `Depth32Float` | 4B | Z-buffer |
| HDR Color | `RGBA16Float` | 8B | Main color buffer |
| Normal/Roughness | `RGBA16Float` | 8B | SSR input |
| Motion Vectors | `RG16Float` | 4B | TAA / motion blur |
| HZB | `R32Float` | 4B/mip | Occlusion queries |
| GTAO | `R16Float` | 2B | Ambient occlusion |
| SSR | `RG16Float` | 4B | Reflections |
| CSM | `Depth32Float` | 4B | Shadow maps (4 cascades) |
| DDGI Irradiance | `RGBA16Float` | 8B | GI probe data |
| DDGI Visibility | `RGBA16Float` | 8B | GI probe data |
| TAA History | `RGBA16Float` | 8B | Temporal buffer (×2) |

**Total per-pixel footprint** (at 1080p, all features on):

```
V-Buffer:      ~12 bytes
HDR + NR:      ~16 bytes
Screen-space:  ~14 bytes (motion, GTAO, SSR, contact shadow)
TAA history:   ~16 bytes (×2 ping-pong)
─────────────────────────
Total:         ~58 bytes/pixel ≈ 115 MB @ 1080p
```

---

## wgpu Features

| Feature | Status | Usage |
|---------|--------|-------|
| `TEXTURE_BINDING_ARRAY` | Enabled | Bindless texture arrays |
| `SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING` | Enabled | Dynamic texture indexing |
| `EXPERIMENTAL_MESH_SHADER` | Enabled | Nanite Task + Mesh shader |
| Storage Buffers | Used | Material eval, instance data |
| Storage Textures | Used | Compute shader output |
| Compute Pipelines | Used | 10+ passes |
| Multi-attachment Render Pass | Used | V-Buffer (2 color + depth) |

**Backend:** Vulkan only (wgpu 28, DX12 disabled due to windows crate conflict)

---

## Multi-Thread Architecture

> `src/renderer/thread.rs`

| Stage | Threading | Description |
|-------|-----------|-------------|
| Frustum Culling | Parallel | Per-chunk frustum tests |
| Material Sorting | Parallel | Sort by material / depth |
| Draw Call Prep | Parallel | Build render commands |
| Command Submission | Single | `queue.submit()` (wgpu requirement) |

---

## Rendering Settings

> `src/app/state.rs` → `RenderSettings`

| Setting | Default | Description |
|---------|---------|-------------|
| `enable_taa` | `true` | Temporal Anti-Aliasing |
| `enable_ddgi` | `true` | Dynamic Diffuse GI (Lumen) |
| `enable_shadows` | `true` | Cascaded Shadow Maps |
| `enable_ssr` | `true` | Screen-Space Reflections |
| `enable_contact_shadows` | `true` | Contact Shadows |
| `enable_gtao` | `true` | Ambient Occlusion |
| `enable_volumetric` | `false` | Volumetric Fog |
| `enable_sss` | `false` | Subsurface Scattering |
| `enable_dof` | `false` | Depth of Field |

---

## Debug Views

| Mode | Description |
|------|-------------|
| `None` | Normal rendering |
| `Depth` | Depth buffer visualization |
| `Normals` | World-space normals |
| `MotionVectors` | Per-pixel velocity |
| `MotionVectorsMagnitude` | Velocity magnitude heatmap |
| `DdgiProbes` | GI probe positions |
| `DdgiIrradiance` | GI irradiance map |

---

## Crate Structure

```
SKOPE/
├── src/
│   ├── app/
│   │   ├── state.rs              # Main application state
│   │   ├── state/render.rs       # Frame render orchestration
│   │   └── gpu_context.rs        # wgpu device creation
│   ├── renderer.rs               # Renderer (V-Buffer path entry)
│   └── renderer/
│       ├── vbuffer.rs            # V-Buffer pipeline
│       ├── material_eval.rs      # Compute material evaluation
│       ├── zprepass.rs           # Z-prepass (legacy, replaced by V-Buffer)
│       ├── shadow_atlas.rs       # Shadow atlas management
│       ├── motion_vectors.rs     # Motion vector generation
│       ├── hzb.rs                # Hierarchical Z-Buffer
│       ├── contact_shadows.rs    # Contact shadows
│       ├── gtao.rs               # Ground Truth AO
│       ├── ssr.rs                # Screen-Space Reflections
│       ├── volumetric.rs         # Volumetric fog
│       ├── ss_composite.rs       # Screen-space composite
│       ├── taa.rs                # Temporal AA
│       ├── sss.rs                # Subsurface scattering
│       ├── dof.rs                # Depth of Field
│       ├── skinned_mesh.rs       # Skinned mesh forward pass
│       ├── oit.rs                # Order-Independent Transparency
│       ├── stochastic_transparency.rs
│       ├── lod.rs                # LOD selection
│       ├── thread.rs             # Multi-threaded render helpers
│       ├── ddgi/                 # DDGI (Lumen GI) integration
│       └── eye.rs                # Eye rendering (iris/sclera)
├── crates/
│   ├── skope_nanite/             # Nanite Virtual Geometry
│   │   ├── src/
│   │   │   ├── meshlet.rs        # Meshlet builder
│   │   │   ├── cull.rs           # GPU culling pipeline
│   │   │   ├── rasterize.rs      # Mesh + SW raster pipelines
│   │   │   ├── visibility.rs     # V-Buffer resolve
│   │   │   └── types.rs          # GPU data types
│   │   └── shaders/
│   │       ├── nanite_cull.wgsl
│   │       ├── nanite_rasterize_mesh.wgsl  # Task + Mesh shader
│   │       └── nanite_rasterize_sw.wgsl
│   ├── skope_lumen/              # Lumen GI (DDGI)
│   │   └── src/screen_probe.rs
│   ├── skope_lighting/           # Shadows, IBL
│   │   └── src/shadows.rs
│   ├── skope_post/               # Post-processing
│   │   └── src/ (bloom, ssao, auto_exposure, tonemapping)
│   ├── skope_effects/            # Particles, Flipbook, VAT
│   ├── skope_hair/               # Hair rendering
│   ├── skope_magic/              # SDF magic circles
│   ├── skope_outline/            # Outline rendering
│   ├── skope_vt/                 # Virtual Textures
│   ├── skope_ui/                 # Slate UI system
│   └── skope_game_ui/            # Game UI (Lua)
└── shaders/                      # Main shader sources
```
