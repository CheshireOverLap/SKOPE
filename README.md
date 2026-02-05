# SKOPE Engine

Pure Rust 3D 게임 엔진. UE5에서 영감을 받은 V-Buffer 렌더링 파이프라인, Nanite 가상 지오메트리,
Lumen GI, 자체 에디터(Slate UI) 포함.

> wgpu 28 (Vulkan) | bevy_ecs 0.15 | 28 crates | 800+ tests

---

## 빌드 및 실행

```bash
# 빌드
cargo build --release

# 실행
cargo run

# 오디오 활성화
cargo run --features audio

# Live Link (Blender 연동)
cargo run --features live_link

# 로그 레벨
RUST_LOG=info cargo run
RUST_LOG=skope=debug cargo run
```

**요구사항:** Rust 1.82+, Vulkan SDK (RTX 2000+ 권장, Mesh Shader 지원)

---

## 렌더링 파이프라인

Deferred V-Buffer + Forward Hybrid 아키텍처.
상세 문서: [`docs/render_pipeline.md`](docs/render_pipeline.md)

```
Visibility (V-Buffer) → CSM Shadows → Material Eval (Compute)
    → Motion Vectors → HZB → Screen-Space Effects
    → Forward (Skinned, Hair, Particles, Transparency)
    → TAA → Post-Processing → Blit
```

### Deferred Path

| Pass | Type | Description |
|------|------|-------------|
| V-Buffer | Render | Triangle ID (R32Uint) + Barycentric (RG16Float) + Depth |
| CSM | Render | 4-cascade shadow maps (2048x2048, PCSS) |
| Material Eval | Compute | Bindless PBR + Clustered Lighting + CSM + DDGI |
| Motion Vectors | Render | Per-pixel velocity (RG16Float) |
| HZB | Compute | Hierarchical Z-Buffer (up to 12 mip levels) |
| Contact Shadows | Compute | Hi-Z ray march |
| GTAO | Compute | Ground Truth AO (3-pass: AO + Spatial + Temporal) |
| SSR | Compute | Screen-Space Reflections (Hi-Z trace + temporal) |
| Volumetric Fog | Compute | Froxel-based scattering |
| SS Composite | Compute | Merge all screen-space effects |

### Forward Path

| Pass | Description |
|------|-------------|
| Skinned Mesh | Bone matrix skinning |
| Hair | Card + Strand rendering (Marschner BRDF) |
| Particles | Flipbook + VAT + GPU Particles |
| Magic Circle | SDF-based procedural effects |
| Transparency | OIT / Stochastic |

### Post-Processing

| Pass | Description |
|------|-------------|
| TAA | Variance-clipped temporal resolve (Halton 16 samples) |
| SSS | Screen-space subsurface scattering |
| DoF | Physically-based bokeh (4-pass) |
| Bloom | Multi-scale threshold + blur + composite |
| Auto Exposure | Luminance histogram adaptation |
| Tonemapping | Tone curve + color grading + film grain |

---

## Nanite Virtual Geometry

> `crates/skope_nanite/`

GPU-Driven 클러스터 기반 메시 렌더링. **Task + Mesh Shader** (wgpu 28).

```
Cull (Compute) → visible_clusters
    ├─ Task Shader → Mesh Shader → V-Buffer  (>32px clusters)
    └─ SW Rasterizer (Compute, atomicMin)     (<32px clusters)
        → Visibility Resolve
```

| 항목 | 값 |
|------|-----|
| Max Vertices/meshlet | 64 |
| Max Triangles/meshlet | 124 |
| Max LOD Levels | 25 |
| Culling | Frustum + HZB Occlusion + Normal Cone + LOD |
| Classification | HW (Mesh Shader) vs SW (Compute) by screen size |

---

## Lumen GI (DDGI)

> `crates/skope_lumen/` + `src/renderer/ddgi/`

Dynamic Diffuse Global Illumination. 3-cascade probe grid.

