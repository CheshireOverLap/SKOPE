# SKOPE Engine TODO

> **최종 업데이트:** 2026-01-03
> **현재 상태:** Phase 24 (Hierarchy 드래그앤드롭) ✅ 완료

---

## 완료된 작업

- [x] V-Buffer 렌더링 파이프라인
- [x] Material Evaluation (PBR)
- [x] glTF 로딩 + 텍스처 배열
- [x] 스킨드 메시 + 애니메이션
- [x] Post Processing (Bloom, Tonemapping, Film Effects)
- [x] Bloom MIP 충돌 수정 (Ping-Pong 패턴)
- [x] ECS 시스템 (hecs)
- [x] 물리 엔진 (Rapier3D)
- [x] Lua 스크립팅
- [x] fyrox-ui 에디터 통합
- [x] 파티클 시스템 (기본)
- [x] 아웃라인 렌더링
- [x] **Critical: PBR 라이팅 안정화** (D_GGX 클램핑, NaN 방지, Barycentric 검증)
- [x] **Phase 13: Skin SSS + Eye 셰이더** (2026-01-01)
- [x] **Phase 14: Clustered Shading** (2026-01-01)
- [x] **Phase 15: 코드베이스 정리** (2026-01-02)
- [x] **Phase 16: 섀도우 맵 통합** (2026-01-02)
- [x] **Phase 17: 레벨 에디터 완성** (2026-01-02)
- [x] **Phase 18: 오디오 시스템** (2026-01-02)
- [x] **Phase 19: Live Link (Blender 실시간 동기화)** (2026-01-02)
- [x] **Phase 24: Hierarchy 드래그앤드롭** (2026-01-03)

---

## ~~Critical - 즉시 수정 필요~~ ✅ 완료

### PBR 라이팅 디버깅 (해결됨)
- [x] D_GGX 스펙큘러 폭발 문제 해결 → `d_ggx_max` 클램핑 적용
- [x] Deferred 렌더링 화이트 스크린 이슈 → `safe_normalize`, Barycentric 검증, NaN 체크
- [x] 스펙큘러 클램핑 값 조정 → UI 슬라이더 연동 완료

**수정된 파일:**
- `src/renderer/material_eval.rs` - MaterialEvalLighting 구조체 확장 (176바이트)
- `src/renderer/mod.rs` - update_lighting() 파라미터 전달
- `src/shaders/material_eval.wgsl` - safe_normalize, D_GGX 클램핑, Barycentric 검증

**기술 노트:**
```
MaterialEvalLighting (176 bytes)
- intensity_scale: 라이트 강도 배율 (기본 0.2)
- d_ggx_max: GGX 스펙큘러 최대값 클램핑 (기본 16.0)
- specular_max: 전체 스펙큘러 최대값 (기본 10.0)
- roughness_min: 최소 러프니스 값 (기본 0.1)

디버그 모드: 0-15, 99 (SpecularOnly=14, SpecularLog=15)
```

---

## High Priority

### ~~Phase 13: Skin SSS + Eye 셰이더~~ ✅ 완료
- [x] Multiple SSS LUT Array (Normal, Thin, Thick)
- [x] Screen-Space SSS Blur (Separable Gaussian, Bilateral)
- [x] Back-lighting SSS (Beer-Lambert 투과)
- [x] Dynamic Pupil Size (조명 적응)
- [x] Eye Caustics (FBM + Voronoi noise)
- [x] Advanced Eye Fresnel + Environment Reflection

**수정된 파일:**
- `src/shading/skin.rs` - SSS LUT 배열, `SssLutType` enum
- `src/shading/eye.rs` - Dynamic pupil, `PupilEmotion` enum
- `src/shaders/skin_gbuffer.wgsl` - Back-lighting SSS, curvature 계산
- `src/shaders/eye_gbuffer.wgsl` - Caustics, Fresnel, Environment reflection
- `src/shaders/noise.wgsl` - Perlin noise, FBM, Voronoi (새 파일)
- `src/post/sss_blur.rs` - Screen-space SSS blur pass (새 파일)
- `src/shaders/sss_blur.wgsl` - SSS blur shader (새 파일)

