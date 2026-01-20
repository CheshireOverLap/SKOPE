# SKOPE Engine - Rendering Technical Specification

## 개요

SKOPE 엔진은 **V-Buffer (Visibility Buffer)** 렌더링 파이프라인을 사용하며, wgpu 27.0 (Vulkan/DX12/Metal) 기반입니다.

V-Buffer 방식은 전통적인 Deferred Rendering의 G-Buffer 대신, Triangle ID와 Barycentric 좌표만 저장하여 대역폭을 절약하고, Material Evaluation을 Compute Shader에서 수행합니다.

### Target Platform

| Platform | GPU | Notes |
|----------|-----|-------|
| **Steam Machine** | AMD RDNA3 28CU | Primary target, 4K/60fps |
| Desktop | NVIDIA RTX 20+ / AMD RDNA2+ | Vulkan 1.2+ |
| Linux | Mesa/RADV | SteamOS 3 지원 |

---

## 0. View Frustum Culling

렌더링 전 CPU에서 수행하는 가시성 컬링입니다.

### 구현 위치
- `src/renderer/frustum.rs` - Frustum 구조체 및 교차 테스트
- `src/app/state/render.rs` - 메시 수집 시 culling 적용

### Frustum 구조
- 6개 평면 (Near, Far, Left, Right, Top, Bottom)
- View-Projection 행렬에서 Gribb/Hartmann 방식으로 추출

```rust
// Frustum 생성
let frustum = Frustum::from_view_proj(proj * view);

// 교차 테스트
frustum.test_sphere(center, radius)      // BoundingSphere
frustum.test_aabb(min, max)              // AABB
frustum.test_transformed_sphere(...)     // 월드 변환된 BoundingSphere
```

### MeshBounds 컴포넌트

```rust
#[derive(Component)]
pub struct MeshBounds {
    pub aabb_min: Vec3,        // 로컬 AABB 최소점
    pub aabb_max: Vec3,        // 로컬 AABB 최대점
    pub sphere_center: Vec3,   // 바운딩 스피어 중심
    pub sphere_radius: f32,    // 바운딩 스피어 반지름
}

// 정점에서 자동 계산
MeshBounds::from_vertices(&positions)
```

### 적용
메시 수집 시 `MeshBounds` 컴포넌트가 있는 엔티티에 대해 frustum 테스트를 수행합니다.
`MeshBounds`가 없으면 무조건 렌더링됩니다 (backward compatibility).

---

## 1. V-Buffer 렌더링 파이프라인

