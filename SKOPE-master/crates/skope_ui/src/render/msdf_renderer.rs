//! MSDF (Multi-channel Signed Distance Field) 렌더링
//!
//! 고품질 텍스트/아이콘 렌더링을 위한 MSDF 파이프라인.
//! 스케일 독립적인 선명한 엣지를 제공합니다.

/// MSDF 텍스처 채널 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsdfChannelType {
    /// 단일 채널 SDF
    Sdf,
    /// 2채널 PSDF (Pseudo-SDF)
    Psdf,
    /// 3채널 MSDF (Multi-channel SDF)
    Msdf,
    /// 4채널 MTSDF (MSDF + True SDF)
    Mtsdf,
}

/// MSDF 글리프 데이터
#[derive(Debug, Clone)]
pub struct MsdfGlyph {
    /// 글리프 유니코드 코드포인트
    pub codepoint: u32,
    /// 아틀라스 내 위치 (UV)
    pub atlas_x: f32,
    pub atlas_y: f32,
    pub atlas_w: f32,
    pub atlas_h: f32,
    /// 플레인 바운드 (em 단위)
    pub plane_left: f32,
    pub plane_bottom: f32,
    pub plane_right: f32,
    pub plane_top: f32,
    /// 진행 폭 (em 단위)
    pub advance: f32,
}

impl MsdfGlyph {
    pub fn atlas_uv(&self) -> (f32, f32, f32, f32) {
        (self.atlas_x, self.atlas_y, self.atlas_w, self.atlas_h)
    }

    pub fn plane_bounds(&self) -> (f32, f32, f32, f32) {
        (self.plane_left, self.plane_bottom, self.plane_right, self.plane_top)
    }

    pub fn plane_size(&self) -> (f32, f32) {
        (self.plane_right - self.plane_left, self.plane_top - self.plane_bottom)
    }
}

/// MSDF 폰트 아틀라스
pub struct MsdfFontAtlas {
    glyphs: std::collections::HashMap<u32, MsdfGlyph>,
    atlas_width: u32,
    atlas_height: u32,
    channel_type: MsdfChannelType,
    pixel_range: f32,
    em_size: f32,
    line_height: f32,
    ascender: f32,
    descender: f32,
}

impl MsdfFontAtlas {
    pub fn new(width: u32, height: u32, channel_type: MsdfChannelType) -> Self {
        Self {
            glyphs: std::collections::HashMap::new(),
            atlas_width: width,
            atlas_height: height,
            channel_type,
            pixel_range: 2.0,
            em_size: 32.0,
            line_height: 1.2,
            ascender: 0.8,
            descender: -0.2,
        }
    }

    pub fn add_glyph(&mut self, glyph: MsdfGlyph) {
        self.glyphs.insert(glyph.codepoint, glyph);
    }

    pub fn get_glyph(&self, codepoint: u32) -> Option<&MsdfGlyph> {
        self.glyphs.get(&codepoint)
    }

    /// 문자열의 총 너비 계산 (em 단위)
    pub fn measure_text_em(&self, text: &str) -> f32 {
        text.chars()
            .filter_map(|c| self.glyphs.get(&(c as u32)))
            .map(|g| g.advance)
            .sum()
    }

    /// 문자열의 총 너비 (픽셀 단위)
    pub fn measure_text_px(&self, text: &str, font_size: f32) -> f32 {
        self.measure_text_em(text) * font_size
    }

    pub fn set_pixel_range(&mut self, range: f32) { self.pixel_range = range; }
    pub fn set_metrics(&mut self, em_size: f32, line_height: f32, ascender: f32, descender: f32) {
        self.em_size = em_size;
        self.line_height = line_height;
        self.ascender = ascender;
        self.descender = descender;
    }

    pub fn atlas_size(&self) -> (u32, u32) { (self.atlas_width, self.atlas_height) }
    pub fn channel_type(&self) -> MsdfChannelType { self.channel_type }
    pub fn pixel_range(&self) -> f32 { self.pixel_range }
    pub fn em_size(&self) -> f32 { self.em_size }
    pub fn line_height(&self) -> f32 { self.line_height }
    pub fn ascender(&self) -> f32 { self.ascender }
    pub fn descender(&self) -> f32 { self.descender }
    pub fn glyph_count(&self) -> usize { self.glyphs.len() }
}

/// MSDF 렌더링 파라미터
#[derive(Debug, Clone)]
pub struct MsdfRenderParams {
    /// 스크린 픽셀당 텍셀 비율
    pub screen_px_range: f32,
    /// 엣지 부드러움 (anti-aliasing)
    pub smoothing: f32,
    /// 아웃라인 두께 (0 = 없음)
    pub outline_width: f32,
    /// 아웃라인 색상
    pub outline_color: [f32; 4],
    /// 드롭 섀도 오프셋
    pub shadow_offset: [f32; 2],
    /// 드롭 섀도 부드러움
    pub shadow_softness: f32,
    /// 드롭 섀도 색상
    pub shadow_color: [f32; 4],
    /// 볼드 가중치 (-1..1)
    pub weight: f32,
}

impl Default for MsdfRenderParams {
    fn default() -> Self {
        Self {
            screen_px_range: 2.0,
            smoothing: 0.0,
            outline_width: 0.0,
            outline_color: [0.0, 0.0, 0.0, 1.0],
            shadow_offset: [0.0, 0.0],
            shadow_softness: 0.0,
            shadow_color: [0.0, 0.0, 0.0, 0.5],
            weight: 0.0,
        }
    }
}

