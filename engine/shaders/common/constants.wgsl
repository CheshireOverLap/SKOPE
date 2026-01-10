// SKOPE Engine - Common Constants
//
// 모든 셰이더에서 공유되는 상수 정의

const PI: f32 = 3.14159265359;
const TAU: f32 = 6.28318530718;
const HALF_PI: f32 = 1.57079632679;
const INV_PI: f32 = 0.31830988618;

const EPSILON: f32 = 0.0000001;

// Invalid markers
const INVALID_TRIANGLE_ID: u32 = 0xFFFFFFFFu;
const INVALID_INDEX: u32 = 0xFFFFFFFFu;

// Light types
const LIGHT_TYPE_DIRECTIONAL: u32 = 0u;
const LIGHT_TYPE_POINT: u32 = 1u;
const LIGHT_TYPE_SPOT: u32 = 2u;
const LIGHT_TYPE_AREA: u32 = 3u;

// Cluster lighting
const MAX_LIGHTS_PER_CLUSTER: u32 = 64u;