```
┌─────────────────────────────────────────────────────────────────┐
│                    V-Buffer Rendering Pipeline                   │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Phase 0: Frustum Culling (CPU)                                 │
│           └─> Visible mesh list                                 │
│                                                                  │
│  Phase 1: Z-Prepass (Depth Only)                                │
│           └─> Depth Buffer (Depth32Float)                       │
│                                                                  │
│  Phase 2: Visibility Pass (EQUAL depth test)                    │
│           └─> V-Buffer: Triangle ID (R32Uint)                   │
│           └─> V-Buffer: Barycentric (RG16Float)                 │
│                                                                  │
│  Phase 2.5: Cascaded Shadow Maps (CSM)                          │
│             └─> 4-level cascade                                 │
│                                                                  │
│  Phase 3: Material Evaluation (Compute Shader)                  │
│           └─> HDR Color Output (Rgba16Float)                    │
│           └─> texture_2d_array 기반 텍스처 샘플링               │
│                                                                  │
│  Phase 4: Motion Vectors + HZB Generation                       │
│           └─> Velocity Buffer (RG16Float)                       │
│           └─> Hierarchical Z-Buffer (mip chain)                 │
│                                                                  │
│  Phase 5: Contact Shadows (Screen-Space)                        │
│                                                                  │
│  Phase 6: GTAO (Ground Truth Ambient Occlusion)                 │
│                                                                  │
│  Phase 7: SSR (Screen-Space Reflections)                        │
│           └─> HZB 기반 ray marching                             │
│                                                                  │
│  Phase 8: DDGI (Dynamic Diffuse Global Illumination)            │
│           └─> 3-Level Cascade Probe System                      │
│           └─> Screen-Space Ray Tracing → Irradiance Atlas       │
│                                                                  │
│  Phase 9: Volumetric Fog/Lighting                               │
│                                                                  │
│  Phase 9.5: Screen-Space Composite                              │
│             └─> GTAO + Contact Shadows + SSR 합성               │
│                                                                  │
│  Phase 10: Forward Pass (Hair, Eye, Particles)                  │
│            └─> Depth: Read-Only, Blend: Alpha                   │
│            └─> Stochastic Transparency (TAA로 노이즈 해소)      │
│                                                                  │
│  Phase 11: TAA (Temporal Anti-Aliasing)                         │
│            └─> Motion Vector 기반 temporal reprojection         │
│            └─> Stochastic 노이즈 해소                           │
│                                                                  │
│  Phase 12: SSS (Subsurface Scattering)                          │
│            └─> Screen-space diffusion                           │
│                                                                  │
│  Phase 13: DoF (Depth of Field)                                 │
│            └─> Bokeh blur                                       │
│                                                                  │
│  Phase 14: Post Processing                                      │
│            └─> Bloom (threshold + blur + composite)             │
│            └─> ACES Tonemapping                                 │
│                                                                  │
│  Phase 15: Blit to Screen                                       │
│            └─> sRGB Gamma Correction                            │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. V-Buffer 구성

### 2.1 V-Buffer Textures

| Buffer | Format | 내용 |
|--------|--------|------|
| Triangle ID | R32Uint | 메시 인덱스 (16bit) + 삼각형 인덱스 (16bit) |
| Barycentric | RG16Float | Barycentric UV 좌표 |
| Depth | Depth32Float | Z-Prepass 깊이 |

### 2.2 V-Buffer 장점

1. **낮은 대역폭**: G-Buffer (64-128 bytes/pixel) 대비 8 bytes/pixel
2. **머티리얼 복잡도 독립**: Compute에서 온디맨드 평가
3. **메모리 효율**: 고정 크기, 머티리얼 수에 무관
4. **디커플링**: 가시성과 셰이딩 분리

---

## 3. Material Evaluation (Compute Shader)

### 3.1 파이프라인

```wgsl
// material_eval.wgsl (Compute Shader)
@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    // 1. V-Buffer에서 Triangle ID, Barycentric 읽기
    let triangle_id = textureLoad(v_triangle_id, id.xy, 0).r;
    let bary = textureLoad(v_barycentric, id.xy, 0).rg;

    // 2. 정점 데이터 fetch (vertices[], indices[], mesh_infos[])
    let v0, v1, v2 = fetch_triangle_vertices(triangle_id);

    // 3. Barycentric 보간으로 속성 계산
    let position = interpolate(v0.pos, v1.pos, v2.pos, bary);
    let normal = interpolate(v0.normal, v1.normal, v2.normal, bary);
    let uv = interpolate(v0.uv, v1.uv, v2.uv, bary);

    // 4. 머티리얼 샘플링
    let material = materials[mesh_info.material_index];
    let albedo = textureSample(albedo_array, sampler, uv, material.albedo_index);

    // 5. 라이팅 계산 (PBR)
    let color = calculate_pbr_lighting(position, normal, albedo, ...);

    // 6. HDR 출력
    textureStore(hdr_output, id.xy, color);
}
```

### 3.2 Bind Group Layout

```
Group 0: V-Buffer
  - binding 0: triangle_id (texture_2d<u32>)
  - binding 1: barycentric (texture_2d<f32>)
  - binding 2: depth (texture_depth_2d)
  - binding 3: sampler

Group 1: Geometry
  - binding 0: vertices (storage buffer)
  - binding 1: indices (storage buffer)
  - binding 2: mesh_infos (storage buffer)

Group 2: Materials + Lighting + Shadows + DDGI
  - binding 0: materials (storage buffer)
  - binding 1: material_sampler
  - binding 2: lighting (uniform buffer)
  - binding 3-5: texture_2d_array (albedo, normal, metallic_roughness)
  - binding 6-9: clustered lighting data
  - binding 10-12: shadow maps (CSM, Depth2DArray)
  - binding 13-15: DDGI (irradiance_atlas, visibility_atlas, probe_params)

Group 3: Output
  - binding 0: HDR output (storage texture, Rgba16Float)
```

---

## 4. DDGI (Dynamic Diffuse Global Illumination)

### 4.1 개요

DDGI는 실시간 글로벌 일루미네이션을 위한 프로브 기반 시스템입니다.

### 4.2 3-Level Cascade System

| Cascade | Grid Size | Spacing | Coverage |
|---------|-----------|---------|----------|
| Level 0 | 8×4×8 | 2.0m | 근거리 (16m) |
| Level 1 | 8×4×8 | 4.0m | 중거리 (32m) |
| Level 2 | 8×4×8 | 8.0m | 원거리 (64m) |

### 4.3 파이프라인

```
1. Ray Tracing (Compute)
   └─> 프로브당 128 rays (Spherical Fibonacci 분포)
   └─> Scene에서 radiance 샘플링
   └─> 출력: RayResult { radiance, distance, normal, hit }

2. Irradiance Update (Compute)
   └─> Ray 결과를 Octahedral 맵으로 누적
   └─> Hysteresis 블렌딩 (temporal stability)
   └─> 출력: Irradiance Atlas (8×8 per probe)

