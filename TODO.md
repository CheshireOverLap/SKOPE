# SKOPE Engine - TODO

## 현재 진행 중: PBR 라이팅 디버깅

### 문제
- 디퍼드 렌더링에서 흰색 화면 발생
- 원인: D_GGX 함수가 낮은 roughness에서 폭발 (수천~수만 값)

### 완료된 작업
- [x] G-Buffer 정상 확인 (Albedo, Normal, Roughness, Metallic, Depth 모두 정상)
- [x] LightingUniform 구조체 정렬 수정 (Rust ↔ WGSL 192바이트 일치)
- [x] 런타임 PBR 파라미터 조절 UI 추가
  - intensity_scale: 라이트 강도 배율
  - d_ggx_max: GGX 스펙큘러 최대값 클램핑
  - specular_max: 전체 스펙큘러 최대값
  - roughness_min: 최소 러프니스 값
- [x] 디버그 시각화 모드 추가
  - Albedo, Normal, Depth, Metallic, Roughness
  - LightingRaw, LightingLog, LightingScaled
  - SimpleLambert (정상 작동 확인됨)
  - UniformValues (버퍼 정렬 검증용)
  - SpecularOnly, SpecularLog (스펙큘러 디버깅용)
- [x] Tonemapping Passthrough 모드 추가 (디버그 출력용)

### 다음 할 일
- [ ] SpecularOnly 모드에서 슬라이더 2,3,4 효과 확인
- [ ] 스펙큘러 값이 폭발하는 정확한 조건 파악
- [ ] d_ggx_max 적정값 찾기 (현재 기본값 16.0)
- [ ] 최종 라이팅 결과가 정상적으로 보이도록 수정

### 기술 노트

#### LightingUniform 구조체 (192 bytes)
```
Offset | Field
-------|------------------
0      | inv_view_proj (mat4x4)
64     | camera_position (vec4)
80     | sun_direction (vec4)
96     | sun_color (vec4)
112    | ambient_color (vec4)
128    | screen_size (vec2)
136    | time (f32)
140    | exposure (f32)
144    | intensity_scale (f32)
148    | d_ggx_max (f32)
152    | specular_max (f32)
156    | roughness_min (f32)
160    | debug_mode (u32)
164    | _pad1 (u32 x 3)
176    | _pad2 (u32 x 4)
```

#### 디버그 모드 매핑
- 0: None (정상 렌더링)
- 1-5: G-Buffer (Albedo, Normal, Roughness, Metallic, Depth)
- 6-8: Lighting (Raw, Log, Scaled)
- 10: UniformValues
- 11: SimpleLambert
- 12: SunColor
- 13: ExposureTime
- 14: SpecularOnly
- 15: SpecularLog
- 99: Passthrough (tonemapping 우회)