### ~~Phase 14: Clustered Shading~~ ✅ 완료
- [x] 클러스터 구조 구현 (16x16 tiles, 24 depth slices)
- [x] 다중 광원 지원 (CPU culling, GPU 루프)
- [x] 포인트/스팟 라이트 최적화

**수정된 파일:**
- `src/lighting/clustered.rs` - ClusterParams, CPU light culling
- `src/renderer/material_eval.rs` - Group 2에 클러스터 바인딩 통합 (6-9)
- `src/shaders/material_eval.wgsl` - 클러스터 인덱스 계산, 다중 광원 루프
- `src/renderer/mod.rs` - update_clustered_lighting() 통합
- `src/main.rs` - 렌더 루프에 클러스터드 라이팅 업데이트 추가

**기술 노트:**
```
ClusterConfig:
- tile_size: 16x16 pixels
- depth_slices: 24 (logarithmic)
- max_lights_per_cluster: 64

Bind Group 제한: wgpu max 4개
- Group 0: V-Buffer
- Group 1: Geometry Buffer
- Group 2: Material + Lighting + Textures + Clustered + Shadows (bindings 0-12)
- Group 3: Output

Light Types:
- Point Light: smooth attenuation, radius falloff
- Spot Light: cone attenuation + direction
```

### ~~Phase 16: 섀도우 맵 통합~~ ✅ 완료
- [x] Cascaded Shadow Map 인프라 분석 및 통합
- [x] Material Eval에 섀도우 바인딩 추가 (Group 2, bindings 10-12)
- [x] PCF 섀도우 샘플링 (Poisson disk 16-tap)
- [x] Compute shader 호환 textureLoad 사용

**수정된 파일:**
- `src/renderer/material_eval.rs` - 섀도우 바인딩 10-12 추가 (shadow_map, shadow_sampler, shadow_uniforms)
- `src/shaders/material_eval.wgsl` - 섀도우 샘플링 함수 및 태양광에 적용

**기술 노트:**
```
Shadow System:
- Cascaded Shadow Maps (4 cascades, 2048x2048)
- PCF with 16-tap Poisson disk sampling
- textureLoad for compute shader compatibility
- Normal offset bias for shadow acne prevention

Bind Group 2 확장:
- bindings 0-9: Material + Lighting + Clustered (기존)
- binding 10: shadow_map (texture_depth_2d_array)
- binding 11: shadow_sampler (NonFiltering)
- binding 12: shadow_uniforms (CascadeData[4] + params)
```

### ~~에디터 완성~~ ✅ 완료
- [x] Inspector 속성 편집 완성 (Transform: Position, Rotation, Scale)
- [x] Gizmo 회전/스케일 동작 연결 (W: Move, E: Rotate, R: Scale, Q: Select)
- [x] 씬 저장/로드 UI (SceneMenuPanel with New/Save/Load)
- [x] Undo/Redo 스택 연결 (Ctrl+Z: Undo, Ctrl+Shift+Z/Ctrl+Y: Redo)

**수정된 파일:**
- `src/editor/panels/scene_menu.rs` - 새 SceneMenuPanel 생성
- `src/editor/panels/mod.rs` - SceneMenuPanel 내보내기
- `src/app/state.rs` - SceneMenu 메시지 처리 추가
- `src/main.rs` - SceneMenuPanel 초기화 및 렌더 연결

**키보드 단축키:**
```
Edit 모드:
- W/E/R/Q: Gizmo 모드 (Move/Rotate/Scale/Select)
- Ctrl+Z: Undo
- Ctrl+Shift+Z / Ctrl+Y: Redo
- Ctrl+S: 씬 저장
- Ctrl+O: 씬 열기 다이얼로그
- Ctrl+D: 복제
- Ctrl+C/V/X: 복사/붙여넣기/잘라내기
- Delete: 삭제
- Shift+A: 생성 메뉴
- F: 선택된 오브젝트에 포커스
- Numpad 0/1/3/7: 카메라 뷰 (Perspective/Front/Right/Top)
```

