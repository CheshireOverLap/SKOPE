# SKOPE Engine Architecture

## Overview

SKOPE는 Rust로 작성된 3D 게임 엔진으로, wgpu 기반 V-Buffer 렌더링 파이프라인과 ECS(Entity Component System) 아키텍처를 사용합니다.

```
┌─────────────────────────────────────────────────────────────────┐
│                        SKOPE Engine                              │
├─────────────────────────────────────────────────────────────────┤
│  Application Layer                                               │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐                │
│  │   Editor    │ │    Game     │ │   Splash    │                │
│  └─────────────┘ └─────────────┘ └─────────────┘                │
├─────────────────────────────────────────────────────────────────┤
│  Core Systems                                                    │
│  ┌──────┐ ┌──────────┐ ┌─────────┐ ┌─────────┐ ┌──────────┐    │
│  │ ECS  │ │ Renderer │ │ Physics │ │  Audio  │ │ Scripting│    │
│  └──────┘ └──────────┘ └─────────┘ └─────────┘ └──────────┘    │
├─────────────────────────────────────────────────────────────────┤
│  Platform Layer (wgpu, winit, rapier3d, mlua)                   │
└─────────────────────────────────────────────────────────────────┘
```

---

## Target Platform

### Steam Machine (Primary Target)

| Component | Specification |
|-----------|---------------|
| **CPU** | Semi-custom AMD Zen 4 6코어/12스레드, 최대 4.8GHz |
| **GPU** | Semi-custom AMD RDNA3 28CU, 최대 2.45GHz |
| **RAM** | 16GB DDR5 + 8GB GDDR6 VRAM |
| **Storage** | 512GB / 2TB NVMe SSD |
| **Display** | 4K/240Hz (DP 1.4) / 4K/120Hz (HDMI 2.0) |
| **OS** | SteamOS 3 (Arch Linux) |

### Minimum Requirements

| Component | Requirement |
|-----------|-------------|
| GPU | AMD RDNA2+ / NVIDIA RTX 20+ |
| VRAM | 6GB+ |
| API | Vulkan 1.2+ |
| Features | Compute Shaders, texture_2d_array |

---

## Directory Structure

```
SKOPE/
├── src/                    # 메인 엔진 소스
│   ├── main.rs            # 엔트리 포인트
│   ├── app/               # 애플리케이션 레이어
│   ├── renderer/          # V-Buffer 렌더링 파이프라인
│   ├── editor/            # 에디터 UI (egui 기반)
│   ├── ecs_components/    # ECS 컴포넌트
│   ├── ecs_systems/       # ECS 시스템
│   ├── shaders/           # WGSL 셰이더
│   ├── scripting/         # Lua 스크립팅
│   ├── audio/             # 오디오 시스템
│   ├── physics.rs         # 물리 시스템 (Rapier3D)
│   ├── material/          # 머티리얼 시스템
│   └── game/              # 게임 로직
│
├── crates/                 # 독립 크레이트
│   ├── skope_render/      # 렌더링 유틸리티
│   ├── skope_gltf/        # glTF 로더
│   ├── skope_physics/     # 물리 래퍼
│   ├── skope_hair/        # 헤어 렌더링
│   ├── skope_effects/     # 파티클 시스템
│   ├── skope_post/        # 포스트 프로세싱
│   ├── skope_lighting/    # 클러스터드 라이팅
│   ├── skope_magic/       # 마법진 시스템
│   └── ...
│
├── engine/                 # 엔진 에셋 (아이콘, 폰트)
├── game/                   # 게임 에셋
│   ├── assets/            # 모델, 텍스처, 머티리얼
│   ├── scripts/           # Lua 스크립트
│   ├── prefabs/           # 프리팹 정의
│   └── levels/            # 레벨 데이터
└── launcher/              # 런처 애플리케이션
```

---

## Rendering Pipeline (V-Buffer)

SKOPE는 전통적인 Deferred Rendering 대신 **V-Buffer (Visibility Buffer)** 방식을 사용합니다.

### Pipeline Stages

