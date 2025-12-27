// SKOPE Engine - Marschner Hair BCSDF
// Phase 12: Physically Based Hair Shading

use bytemuck::{Pod, Zeroable};

/// Marschner BCSDF 파라미터
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct MarschnerParams {
    /// 흡수 계수 (머리카락 색상)
    pub sigma_a: [f32; 3],
    /// 큐티클 기울기 (degrees, 보통 -3 ~ 3)
    pub alpha: f32,

    /// 큐티클 거칠기 (R lobe용)
    pub beta_r: f32,
    /// TT lobe 거칠기
    pub beta_tt: f32,
    /// TRT lobe 거칠기
    pub beta_trt: f32,
    /// 굴절률 (보통 1.55)
    pub eta: f32,

    /// R lobe 강도
    pub r_intensity: f32,
    /// TT lobe 강도
    pub tt_intensity: f32,
    /// TRT lobe 강도
    pub trt_intensity: f32,
    pub _pad: f32,
}

impl Default for MarschnerParams {
    fn default() -> Self {
        Self {
            // 갈색 머리카락
            sigma_a: [0.4, 0.6, 1.0],
            alpha: -2.0,  // degrees

            beta_r: 5.0,    // degrees
            beta_tt: 10.0,
            beta_trt: 20.0,
            eta: 1.55,

            r_intensity: 1.0,
            tt_intensity: 0.5,
            trt_intensity: 0.8,
            _pad: 0.0,
        }
    }
}

impl MarschnerParams {
    /// 검은 머리카락
    pub fn black() -> Self {
        Self {
            sigma_a: [1.5, 2.0, 2.5],
            ..Default::default()
        }
    }

    /// 금발
    pub fn blonde() -> Self {
        Self {
            sigma_a: [0.06, 0.1, 0.3],
            ..Default::default()
        }
    }

    /// 빨간 머리
    pub fn red() -> Self {
        Self {
            sigma_a: [0.15, 0.8, 1.2],
            ..Default::default()
        }
    }

    /// 흰 머리 (은발)
    pub fn white() -> Self {
        Self {
            sigma_a: [0.01, 0.01, 0.02],
            beta_r: 8.0,  // 약간 더 거친 스펙큘러
            ..Default::default()
        }
    }
}

/// CPU 측 Marschner 계산 (디버깅/프리뷰용)
pub mod cpu {
    use glam::Vec3;

    /// 가우시안 분포
    fn gaussian(x: f32, mean: f32, variance: f32) -> f32 {
        let diff = x - mean;
        (-diff * diff / (2.0 * variance)).exp()
    }

    /// Longitudinal scattering (M term)
    pub fn m_term(theta_h: f32, alpha: f32, beta: f32) -> f32 {
        let beta_rad = beta.to_radians();
        let alpha_rad = alpha.to_radians();
        let variance = beta_rad * beta_rad;
        gaussian(theta_h, alpha_rad, variance)
    }

    /// Azimuthal scattering (N term) - 간소화 버전
    pub fn n_term(phi: f32, _eta: f32, _p: i32) -> f32 {
        // 정확한 구현은 Fresnel과 Bravais 굴절 필요
        // 여기서는 간소화된 버전 사용
        let cos_phi_half = (phi / 2.0).cos();
        cos_phi_half.abs().powf(2.0)
    }

    /// Attenuation (A term)
    pub fn attenuation(sigma_a: Vec3, cos_theta: f32) -> Vec3 {
        let path_length = 2.0 / cos_theta.abs().max(0.01);
        Vec3::new(
            (-sigma_a.x * path_length).exp(),
            (-sigma_a.y * path_length).exp(),
            (-sigma_a.z * path_length).exp(),
        )
    }

    /// 간소화된 Marschner BCSDF
    pub fn marschner_simplified(
        light_dir: Vec3,
        view_dir: Vec3,
        tangent: Vec3,
        sigma_a: Vec3,
        alpha: f32,
        beta_r: f32,
        beta_tt: f32,
        beta_trt: f32,
    ) -> Vec3 {
        // Tangent 기반 각도 계산
        let cos_theta_i = light_dir.dot(tangent);
        let cos_theta_o = view_dir.dot(tangent);
        let theta_i = cos_theta_i.acos();
        let theta_o = cos_theta_o.acos();
        let theta_h = (theta_i + theta_o) / 2.0;
        let theta_d = (theta_i - theta_o) / 2.0;

        // Azimuth 각도 (간소화)
        let phi = 0.0f32; // 정확한 계산은 더 복잡함

        // R lobe (직접 반사)
        let m_r = m_term(theta_h, alpha, beta_r);
        let n_r = n_term(phi, 1.55, 0);
        let r = m_r * n_r;

        // TT lobe (투과-투과)
        let m_tt = m_term(theta_h, -alpha / 2.0, beta_tt);
        let n_tt = n_term(phi, 1.55, 1);
        let a_tt = attenuation(sigma_a, theta_d.cos());
        let tt = m_tt * n_tt * a_tt;

        // TRT lobe (투과-반사-투과)
        let m_trt = m_term(theta_h, -3.0 * alpha / 2.0, beta_trt);
        let n_trt = n_term(phi, 1.55, 2);
        let a_trt = attenuation(sigma_a * 2.0, theta_d.cos());
        let trt = m_trt * n_trt * a_trt;

        // 합산 (화이트 스펙큘러)
        let spec_white = Vec3::splat(r);
        spec_white + tt + trt
    }
}