| Cascade | Spacing | Coverage |
|---------|---------|----------|
| 0 | 2m | Indoor / near |
| 1 | 8m | Mid-range |
| 2 | 32m | Outdoor / far |

Probe encoding: 8x8 irradiance + 16x16 visibility (octahedral, RGBA16Float).

---

## 주요 기능

### 렌더링
- **V-Buffer Rendering** (Visibility Buffer — 8 bytes/pixel vs G-Buffer 24-32B)
- **Nanite** Virtual Geometry (Task + Mesh Shader, GPU culling, SW rasterizer)
- **Lumen GI** (DDGI, 3-cascade probe, screen-space + SDF hybrid)
- PBR Lighting (Clustered Forward+, Point/Spot/Directional)
- Cascaded Shadow Maps (4-cascade, PCSS)
- SSR (Hi-Z ray trace + temporal)
- GTAO (horizon-based + temporal)
- Contact Shadows, Volumetric Fog
- TAA (variance-clipped, Halton jitter)
- SSS, DoF, Bloom, Auto Exposure, Tonemapping
- Hair Rendering (Marschner BRDF, Card + Strand)
- Eye Rendering (Parallax Iris)
- Outline Rendering (SDF-based)
- Bindless Textures (4096 slots)
- Multi-threaded frustum culling + draw call prep

### 이펙트 시스템
- Flipbook 애니메이션 (Sprite sheet)
- VAT (Vertex Animation Textures, Houdini/JangaFX)
- GPU Particles (Compute simulation + rendering)
- Magic Circle SDF (Procedural, node graph)

### ECS 컴포넌트
- Transform, MeshInstance, Camera
- Player, Health, Team, Weapon
- EnemySpawner, Item, Trigger, Light
- Skeleton/Joint (스켈레탈 애니메이션)
- Physics (Rapier3D)

### 스크립팅
- Lua 5.4 (mlua)
- Vec3, Quat, Math, Time, Input API
- Entity, Transform, Audio, Collision, Debug API
- 문서: `docs/LUA_API.md`, `docs/LUA_QUICK_REF.md`

### 에디터
- **Slate UI** (UE5-style, `skope_ui` crate)
- Docking system (탭 드래그, 스플릿)
- Scene Viewer (Grid + Gizmo)
- Properties / Hierarchy / Content Browser
- Play mode (F5)
- Shader hot reload (Debug build)

### 에셋
- glTF/GLB (mesh, skeletal, animation)
- KTX2 (BC/ASTC/ETC2 GPU 압축)
- .skope 씬 포맷 (RON)
- Prefab 시스템
- UI 핫 리로드

---

## 프로젝트 구조