```
┌─────────────────────────────────────────────────────────────────┐
│                    V-Buffer Pipeline                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Phase 1: Z-Prepass (Depth Only)                                │
│           └─> Depth Buffer                                      │
│                                                                  │
│  Phase 2: Visibility Pass (EQUAL depth test)                    │
│           └─> Triangle ID + Barycentric Coordinates             │
│                                                                  │
│  Phase 2.5: Cascaded Shadow Maps (CSM)                          │
│                                                                  │
│  Phase 3: Material Evaluation (Compute Shader)                  │
│           └─> HDR Color Output (Rgba16Float)                    │
│                                                                  │
│  Phase 4: Motion Vectors + HZB Generation                       │
│           └─> Velocity Buffer, Hierarchical Z-Buffer            │
│                                                                  │
│  Phase 5: Contact Shadows                                       │
│                                                                  │
│  Phase 6: GTAO (Ground Truth Ambient Occlusion)                 │
│                                                                  │
│  Phase 7: SSR (Screen-Space Reflections)                        │
│                                                                  │
│  Phase 8: DDGI (Dynamic Diffuse Global Illumination)            │
│                                                                  │
│  Phase 9: Volumetric Fog/Lighting                               │
│                                                                  │
│  Phase 9.5: Screen-Space Composite                              │
│             └─> GTAO + Contact Shadows + SSR 합성               │
│                                                                  │
│  Phase 10: Forward Pass (Hair, Eye, Particles)                  │
│            └─> Depth: Read-Only, Blend: Alpha                   │
│            └─> Stochastic Transparency (반투명 오브젝트)        │
│                                                                  │
│  Phase 11: TAA (Temporal Anti-Aliasing)                         │
│            └─> Motion Vector 기반 temporal reprojection         │
│            └─> Stochastic 노이즈 해소                           │
│                                                                  │
│  Phase 12: SSS (Subsurface Scattering)                          │
│                                                                  │
│  Phase 13: DoF (Depth of Field)                                 │
│                                                                  │
│  Phase 14: Post Processing (Bloom, Tonemapping)                 │
│                                                                  │
│  Phase 15: Blit to Screen (Gamma Correction)                    │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Key Renderer Components

| 파일 | 설명 |
|------|------|
| `renderer.rs` | VBufferRenderer 메인 구조체 |
| `vbuffer.rs` | V-Buffer 텍스처 (Triangle ID, Barycentric) |
| `material_eval.rs` | Material Evaluation Compute Pipeline |
| `zprepass.rs` | Z-Prepass Pipeline |
| `taa.rs` | Temporal Anti-Aliasing |
| `gtao.rs` | Ground Truth Ambient Occlusion |
| `ssr.rs` | Screen-Space Reflections |
| `contact_shadows.rs` | Contact Shadows |
| `volumetric.rs` | Volumetric Fog/Lighting |
| `sss.rs` | Subsurface Scattering |
| `dof.rs` | Depth of Field |
| `hzb.rs` | Hierarchical Z-Buffer |
| `motion_vectors.rs` | Motion Vector Generation |
| `skinned_mesh.rs` | Skeletal Animation Rendering |
| `ddgi/` | Dynamic Diffuse Global Illumination |
| `stochastic_transparency.rs` | Stochastic Transparency |
| `magic_circle.rs` | 마법진 렌더링 |
| `eye.rs` | 눈/홍채 렌더링 |
| `ss_composite.rs` | Screen-Space Effect Compositor |

### Bind Group Layout (4 Groups - wgpu limit)

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

Group 2: Materials + Lighting + Clustered + Shadows + DDGI
  - binding 0-5: materials, sampler, lighting, texture_2d_array (albedo, normal, metallic_roughness)
  - binding 6-9: clustered lighting
  - binding 10-12: shadow maps (CSM)
  - binding 13-15: DDGI (irradiance, visibility, probe_params)
  ※ 현재 texture_2d_array 사용, Bindless API 통합 예정

Group 3: Output
  - binding 0: HDR output (storage texture)
```

---

## ECS Architecture

**bevy_ecs 0.15** 기반 Entity Component System을 사용합니다.

### Components (`src/ecs_components/`)