### ~~오디오 시스템~~ ✅ 완료
- [x] 3D 공간 오디오 구현 (SpatialSink 기반, 리스너/소스 위치 추적)
- [x] 오디오 풀링 (AudioPool with LRU 캐시)
- [x] 컴파일: `cargo run --features audio`

**수정된 파일:**
- `src/audio/mod.rs` - 3D 공간 오디오, SpatialSettings, AudioPool 추가

**기술 노트:**
```
3D Spatial Audio:
- SpatialSink: rodio의 공간 오디오 지원
- 리스너 위치/방향: AudioListener 컴포넌트에서 업데이트
- 거리 감쇠: inverse distance + smooth falloff
- 귀 위치 계산: forward × up 벡터로 left/right ear 오프셋

AudioPool:
- CachedSound: 미리 디코딩된 오디오 샘플 저장
- LRU Eviction: 메모리 한도 초과 시 오래된 항목 제거
- 기본 캐시 크기: 64 MB

SpatialSettings:
- position: [f32; 3] - 월드 좌표
- max_distance: 최대 청취 거리 (기본 50.0)
- rolloff_factor: 감쇠 비율 (1.0 = 현실적)
```

---

## Phase 20: Effect System (JangaFX 통합)

### 구현 완료
- [x] **Phase E1: 기반 구조** (2026-01-02)
  - `src/effects/mod.rs` - 모듈 진입점
  - `src/effects/data.rs` - FlipbookMeta, VatMeta, GPU Instance 구조체
  - `src/effects/loader.rs` - RON/PNG/EXR 로더
  - `src/effects/components.rs` - FlipbookEffect, VatEffect ECS 컴포넌트
  - `src/effects/spawner.rs` - EffectSpawner, EffectHandle
  - `exr` 크레이트 의존성 추가

- [x] **Phase E2: Flipbook 렌더러** (2026-01-02)
  - `src/effects/flipbook.rs` - GPU 인스턴스드 빌보드 렌더러
  - `src/shaders/flipbook.wgsl` - UV 애니메이션, 프레임 블렌딩, 소프트 파티클

- [x] **Phase E3: VAT 렌더러** (2026-01-02)
  - `src/effects/vat.rs` - VAT 렌더러 (Camera/Model/Light uniforms)
  - `src/shaders/vat.wgsl` - Soft/Rigid/Fluid 모드

- [x] **Phase E4: Effect Spawner + Lua API** (2026-01-02)
  - `src/effects/spawner.rs` - EffectSpawner, EffectHandle
  - `src/scripting/api.rs` - SKOPE.Effect API 추가
  - 지원 함수: spawn, stop, on_complete, set_speed, pause, resume, attach, detach, is_playing

- [x] **Phase E5: 렌더러 통합** (2026-01-02)
  - `src/app/state.rs` - flipbook_renderer, vat_renderer 필드 추가
  - State::new()에서 렌더러 초기화

- [x] **Phase E6: 에디터 Effect Panel** (2026-01-02)
  - `src/editor/panels/effect_panel.rs` - EffectPanel 구현
  - EffectAction enum (Spawn, Stop, StopAll, SetSpeed)
  - UI: 이펙트 이름, 위치(X/Y/Z), Speed, Scale 입력
  - Spawn/StopAll 버튼, 활성 이펙트 수 표시

**파일 구조:**
```
src/effects/
├── mod.rs        - 모듈 진입점
├── data.rs       - 메타데이터 + GPU 구조체
├── loader.rs     - RON/PNG/EXR 로더
├── components.rs - ECS 컴포넌트
├── flipbook.rs   - Flipbook 렌더러
├── vat.rs        - VAT 렌더러
└── spawner.rs    - Effect 생성/관리

src/shaders/
├── flipbook.wgsl - Flipbook 셰이더
└── vat.wgsl      - VAT 셰이더

src/editor/panels/
└── effect_panel.rs - Effect 에디터 패널
```

---

## Medium Priority

### Post Processing 추가 기능