3. Visibility Update (Compute)
   └─> Chebyshev 거리 기반 가시성
   └─> 출력: Visibility Atlas (16×16 per probe)

4. Material Evaluation에서 샘플링
   └─> 8개 인접 프로브 trilinear 보간
   └─> Cascade 간 블렌딩
```

---

## 5. Screen-Space Effects

### 5.1 SSR (Screen-Space Reflections)

- HZB 기반 ray marching
- Roughness에 따른 cone tracing
- Fallback: DDGI 또는 환경맵

### 5.2 GTAO (Ground Truth Ambient Occlusion)

- Multi-bounce approximation
- Temporal accumulation
- Bent normal 출력

### 5.3 Contact Shadows

- Screen-space ray marching
- 태양광 방향 기준
- 근거리 디테일 강화

---

## 6. TAA (Temporal Anti-Aliasing)

### 6.1 구현

```wgsl
// taa.wgsl
fn main() {
    let velocity = textureLoad(motion_vectors, coord, 0).rg;
    let history_coord = coord - velocity;

    let current = textureLoad(current_frame, coord, 0);
    let history = textureSample(history_buffer, sampler, history_coord);

    // Neighborhood clamping (ghosting 방지)
    let clamped_history = clamp_to_neighborhood(history, current, 3x3);

    // Temporal blend
    let result = mix(current, clamped_history, 0.9);
}
```

### 6.2 Jitter Pattern

- Halton(2, 3) 시퀀스
- 8 샘플 사이클
- 서브픽셀 오프셋

### 6.3 Skinned Mesh Velocity

Skeletal animation이 있는 메시는 본 애니메이션으로 인한 픽셀 이동을 별도로 계산해야 TAA가 정확하게 작동합니다.

#### 구현
- `JointMatricesUniform.prev_matrices`: 이전 프레임 본 매트릭스 저장
- `forward_skinned.wgsl`: vertex shader에서 velocity 계산

```wgsl
// 현재/이전 프레임 스키닝
let skin_matrix = get_skin_matrix(joints, weights);
let prev_skin_matrix = get_prev_skin_matrix(joints, weights);

let skinned_pos = skin_matrix * vec4(position, 1.0);
let prev_skinned_pos = prev_skin_matrix * vec4(position, 1.0);

// 클립 공간 변환
let current_clip = model_view_proj * skinned_pos;
let prev_clip = model_view_proj * prev_skinned_pos;

// Velocity = (current_ndc - prev_ndc) * 0.5
let velocity = (current_clip.xy/current_clip.w - prev_clip.xy/prev_clip.w) * 0.5;
```

#### 데이터 흐름
1. Animation update: 현재 프레임 본 매트릭스 계산
2. GPU 업로드: `JointMatricesUniform` (current + prev 128개씩)
3. 다음 프레임: 현재 매트릭스 → prev로 복사

---

## 7. Post Processing

### 7.1 Bloom

```
1. Threshold: HDR에서 밝은 픽셀 추출 (threshold > 1.0)
2. Downsample: 6-level mip chain
3. Upsample: Tent filter로 블러
4. Composite: 원본 + bloom * intensity
```

### 7.2 Tonemapping (ACES)

```wgsl
fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return saturate((x * (a * x + b)) / (x * (c * x + d) + e));
}
```

---

## 8. 텍스처 시스템

### 8.1 KTX2 Loader

지원 포맷:
- BC1-BC7 (Desktop, Steam Machine)
- ASTC 4x4-12x12 (Mobile)
- ETC2 (Mobile fallback)

### 8.2 현재 텍스처 바인딩 (texture_2d_array)

```wgsl
// material_eval.wgsl (현재 구현)
@group(2) @binding(3) var albedo_tex_array: texture_2d_array<f32>;
@group(2) @binding(4) var normal_tex_array: texture_2d_array<f32>;
@group(2) @binding(5) var metallic_roughness_tex_array: texture_2d_array<f32>;

// 머티리얼별 레이어 인덱스로 접근
fn sample_albedo(uv: vec2<f32>, layer: u32) -> vec4<f32> {
    return textureSampleLevel(albedo_tex_array, material_sampler, uv, layer, 0.0);
}
```

### 8.3 Bindless Textures API (구현됨, 통합 예정)

```rust
// texture/bindless.rs
pub const MAX_BINDLESS_TEXTURES: u32 = 4096;

let handle = bindless_heap.register(texture_view);
// 셰이더에서: sample_bindless(handle, uv)
```

### 8.4 Stochastic Transparency

```
Forward Pass (Hair, Eye, Particles)에서 사용