impl MsdfRenderParams {
    pub fn with_outline(mut self, width: f32, color: [f32; 4]) -> Self {
        self.outline_width = width;
        self.outline_color = color;
        self
    }

    pub fn with_shadow(mut self, offset: [f32; 2], softness: f32, color: [f32; 4]) -> Self {
        self.shadow_offset = offset;
        self.shadow_softness = softness;
        self.shadow_color = color;
        self
    }

    pub fn with_weight(mut self, weight: f32) -> Self {
        self.weight = weight.clamp(-1.0, 1.0);
        self
    }

    pub fn has_outline(&self) -> bool { self.outline_width > 0.0 }
    pub fn has_shadow(&self) -> bool {
        self.shadow_offset[0].abs() > 0.001 || self.shadow_offset[1].abs() > 0.001
    }
}

/// MSDF 중간 거리(median) 계산
pub fn msdf_median(r: f32, g: f32, b: f32) -> f32 {
    r.max(g.min(b)).min(r.min(g).max(b))
}

/// MSDF 스크린 픽셀 범위 계산
pub fn compute_screen_px_range(font_size: f32, em_size: f32, pixel_range: f32) -> f32 {
    let scale = font_size / em_size;
    pixel_range * scale
}

/// MSDF 거리에서 opacity 계산
pub fn msdf_opacity(distance: f32, screen_px_range: f32) -> f32 {
    let px_dist = screen_px_range * (distance - 0.5);
    (px_dist + 0.5).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_atlas_with_glyphs() -> MsdfFontAtlas {
        let mut atlas = MsdfFontAtlas::new(512, 512, MsdfChannelType::Msdf);
        atlas.add_glyph(MsdfGlyph {
            codepoint: 'A' as u32,
            atlas_x: 0.0, atlas_y: 0.0, atlas_w: 0.05, atlas_h: 0.05,
            plane_left: 0.0, plane_bottom: -0.2, plane_right: 0.6, plane_top: 0.8,
            advance: 0.6,
        });
        atlas.add_glyph(MsdfGlyph {
            codepoint: 'B' as u32,
            atlas_x: 0.05, atlas_y: 0.0, atlas_w: 0.05, atlas_h: 0.05,
            plane_left: 0.0, plane_bottom: -0.2, plane_right: 0.55, plane_top: 0.8,
            advance: 0.55,
        });
        atlas
    }

    #[test]
    fn test_msdf_atlas_creation() {
        let atlas = MsdfFontAtlas::new(1024, 1024, MsdfChannelType::Msdf);
        assert_eq!(atlas.atlas_size(), (1024, 1024));
        assert_eq!(atlas.channel_type(), MsdfChannelType::Msdf);
        assert_eq!(atlas.glyph_count(), 0);
    }

    #[test]
    fn test_msdf_glyph_lookup() {
        let atlas = make_atlas_with_glyphs();
        assert_eq!(atlas.glyph_count(), 2);
        let g = atlas.get_glyph('A' as u32).unwrap();
        assert_eq!(g.advance, 0.6);
        assert!(atlas.get_glyph('Z' as u32).is_none());
    }

    #[test]
    fn test_msdf_measure_text() {
        let atlas = make_atlas_with_glyphs();
        let em_width = atlas.measure_text_em("AB");
        assert!((em_width - 1.15).abs() < 0.01); // 0.6 + 0.55
        let px_width = atlas.measure_text_px("AB", 16.0);
        assert!((px_width - 18.4).abs() < 0.1); // 1.15 * 16
    }

    #[test]
    fn test_msdf_median() {
        assert_eq!(msdf_median(0.3, 0.5, 0.7), 0.5);
        assert_eq!(msdf_median(0.1, 0.9, 0.5), 0.5);
        assert_eq!(msdf_median(0.8, 0.2, 0.6), 0.6);
    }

    #[test]
    fn test_msdf_opacity() {
        // distance > 0.5 → inside → visible
        assert!(msdf_opacity(0.8, 2.0) > 0.5);
        // distance < 0.5 → outside → not visible
        assert!(msdf_opacity(0.2, 2.0) < 0.5);
        // distance == 0.5 → edge
        assert!((msdf_opacity(0.5, 2.0) - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_msdf_render_params() {
        let params = MsdfRenderParams::default()
            .with_outline(2.0, [1.0, 0.0, 0.0, 1.0])
            .with_shadow([2.0, 2.0], 1.0, [0.0, 0.0, 0.0, 0.5])
            .with_weight(0.3);
        assert!(params.has_outline());
        assert!(params.has_shadow());
        assert_eq!(params.weight, 0.3);
    }

    #[test]
    fn test_screen_px_range() {
        let range = compute_screen_px_range(32.0, 32.0, 2.0);
        assert_eq!(range, 2.0); // scale=1
        let range2 = compute_screen_px_range(64.0, 32.0, 2.0);
        assert_eq!(range2, 4.0); // scale=2
    }

    #[test]
    fn test_glyph_plane_size() {
        let atlas = make_atlas_with_glyphs();
        let g = atlas.get_glyph('A' as u32).unwrap();
        let (w, h) = g.plane_size();
        assert!((w - 0.6).abs() < 0.01);
        assert!((h - 1.0).abs() < 0.01);
    }
}