| 기능 | 상태 | 파일 | 비고 |
|------|------|------|------|
| DOF | 구현됨, 비활성화 | `post/dof.rs` | R32Float 사용 (R16Float 미지원) |
| Motion Blur | 구현됨, 비활성화 | `post/motion_blur.rs` | |
| SSAO | 구현됨, 비활성화 | `post/ssao.rs` | R32Float 사용 |
| TAA | 구현됨, 활성화 | `post/taa.rs` | |
| Color Grading | 구현됨, 활성화 | `post/color_grading.rs` | |

### ~~셰이더 수정~~ ✅ 완료
- [x] `hair_card.wgsl:69`: MVP 변환 추가 (현재 identity) → Camera/Transform uniform 바인딩 추가
- [x] `hair_card.wgsl:114-117`: 라이트 방향/색상 하드코딩 제거 → Light uniform 바인딩 추가

**수정된 파일:**
- `src/shaders/hair_card.wgsl` - CameraUniforms, ModelTransform, LightParams 구조체 및 바인딩 3-5 추가
- `src/hair/data.rs` - HairCameraUniform, HairModelTransform, HairLightParams 구조체 추가
- `src/hair/pipeline.rs` - 바인딩 레이아웃 확장, 버퍼 생성, update 메서드 추가
- `src/app/state.rs` - 렌더 루프에서 카메라/트랜스폼/라이트 uniform 업데이트 추가

### ~~스크립팅 강화~~ ✅ 완료 (Phase 21)
- [x] Lua 샌드박싱 통합 (`scripting/sandbox.rs`)
- [x] AI 코드 검증기 통합 (`scripting/validator.rs`)
- [x] 에러 리포터 연결 (`scripting/error.rs`)
- [x] Console 패널 (`editor/panels/console_panel.rs`)

**수정된 파일:**
- `src/scripting/mod.rs` - ScriptEngine에 TrustLevel, AiCodeValidator, ErrorReporter 통합
- `src/editor/panels/console_panel.rs` - Console 패널 (로그 표시, Lua 실행, 필터링)

**기술 노트:**
```
TrustLevel:
- AiGenerated: AI 생성 코드 (가장 엄격한 샌드박스)
- UserScript: 사용자 스크립트 (중간 제한)
- GameScript: 게임 번들 스크립트 (느슨한 제한)
- Engine: 엔진 내부 (제한 없음)

ScriptEngine API:
- new_sandboxed(): AI 코드용 샌드박싱된 엔진
- with_trust_level(level): 특정 신뢰 레벨 지정
- error_reporter(): 에러 리포터 접근
- take_errors_for_broadcast(): Live Link 전송용 에러 추출
```

### ~~파티클 시스템~~ ✅ 완료 (Phase 22)
- [x] Turbulence 효과 (노이즈 기반 난류)
- [x] Vortex 효과 (축 기반 회전력)
- [x] Attractor/Repulsor, Wind, Drag 추가
- [ ] GPU 시뮬레이션 (선택, 미구현)

**수정된 파일:**
- `src/particles/force_fields.rs` - ForceField, ForceFieldSystem 모듈 (새 파일)
- `src/particles/mod.rs` - EmitterConfig에 force_fields 통합, 새 프리셋 추가

**Force Field 타입:**
```
Turbulence: curl noise 기반 난류
Vortex: 축 중심 회전 (tornado 포함)
Attractor: 점 끌어당김/밀어냄
Wind: 일정 방향 바람 + 흔들림
Drag: 속도 감쇠
```

**새 프리셋:**
```rust
EmitterConfig::tornado()      // Vortex + Turbulence
EmitterConfig::magic()        // Turbulence + Attractor
EmitterConfig::flame_vortex() // Vortex + Turbulence (불꽃)
```

---

## Low Priority

### ~~Phase 15: 코드베이스 정리~~ ✅ 완료
- [x] main.rs 모듈화 (4260줄 → 1399줄, 67% 감소)
- [x] dead_code 경고 정리 (22개 → 0개)
- [x] 미사용 import 제거

