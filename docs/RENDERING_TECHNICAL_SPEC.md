# SKOPE Engine - Rendering Technical Specification

## 개요

SKOPE 엔진은 **Deferred Rendering** 파이프라인을 사용하며, wgpu 27.0 (Vulkan/DX12/Metal) 기반입니다.

---

## 1. 렌더링 파이프라인 흐름

```
┌─────────────────────────────────────────────────────────────────┐
│                    SKOPE Rendering Pipeline                      │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐       │
│  │ Geometry Pass│───▶│ Lighting Pass│───▶│  Blit Pass   │       │
│  │  (G-Buffer)  │    │  (Deferred)  │    │ (Tonemapping)│       │
│  └──────────────┘    └──────────────┘    └──────────────┘       │
│         │                   │                   │                │
│         ▼                   ▼                   ▼                │
│   G-Buffer RTs         HDR Buffer          Screen Output        │
│   (RGBA8/16F)         (RGBA16Float)         (Bgra8Unorm)        │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Pass 1: Geometry Pass
- **입력**: 메시, 머티리얼, 카메라
- **출력**: G-Buffer (3 render targets + depth)
- **셰이더**: `geometry_pass.wgsl`

### Pass 2: Lighting Pass
- **입력**: G-Buffer, 라이트 버퍼, 섀도우 맵
- **출력**: HDR Buffer (RGBA16Float)
- **셰이더**: `deferred_lighting.wgsl`

### Pass 3: Blit Pass
- **입력**: HDR Buffer
- **출력**: 화면 (LDR)
- **처리**: Bloom + ACES Tonemapping + Gamma Correction

---

## 2. G-Buffer 구성

| RT | Format | 채널 구성 | 설명 |
|----|--------|----------|------|
| RT0 | RGBA8Unorm | RGB: Albedo, A: Metallic | 기본 색상 + 금속성 |
| RT1 | **RGBA16Float** | RG: Normal (Octahedron), B: Roughness, A: ModelID | 노말 + 거칠기 |
| RT2 | RGBA8Unorm | RGB: Emission, A: AO | 발광 + 앰비언트 오클루전 |
| Depth | Depth32Float | Depth | 깊이 버퍼 |

### 노말 인코딩: Octahedron Encoding
```wgsl
// Encode (geometry_pass.wgsl)
fn encode_normal_octahedron(n: vec3<f32>) -> vec2<f32>

// Decode (deferred_lighting.wgsl)
fn decode_normal_octahedron(encoded: vec2<f32>) -> vec3<f32>
```

---

## 3. 라이팅 시스템

### 3.1 Light Types
```rust
enum LightType {
    Directional = 0,  // 태양, 달
    Point = 1,        // 전구, 횃불
    Spot = 2,         // 손전등
    AreaRect = 3,     // 사각형 면광원
    AreaDisk = 4,     // 원형 면광원
}
```

### 3.2 GpuLight 구조체 (80 bytes)
```rust
#[repr(C)]
struct GpuLight {
    position_type: [f32; 4],      // xyz: position, w: light_type
    direction_radius: [f32; 4],   // xyz: direction, w: radius
    color_intensity: [f32; 4],    // xyz: color, w: intensity  ← 여기!
    params0: [f32; 4],            // spot angles, area size
    params1: [f32; 4],            // source_radius, shadow_bias, etc.
}
```

### 3.3 Lighting Uniform
```rust
struct LightingUniform {
    inv_view_proj: mat4x4,
    camera_position: vec4,
    sun_direction: vec4,
    sun_color: vec4,           // w = sun_intensity (unused in shader)
    ambient_color: vec4,       // RGB = (0.03, 0.03, 0.05)
    screen_size: vec2,
    time: f32,
    exposure: f32,             // 기본값: 1.0
}
```

### 3.4 현재 라이트 설정값 (main.rs)
| Light | Type | Intensity | Color |
|-------|------|-----------|-------|
| Sun | Directional | 3.0 | (1.0, 0.98, 0.95) |
| Point 1 | Point | 5.0 | (1.0, 0.9, 0.8) |
| Point 2 | Point | 5.0 | (0.8, 0.9, 1.0) |
| Spot | Spot | 10.0 | (1.0, 1.0, 0.9) |

---

## 4. BRDF 구현 (Cook-Torrance)

### 4.1 공식
```
f_cook_torrance = DFG / (4 * (n·v) * (n·l))
```

### 4.2 현재 구현 (deferred_lighting.wgsl)

#### D: GGX/Trowbridge-Reitz NDF
```wgsl
fn d_ggx(n_dot_h: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let n_dot_h2 = n_dot_h * n_dot_h;
    let denom = n_dot_h2 * (a2 - 1.0) + 1.0;
    return a2 / (PI * denom * denom + 0.0001);
}
```

#### G: Smith's Method (Schlick-GGX)
```wgsl
fn g_schlick_ggx(n_dot_v: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;  // Direct lighting용 k
    return n_dot_v / (n_dot_v * (1.0 - k) + k + 0.0001);
}
```

#### F: Fresnel-Schlick
```wgsl
fn f_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (1.0 - f0) * pow(1.0 - cos_theta, 5.0);
}
```

---

## 5. 문제점 분석: 흰색 화면

### 5.1 데이터 흐름 추적

```
Rust (main.rs)                    WGSL (셰이더)
─────────────────                 ─────────────────
intensity = 3.0~10.0
        │
        ▼