```
SKOPE/
├── src/                        # Main application
│   ├── main.rs                 # Entry point
│   ├── app/                    # State, GPU context, render loop
│   ├── renderer/               # Render passes (22+ passes)
│   │   ├── vbuffer.rs          #   V-Buffer pipeline
│   │   ├── material_eval.rs    #   Compute material evaluation
│   │   ├── ddgi/               #   DDGI (Lumen GI) integration
│   │   ├── ssr.rs              #   Screen-Space Reflections
│   │   ├── gtao.rs             #   Ambient Occlusion
│   │   ├── taa.rs              #   Temporal AA
│   │   ├── dof.rs              #   Depth of Field
│   │   └── ...                 #   (15+ more passes)
│   ├── editor/                 # Editor (gizmo, scene viewer)
│   ├── scripting/              # Lua scripting
│   └── shaders/                # Shader preprocessor
├── crates/                     # 28 engine sub-crates
│   ├── skope_nanite/           #   Nanite Virtual Geometry
│   │   ├── src/                #     Meshlet, Cull, Rasterize, Visibility
│   │   └── shaders/            #     WGSL (cull, mesh shader, SW raster)
│   ├── skope_lumen/            #   Lumen GI (DDGI)
│   ├── skope_vt/               #   Virtual Textures
│   ├── skope_ui/               #   Slate UI (UE5-style)
│   ├── skope_lighting/         #   Shadows, IBL
│   ├── skope_post/             #   Bloom, SSAO, Auto Exposure
│   ├── skope_effects/          #   Flipbook, VAT, GPU Particles
│   ├── skope_hair/             #   Hair rendering
│   ├── skope_magic/            #   SDF magic circles
│   ├── skope_outline/          #   Outline rendering
│   ├── skope_rdg/              #   Render Dependency Graph
│   ├── skope_resource/         #   GPU resource management
│   ├── skope_render/           #   Render primitives
│   ├── skope_shading/          #   Shading models
│   ├── skope_core/             #   Core types
│   ├── skope_physics/          #   Physics (Rapier3D)
│   ├── skope_scripting/        #   Lua integration
│   ├── skope_gltf/             #   glTF loader
│   ├── skope_audio/            #   Audio (rodio)
│   ├── skope_editor/           #   Editor logic
│   ├── skope_game_ui/          #   Game UI (Lua-driven)
│   ├── skope_debug_ui/         #   Debug overlays
│   ├── skope_mcp/              #   MCP integration
│   └── skope_app/              #   Application framework
├── engine/                     # Built-in resources
│   ├── shaders/                #   WGSL shaders
│   ├── fonts/                  #   Editor fonts
│   └── icons/                  #   SVG icons
├── game/                       # Game project
│   ├── assets/                 #   Models, textures, effects
│   ├── levels/                 #   .skope scene files
│   ├── prefabs/                #   Prefabs (RON)
│   └── scripts/                #   Lua scripts
├── docs/                       # Documentation
│   ├── render_pipeline.md      #   Render pipeline architecture
│   ├── LUA_API.md              #   Lua API reference
│   └── LUA_QUICK_REF.md        #   Lua quick reference
└── launcher/                   # Tauri launcher
```

---

## 기술 스택

| 분야 | 라이브러리 |
|------|-----------|
| 그래픽 | wgpu 28.0 (Vulkan, Mesh Shader) |
| 윈도우 | winit 0.30 |
| ECS | bevy_ecs 0.15 |
| 물리 | rapier3d 0.22 |
| 스크립팅 | mlua 0.10 (Lua 5.4) |
| UI | Slate UI (자체 구현, UE5-style) |
| 텍스처 | KTX2 (BC/ASTC/ETC2), Bindless (4096 slots) |
| 오디오 | rodio 0.19 (optional) |
| 병렬화 | rayon 1.10 |

### wgpu Features

| Feature | Usage |
|---------|-------|
| `TEXTURE_BINDING_ARRAY` | Bindless texture arrays |
| `NON_UNIFORM_INDEXING` | Dynamic texture indexing in compute |
| `EXPERIMENTAL_MESH_SHADER` | Nanite Task + Mesh shader |

---

## 씬 파일 형식 (.skope)

```ron
(
    entities: [
        (
            name: "Player",
            position: (x: 0.0, y: 1.0, z: 0.0),
            scale: (x: 1.0, y: 1.0, z: 1.0),
            component: PlayerSpawn,
        ),
        (
            name: "Light",
            position: (x: 5.0, y: 5.0, z: 5.0),
            component: Light(
                light_type: Point,
                light_energy: 100.0,
                light_color: (1.0, 0.9, 0.8),
            ),
        ),
    ],
)
```

---

## 에디터 단축키

| 키 | 기능 |
|----|------|
| `F5` | Play 모드 토글 |
| `Shift+F5` | 셰이더 핫리로드 |
| `~` | 콘솔 열기 |
| `G` | 이동 모드 |
| `R` | 회전 모드 |
| `S` | 스케일 모드 |

## 디버그 콘솔

실행 중 `~` 키로 콘솔 열기:

```
help          - 명령어 목록
spawn <name>  - 엔티티/프리팹 스폰
reload        - 씬 리로드
lua <code>    - Lua 코드 실행
```

---

## 라이선스

MIT OR Apache-2.0

---

**SKOPE Games** - https://github.com/SKOPEGames/SKOPE