| 컴포넌트 | 설명 |
|----------|------|
| `Transform` | 위치, 회전, 스케일 |
| `MeshInstance` | 메시 렌더링 데이터 |
| `Camera` | 카메라 설정 |
| `PointLight`, `SpotLight`, `DirectionalLight` | 조명 |
| `RigidBody`, `Collider` | 물리 컴포넌트 |
| `ScriptComponent` | Lua 스크립트 |
| `Animator` | 애니메이션 상태 |
| `Hierarchy`, `Parent`, `Children` | 씬 계층 구조 |
| `InventoryComponent` | 인벤토리 시스템 |
| `AIController` | AI 행동 |

### Systems (`src/ecs_systems/`)

```rust
// bevy_ecs Schedule 기반 시스템 실행
app.add_systems(Update, (
    input_system,           // 입력 처리
    script_system,          // Lua 스크립트
    ai_system,              // AI 업데이트
    physics_system,         // 물리 시뮬레이션
    animation_system,       // 애니메이션
    transform_system,       // 트랜스폼 계산
    camera_system,          // 카메라 업데이트
));
```

---

## Implemented Features

### Magic Circle System (마법진)

`skope_magic` crate + `editor/magic_system/`

- 절차적 마법진 생성
- 룬 문자 배치
- 회전/발광 애니메이션
- 에디터 통합 (Magic Circle Builder)

### Hair Rendering (헤어)

`skope_hair` crate

- Strand-based hair rendering
- Flyaway strand generation (Compute)
- Alpha blended forward pass
- Wind simulation

### Eye Rendering (눈)

`renderer/eye.rs`

- Parallax iris
- Cornea refraction
- Subsurface scattering for sclera

### Stochastic Transparency (반투명)

`renderer/stochastic_transparency.rs`

- TAA와 통합된 확률적 알파 블렌딩
- OIT 대비 메모리 효율적
- Hair, Particle 등 forward pass 오브젝트에 사용

---

## Editor (egui-based)

### Editor Panels

| 패널 | 설명 |
|------|------|
| `Scene View` | 3D 씬 뷰포트 |
| `Game View` | 게임 카메라 뷰 |
| `Hierarchy` | 엔티티 트리 뷰 |
| `Inspector` | 컴포넌트 프로퍼티 편집 |
| `Asset Browser` | 에셋 탐색기 |
| `Console` | 로그 출력 |
| `Animation Timeline` | 애니메이션 편집 |
| `AI Panel` | AI 디버그 |
| `Lua Inspector` | 스크립트 디버그 |
| `Magic Circle Builder` | 마법진 에디터 |

### Gizmo System

```
- Translation (W)
- Rotation (E)
- Scale (R)
- Local/World Space Toggle
- Snap to Grid
```

### Docking Layout

egui_dock 기반 자유로운 패널 배치를 지원합니다.

---

## Scripting (Lua)

mlua 기반 Lua 5.4 스크립팅 시스템입니다.

### API Modules

| 모듈 | 설명 |
|------|------|
| `math_api` | Vec3, Quat, Mat4 연산 |
| `entity_api` | 엔티티 생성/삭제/쿼리 |
| `core_api` | 시간, 입력, 로깅 |
| `world_api` | 월드 쿼리, 레이캐스트 |
| `animation_api` | 애니메이션 제어 |
| `audio_api` | 사운드 재생 |
| `gameplay_api` | 게임 로직 유틸리티 |

### Script Example

```lua
-- game/scripts/rotator.lua
function on_update(entity, dt)
    local transform = get_transform(entity)
    local rotation = transform.rotation
    rotation.y = rotation.y + dt * 45  -- 45도/초 회전
    set_rotation(entity, rotation)
end
```

---

## Physics (Rapier3D)

### Features

- Rigid Body Dynamics
- Collision Detection
- Ray Casting
- Character Controller
- Joints & Constraints

### Collider Types

```rust
ColliderShape::Box { half_extents }
ColliderShape::Sphere { radius }
ColliderShape::Capsule { half_height, radius }
ColliderShape::Cylinder { half_height, radius }
ColliderShape::ConvexHull { points }
ColliderShape::TriMesh { vertices, indices }
```

---

## Asset Pipeline

### Supported Formats

| 타입 | 포맷 |
|------|------|
| 3D Models | glTF 2.0 (.gltf, .glb) |
| Textures | PNG, JPEG, EXR, **KTX2** (BC/ASTC/ETC2) |
| Audio | OGG, WAV, MP3 |
| Scripts | Lua (.lua) |
| Materials | RON (.material.ron) |
| Prefabs | RON (.prefab.ron) |
| Effects | RON (.effect.ron) |

