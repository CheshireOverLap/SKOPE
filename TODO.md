# SKOPE Engine TODO

> **최종 업데이트:** 2026-01-01
> **현재 상태:** Phase 12.5 완료 (Post Processing + Bloom)

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

---

## Critical - 즉시 수정 필요

### PBR 라이팅 디버깅
- [ ] D_GGX 스펙큘러 폭발 문제 해결
- [ ] Deferred 렌더링 화이트 스크린 이슈
- [ ] 스펙큘러 클램핑 값 조정 (`d_ggx_max` 적정값 찾기)

**기술 노트:**
```
LightingUniform (192 bytes)
- intensity_scale: 라이트 강도 배율
- d_ggx_max: GGX 스펙큘러 최대값 클램핑 (기본 16.0)
- specular_max: 전체 스펙큘러 최대값
- roughness_min: 최소 러프니스 값

디버그 모드: 0-15, 99 (SpecularOnly=14, SpecularLog=15)
```

---

## High Priority

### Phase 13: Skin SSS + Eye 셰이더
- [ ] Subsurface Scattering (피부 렌더링)
- [ ] 눈 셰이더 (굴절, 코스틱)
- [ ] Pre-integrated Skin LUT

### Phase 14: Clustered Shading
- [ ] 클러스터 구조 구현
- [ ] 다중 광원 지원 (100+ lights)
- [ ] 포인트/스팟 라이트 최적화

### 에디터 완성
- [ ] Inspector 속성 편집 완성
- [ ] Gizmo 회전/스케일 동작 연결
- [ ] 씬 저장/로드 UI
- [ ] Undo/Redo 스택 연결

### 오디오 시스템
- [ ] 3D 공간 오디오 구현 (`audio/mod.rs:367` 스텁)
- [ ] 오디오 풀링
- [ ] 컴파일: `cargo run --features audio`

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

### 셰이더 수정
- [ ] `hair_card.wgsl:69`: MVP 변환 추가 (현재 identity)
- [ ] `hair_card.wgsl:114-117`: 라이트 방향/색상 하드코딩 제거

### 스크립팅 강화
- [ ] Lua 샌드박싱 통합 (`scripting/sandbox.rs` 존재)
- [ ] AI 코드 검증기 통합 (`scripting/validator.rs` 존재)
- [ ] 에러 리포터 연결 (`scripting/error.rs` 존재)

### 파티클 시스템
- [ ] Turbulence 효과
- [ ] Vortex 효과
- [ ] GPU 시뮬레이션 (선택)

---

## Low Priority

### Phase 15: 코드베이스 정리
- [ ] main.rs 모듈화 (2000줄+)
- [ ] dead_code 경고 정리 (14개 파일)
- [ ] 미사용 import 제거

### 선택적 기능
- [ ] Live Link (Blender 실시간 동기화)
  - `cargo run --features live_link`
- [ ] 동적 콜라이더 지원 (`skope_data.rs:391` 예약됨)

### 에디터 패널
- [ ] Hierarchy 패널 드래그앤드롭
- [ ] Asset Browser 프리뷰
- [ ] Console 패널

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
| `shaders/hair_card.wgsl` | 69 | MVP 변환 추가 필요 |
| `post/dof.rs` | 214 | R16Float → R32Float (스토리지 제한) |
| `renderer/mod.rs` | 89 | 섀도우 바인드 그룹 미사용 |
| `audio/mod.rs` | 367 | 공간 오디오 스텁 |
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
| Critical | 3 |
| High Priority | 10 |
| Medium Priority | 12 |
| Low Priority | 8 |
| **총계** | **33** |

---

## 향후 Phase 로드맵

| Phase | 목표 | 상태 |
|-------|------|------|
| 12.5 | Bloom MIP 충돌 수정 | ✅ 완료 |
| 13 | Skin SSS + Eye 셰이더 | 대기 |
| 14 | Clustered Shading | 대기 |
| 15 | 코드베이스 정리 | 대기 |
| 16 | 섀도우 맵 통합 | 대기 |
| 17 | 레벨 에디터 완성 | 대기 |