1. 확률적 알파 테스트 (dither pattern)
2. 여러 프레임에 걸쳐 샘플 누적
3. TAA에서 노이즈 해소
4. OIT 대비 메모리 효율적 (per-pixel list 불필요)
```

---

## 9. 렌더러 파일 구조

```
src/renderer/
├── mod.rs              # VBufferRenderer 메인
├── types.rs            # 공통 타입 정의
├── vbuffer.rs          # V-Buffer 텍스처 관리
├── material_eval.rs    # Material Evaluation Compute
├── zprepass.rs         # Z-Prepass Pipeline
├── taa.rs              # Temporal Anti-Aliasing
├── gtao.rs             # Ground Truth AO
├── ssr.rs              # Screen-Space Reflections
├── contact_shadows.rs  # Contact Shadows
├── volumetric.rs       # Volumetric Fog
├── sss.rs              # Subsurface Scattering
├── dof.rs              # Depth of Field
├── hzb.rs              # Hierarchical Z-Buffer
├── motion_vectors.rs   # Motion Vector Generation
├── velocity_viz.rs     # Velocity Debug Visualization
├── frustum.rs          # View Frustum Culling
├── lod.rs              # LOD System
├── hlod.rs             # Hierarchical LOD
├── ddgi/               # DDGI 시스템
│   ├── mod.rs
│   └── pipeline.rs
├── shadow_atlas/       # Shadow Atlas (CSM)
├── texture_array.rs    # 텍스처 배열 관리
├── skinned_mesh.rs     # Skeletal Animation
├── stochastic_transparency.rs  # Stochastic Transparency
├── magic_circle.rs     # Magic Circle Rendering
├── eye.rs              # Eye/Iris Rendering
└── ss_composite.rs     # Screen-Space Compositor
```

---

## 10. 성능 최적화

### 10.1 Culling 최적화

- **Frustum Culling**: CPU에서 BoundingSphere/AABB 기반 view frustum 테스트
- **HZB Occlusion**: Hierarchical Z-Buffer로 가려진 오브젝트 제거
- **LOD System**: 거리 기반 메시 LOD 선택, 스크린 커버리지 기준

### 10.2 V-Buffer 최적화

- Z-Prepass로 오버드로우 제거
- EQUAL depth test로 visibility pass 최적화
- Compute shader로 머티리얼 평가 (wave occupancy 최대화)

### 10.3 Lighting 최적화

- Clustered Lighting으로 라이트 컬링
- HZB 기반 오클루전 컬링
- Shadow Atlas로 섀도우 맵 재사용

### 10.4 Temporal 최적화

- TAA로 temporal super sampling
- DDGI hysteresis로 temporal stability
- Motion vector 기반 reprojection

---

## 11. Forward Pass 상세

### 11.1 Hair Rendering (Hybrid System)

`skope_hair` crate + Forward Pass (Phase 18)

Hair 렌더링은 Card + Strand + Flyaway를 조합한 Hybrid 시스템입니다.

#### 파이프라인
1. **Flyaway Generation** (Compute): Scalp points에서 동적 strand 생성
2. **Strand Rendering** (Forward): Line strip → triangle strip 렌더링
3. **Card Rendering** (Forward): Alpha-tested card mesh, Marschner BRDF

#### LOD 시스템
| LOD | Distance | 구성 |
|-----|----------|------|
| Full | 0-3m | Card + Strand + Flyaway (100%) |
| Reduced | 3-8m | Card + 50% Flyaway |
| CardSilhouette | 8-15m | Card + 50% Silhouette strand |
| CardOnly | 15m+ | Card only |

#### 셰이딩
- Marschner BRDF (R, TT, TRT lobes)
- Anisotropic specular highlights
- Kajiya-Kay tangent-based lighting

### 11.2 Eye Rendering

`renderer/eye.rs`

```
1. Parallax mapping (iris depth illusion)
2. Cornea refraction (normal offset)
3. Subsurface scattering (sclera)
4. Specular highlights (wet surface)
```

### 11.3 Particle System

`skope_effects` crate

```
1. GPU particle simulation (Compute)
2. Billboard rendering (Forward Pass)
3. Soft particles (depth fade)
4. Stochastic transparency
```

---

## 12. 참고 자료

- [The Visibility Buffer: A Cache-Friendly Approach to Deferred Shading](http://jcgt.org/published/0002/02/04/)
- [Dynamic Diffuse Global Illumination (DDGI)](https://morgan3d.github.io/articles/2019-04-01-ddgi/)
- [Stochastic Transparency](https://research.nvidia.com/publication/stochastic-transparency)
- [LearnOpenGL - PBR Theory](https://learnopengl.com/PBR/Theory)
- [Filament Material Guide](https://google.github.io/filament/Materials.html)