**수정된 파일:**
- `src/app/mod.rs` - 새 모듈 생성
- `src/app/state.rs` - State 구조체 및 구현 (2900줄)
- `src/main.rs` - App 구조체와 ApplicationHandler만 유지
- `src/editor/mod.rs` - `#![allow(dead_code)]` 추가 (개발 중)
- `src/texture_array.rs` - `#[allow(dead_code)]` 추가

### 선택적 기능
- [x] Live Link (Blender 실시간 동기화) ✅ 완료
  - `cargo run --features live_link`
  - WebSocket 서버 (port 9999)
  - Blender 애드온 연동
  - EntityUpdate, Play/Stop, SceneSync 지원
- [ ] 동적 콜라이더 지원 (`skope_data.rs:391` 예약됨)

### 에디터 패널
- [ ] Hierarchy 패널 드래그앤드롭
- [ ] Asset Browser 프리뷰
- [x] Console 패널 ✅ (Phase 21에서 완료)

---

## 현재 설정값

```rust
// src/post/pipeline.rs - PostProcessConfig
bloom_enabled: true,         // 활성화
tonemapping_enabled: true,   // 활성화
color_grading_enabled: true, // 활성화
taa_enabled: true,           // 활성화
dof_enabled: false,          // 비활성화
motion_blur_enabled: false,  // 비활성화
ssao_enabled: false,         // 비활성화
film_effects_enabled: true,  // 활성화
```

---

## 파일별 TODO 위치

| 파일 | 라인 | 내용 |
|------|------|------|
| `shaders/hair_card.wgsl` | - | ✅ MVP/Light uniform 완료 |
| `post/dof.rs` | 214 | R16Float → R32Float (스토리지 제한) |
| `renderer/mod.rs` | 89 | 섀도우 바인드 그룹 미사용 |
| `audio/mod.rs` | - | ✅ 공간 오디오 완료 |
| `scripting/mod.rs` | 32-38 | sandbox, validator, error 미사용 |

---

## 빌드 옵션

```bash
# 기본 빌드
cargo build --release

# 오디오 기능 포함
cargo build --release --features audio

# Live Link 포함
cargo build --release --features live_link

# 전체 기능
cargo build --release --features "audio live_link"
```

---

## 통계

| 카테고리 | 개수 |
|----------|------|
| Critical | ~~3~~ 0 ✅ |
| High Priority | ~~10~~ 0 ✅ |
| Medium Priority | ~~11~~ 9 |
| Low Priority | 4 |
| **총계** | **13** |

---

## 향후 Phase 로드맵

| Phase | 목표 | 상태 |
|-------|------|------|
| 12.5 | Bloom MIP 충돌 수정 | ✅ 완료 |
| 12.6 | Critical PBR 이슈 해결 | ✅ 완료 |
| 13 | Skin SSS + Eye 셰이더 | ✅ 완료 |
| 14 | Clustered Shading | ✅ 완료 |
| 15 | 코드베이스 정리 | ✅ 완료 |
| 16 | 섀도우 맵 통합 | ✅ 완료 |
| 20 | Effect System (JangaFX 통합) | ✅ 완료 |
| 21 | 스크립팅 강화 (샌드박스/검증/Console) | ✅ 완료 |
| 22 | 파티클 시스템 강화 (Turbulence/Vortex) | ✅ 완료 |
| 17 | 레벨 에디터 완성 | ✅ 완료 |
| 18 | 오디오 시스템 | ✅ 완료 |
| 19 | Live Link (Blender 실시간 동기화) | ✅ 완료 |
| 23 | 멀티 크레이트 마이그레이션 | ✅ 완료 |
| 24 | Hierarchy 드래그앤드롭 | ✅ 완료 |

---

## Phase 24: Hierarchy 드래그앤드롭

### 목표
에디터 Hierarchy 패널에서 드래그앤드롭으로 부모-자식 관계 변경

### 구현 완료
- [x] DragDropState 상태 관리
- [x] HierarchyAction enum (None, SelectionChanged, Reparented, CyclicError)
- [x] Tree 노드 드래그/드롭 활성화 (with_allow_drag/drop)
- [x] 순환 참조 검사 (is_ancestor_of)
- [x] ReparentCommand 통합 (Undo/Redo 지원)
- [x] 메인 루프 통합 (state.rs)