GpuLight.color_intensity.w ──────▶ intensity = light.color_intensity.w
                                          │
                                          ▼
                                   radiance = color * intensity
                                          │
                                          ▼
                                   (diffuse + specular) * radiance * n_dot_l
                                          │
                                          ▼
                                   total_lighting (누적)
                                          │
                                          ▼
                                   * exposure (1.0)
                                          │
                                          ▼
                                   HDR Buffer (RGBA16Float)
                                          │
                                          ▼
                                   Blit: ACES Tonemapping
                                          │
                                          ▼
                                   화면 출력
```

### 5.2 잠재적 문제점

#### 문제 1: Intensity 단위 불일치
- **현재**: intensity = 3~10 (임의 단위)
- **문제**: PBR에서 intensity는 물리 단위 (lux, candela)여야 함
- **증상**: 라이팅 결과가 HDR 범위를 초과 → 톤매핑 후에도 흰색

#### 문제 2: Cook-Torrance Specular 폭발
- **D_GGX**: roughness가 낮을 때 (0.04) 값이 수천~수만까지 증가
- **분모**: `4 * n_dot_v * n_dot_l`이 0에 가까우면 specular 폭발

#### 문제 3: 이중 라이팅?
- Blit Pass에서 `hdr + bloom` 후 ACES 적용
- Bloom이 이미 밝은 영역을 더 밝게 만듦

#### 문제 4: G-Buffer Clear 값
```rust
// normal_roughness RT clear:
r: 0.5, g: 0.5, b: 1.0, a: 0.5
```
- 빈 픽셀의 roughness = 0.5 (b 채널이 아닌 a 채널이 clear)
- **문제**: b = 1.0이면 roughness = 1.0 → 예상과 다름

---

## 6. 수정 권장사항

### 6.1 Intensity 정규화
```wgsl
// 현재 (임시 해결책)
let intensity_scale = 0.15;
let intensity = light.color_intensity.w * intensity_scale;

// 권장: Rust 측에서 수정
// Directional: 1.0 = 태양광 기준
// Point: candela 단위 사용, attenuation에서 감쇄
```

### 6.2 Specular 클램핑
```wgsl
// 현재 추가된 코드
return clamp(specular, vec3(0.0), vec3(10.0));

// 더 나은 방법: roughness minimum 보장
let roughness = max(normal_roughness.b, 0.1);
```

### 6.3 권장 라이트 강도 (참고: Filament, Unreal)
| Light Type | Recommended Intensity |
|------------|----------------------|
| Directional (Sun) | 80,000~120,000 lux → 정규화 후 1.0~2.0 |
| Point (100W bulb) | ~1,700 lumen → 정규화 후 0.1~0.5 |
| Spot | Similar to point |

---

## 7. 파일 구조

```
src/
├── renderer/
│   ├── mod.rs           # Renderer 메인 (파이프라인)
│   ├── gbuffer.rs       # G-Buffer 정의
│   └── resources.rs     # Uniform 구조체들
├── lighting/
│   ├── mod.rs           # Lighting 시스템
│   ├── lights.rs        # Light types + GpuLight
│   └── pipeline.rs      # Lighting pipeline
├── shaders/
│   ├── geometry_pass.wgsl    # G-Buffer 출력
│   ├── deferred_lighting.wgsl # 라이팅 계산
│   └── (기타 30개 셰이더)
└── main.rs              # 앱 엔트리 + 라이트 설정
```

---

## 8. 디버그 방법

### 셰이더 디버그 출력
```wgsl
// geometry_pass.wgsl에서 G-Buffer 확인
// deferred_lighting.wgsl 상단에 추가:

// 1. Albedo만 출력
return vec4<f32>(albedo, 1.0);

// 2. Normal 시각화
return vec4<f32>(n * 0.5 + 0.5, 1.0);

// 3. Simple Lambert (라이트 버퍼 무시)
let simple_light_dir = normalize(vec3<f32>(1.0, 1.0, 0.5));
let simple_ndotl = max(dot(n, simple_light_dir), 0.0);
return vec4<f32>(albedo * (0.2 + 0.8 * simple_ndotl), 1.0);
```

---

## 9. 참고 자료

- [LearnOpenGL - PBR Theory](https://learnopengl.com/PBR/Theory)
- [Filament Material Guide](https://google.github.io/filament/Materials.html)
- [Naty Hoffman - Physics and Math of Shading](https://blog.selfshadow.com/publications/s2013-shading-course/)
