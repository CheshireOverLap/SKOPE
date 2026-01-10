# SKOPE Engine

Rust 기반 고성능 게임 엔진. 자체 에디터 포함.

## 빌드 및 실행

```bash
# 빌드
cargo build --release

# 실행
cargo run

# 오디오 활성화 (libasound2-dev 필요)
cargo run --features audio

# 로그 레벨 설정
RUST_LOG=debug cargo run      # 모든 로그
RUST_LOG=info cargo run       # 정보성 로그만
RUST_LOG=SKOPE=debug cargo run # SKOPE 모듈만 디버그
```

## 주요 기능

### 렌더링
- Deferred Rendering (G-Buffer)
- Cascaded Shadow Maps (4단계)
- PBR 라이팅 (Point, Spot, Sun)
- Clustered Forward+ Lighting
- 포스트 프로세싱 (Bloom, Tonemapping, SSAO, TAA, DOF, Motion Blur)
- 아웃라인 렌더링
- Hair 렌더링 (Flyaway + Silhouette)
- 파티클 시스템
- 셰이더 핫리로드 (Debug 빌드)

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

### 에셋
- glTF/GLB 로딩 (메시, 스켈레탈, 애니메이션)
- .skope 씬 포맷 (RON)
- Prefab 시스템
- UI 핫 리로드

## 프로젝트 구조

```
SKOPE/
├── src/                    # Rust 엔진 코드
│   ├── main.rs            # 엔트리 포인트
│   ├── ecs_components.rs  # ECS 컴포넌트
│   ├── scripting/         # Lua 스크립팅
│   ├── shaders/           # 셰이더 시스템
│   ├── editor/            # 에디터 UI
│   └── ...
├── engine/                 # 엔진 빌트인 리소스
│   ├── shaders/           # WGSL 셰이더
│   ├── fonts/             # 에디터 폰트
│   └── icons/             # SVG 아이콘
├── game/                   # 게임 프로젝트
│   ├── assets/            # 모델, 텍스처, 이펙트
│   ├── levels/            # .skope 씬 파일
│   ├── prefabs/           # 프리팹 (RON)
│   └── scripts/           # Lua 스크립트
├── crates/                 # 엔진 서브 크레이트
├── docs/                   # 문서
└── launcher/               # Tauri 런처
```

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

## 기술 스택

| 분야 | 라이브러리 |
|------|-----------|
| 그래픽 | wgpu 27.0 (Vulkan/DX12/Metal) |
| 윈도우 | winit 0.30 |
| ECS | bevy_ecs 0.15 |
| 물리 | rapier3d 0.22 |
| 스크립팅 | mlua 0.10 (Lua 5.4) |
| UI | egui 0.33, 커스텀 게임 UI |
| 오디오 | rodio 0.19 (optional) |

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

## 라이선스

MIT OR Apache-2.0

---

**SKOPE Games** - https://github.com/SKOPEGames/SKOPE