### Texture System

| 모듈 | 설명 | 상태 |
|------|------|------|
| `texture/ktx2_loader.rs` | KTX2 텍스처 로더 (BC1-7, ASTC, ETC2) | ✅ 사용 중 |
| `texture/bindless.rs` | Bindless Texture Heap API (4096 슬롯) | API만 구현 |
| `renderer/texture_array.rs` | 런타임 텍스처 배열 관리 | ✅ 사용 중 |

**현재 Material Eval 셰이더**:
```wgsl
// texture_2d_array 방식 (현재)
@group(2) @binding(3) var albedo_tex_array: texture_2d_array<f32>;
@group(2) @binding(4) var normal_tex_array: texture_2d_array<f32>;
@group(2) @binding(5) var metallic_roughness_tex_array: texture_2d_array<f32>;
```

**Bindless API** (통합 예정):
```rust
let handle = bindless_heap.register(texture_view);
// 셰이더에서: sample_bindless(handle, uv)
```

### Hot Reload

- Shaders: 자동 감지 및 재컴파일
- Materials: RON 파일 변경 감지
- Scripts: Lua 파일 변경 감지

---

## Shader System

### Preprocessor

`build.rs`에서 빌드 타임에 `#include` 지시문을 처리합니다.

```wgsl
// src/shaders/material_eval.wgsl
#include "common/constants.wgsl"
#include "common/shadow.wgsl"
```

### Common Includes

| 파일 | 내용 |
|------|------|
| `constants.wgsl` | PI, 상수 정의 |
| `shadow.wgsl` | 섀도우 샘플링 함수 |
| `pbr.wgsl` | PBR BRDF 함수 |

---

## Build & Run

```bash
# 개발 빌드
cargo build

# 릴리즈 빌드
cargo build --release

# 실행
cargo run

# 오디오 기능 포함
cargo run --features audio
```

---

## Dependencies

### Core

- `wgpu` - GPU 추상화 레이어
- `winit` - 윈도우 관리
- `egui` - 즉시 모드 GUI
- `bevy_ecs` - Entity Component System
- `rapier3d` - 물리 엔진
- `mlua` - Lua 바인딩
- `glam` - 수학 라이브러리

### Asset Loading

- `gltf` - glTF 파싱
- `image` - 이미지 로딩
- `rodio` - 오디오 재생

### Utilities

- `bytemuck` - 안전한 바이트 캐스팅
- `ron` - Rusty Object Notation
- `notify` - 파일 시스템 감시
- `env_logger` - 로깅

---

## Performance Considerations

### V-Buffer 장점

1. **낮은 대역폭**: Triangle ID만 저장 (G-Buffer 대비)
2. **머티리얼 복잡도 독립**: Compute에서 처리
3. **메모리 효율**: 고정 크기 V-Buffer

### Optimizations

- Z-Prepass로 오버드로우 제거
- Clustered Lighting으로 라이트 컬링
- HZB 기반 오클루전 컬링
- TAA로 temporal super sampling

---

## Roadmap

### Implemented

- [x] V-Buffer Rendering Pipeline
- [x] PBR Material System
- [x] Skeletal Animation
- [x] Lua Scripting
- [x] Physics (Rapier3D)
- [x] Magic Circle System
- [x] Hair Rendering
- [x] Eye Rendering
- [x] Particle Effects
- [x] TAA, GTAO, SSR, Contact Shadows
- [x] Volumetric Fog
- [x] SSS, DoF
- [x] Editor with Docking Layout
- [x] DDGI (Dynamic Diffuse Global Illumination)
- [x] KTX2 Texture Loader (BC/ASTC/ETC2)
- [x] Stochastic Transparency
- [x] Velocity Debug Visualization
- [x] Bindless Texture API (4096 slots)

### In Progress

- [ ] Bindless Texture → Material Eval 통합
- [ ] Shadow Atlas (CSM) 최적화
- [ ] Animation State Machine
- [ ] 텍스처 스트리밍

### Future Plans

- [ ] DDGI Scene Proxy (Light Leak 방지)
- [ ] Nanite-style Virtualized Geometry
- [ ] Neural Rendering Features
- [ ] VR/AR Support