### 수정된 파일
- `src/editor/panels/hierarchy.rs` - 드래그앤드롭 로직
- `src/editor/panels/mod.rs` - HierarchyAction export
- `src/app/state.rs` - HierarchyAction 처리

---

## Phase 23: 멀티 크레이트 마이그레이션

### 목표
120개 파일의 단일 크레이트 구조를 Cargo Workspace 기반 멀티 크레이트로 분리
- 컴파일 시간 단축 (병렬 빌드)
- 모듈 간 의존성 명확화
- 코드 재사용성 향상

### 진행 상태 - ✅ 완료

| 크레이트 | 상태 | 파일 수 | 비고 |
|----------|------|---------|------|
| skope_core | ✅ 완료 | 3 | ECS 컴포넌트, 리소스 |
| skope_shading | ✅ 완료 | 7 | Skin SSS, Eye, Face 데이터 |
| skope_render | ✅ 완료 | 5 | Uniforms, Lights, Vertex, Config |
| skope_post | ✅ 완료 | 1 | Post processing 설정 |
| skope_physics | ✅ 완료 | 3 | Rapier3D 통합 |
| skope_scripting | ✅ 완료 | 5 | Lua VM, 샌드박스, 검증 |
| skope_effects | ✅ 완료 | 3 | 파티클, Flipbook, VAT 데이터 |
| skope_audio | ✅ 완료 | 1 | 공간 오디오 설정 |
| skope_editor | ✅ 완료 | 1 | 에디터 구성/유틸 타입 |
| skope_app | ✅ 완료 | 1 | AppConfig, Input, Time |

### 완료된 크레이트

#### 워크스페이스 구조
```
SKOPE/crates/
├── skope_core/       # ECS 컴포넌트, 리소스
├── skope_shading/    # Skin SSS, Eye, Face 데이터
├── skope_render/     # Uniforms, Lights, Vertex 타입
├── skope_post/       # Post processing 설정
├── skope_physics/    # Rapier3D 물리 타입
├── skope_scripting/  # Lua 샌드박스, 검증, 에러 처리
├── skope_effects/    # 파티클, Flipbook, VAT 데이터
├── skope_audio/      # 공간 오디오 설정
├── skope_editor/     # 에디터 구성, Ray, AABB 유틸
└── skope_app/        # AppConfig, Input, Time, GraphicsSettings
```

#### skope_core
- `components.rs`: Transform, Camera, Physics, Gameplay 컴포넌트
- `resources.rs`: GPU Context, Assets, Input, Time 리소스

#### skope_shading
- `model_id.rs`: ShadingModelId enum
- `face.rs`, `skin.rs`, `eye.rs`: 셰이딩 파라미터
- `shadow.rs`: HairShadowProxyParams
- `color_manipulation.rs`: RGB/HSV 변환
- `gbuffer.rs`: GBuffer 레이아웃

#### skope_render
- `uniforms.rs`: CameraUniform, LightingUniform
- `lights.rs`: GpuLight, DirectionalLight, PointLight
- `vertex.rs`: Vertex, SkinnedVertex 타입
- `config.rs`: RenderConfig

#### skope_scripting
- `sandbox.rs`: ResourceLimits, TrustLevel, 샌드박스 Lua
- `validator.rs`: AiCodeValidator, 코드 검증
- `error.rs`: ErrorReporter, LuaErrorInfo

#### skope_effects
- `data.rs`: FlipbookMeta, VatMeta
- `particle.rs`: EmitterConfig, ParticleInstance, 프리셋

#### skope_app
- `lib.rs`: AppConfig, GraphicsSettings, InputState, Time, WindowState

### 발견된 미구현 기능

| 항목 | 상태 | 비고 |
|------|------|------|
| GPU 파티클 시뮬레이션 | 미구현 | Phase 22에서 CPU만 구현 |
| Hierarchy 드래그앤드롭 | ✅ 완료 | Phase 24에서 구현 |
| Asset Browser 프리뷰 | 미구현 | 에디터 UX |
| 동적 콜라이더 | 예약됨 | skope_data.rs:391 |
