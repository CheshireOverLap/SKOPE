//! SDF Text Renderer
//!
//! Signed Distance Field 기반 텍스트 렌더링.
//! 비트맵 글리프를 SDF로 변환하여 스케일 독립적으로 선명하게 표시.

/// SDF 글리프 아틀라스 엔트리
#[derive(Debug, Clone)]
pub struct SdfGlyph {
    /// 유니코드 코드포인트
    pub codepoint: char,
    /// 아틀라스 내 UV 영역 (x, y, w, h) — 0.0~1.0
    pub uv_rect: [f32; 4],
    /// 원본 글리프 메트릭 (픽셀)
    pub advance: f32,
    pub bearing_x: f32,
    pub bearing_y: f32,
    pub width: f32,
    pub height: f32,
}

/// SDF 아틀라스 크기
const SDF_ATLAS_SIZE: u32 = 2048;
/// SDF 생성 시 패딩 (거리 필드 여유)
const SDF_PADDING: u32 = 4;
/// SDF 스프레드 (거리 필드 범위 픽셀)
const SDF_SPREAD: f32 = 8.0;

/// Dead-reckoning sweep 기반 SDF 생성
///
/// 비트맵(alpha threshold) → signed distance field.
/// O(w*h) 2-pass sweep.
pub fn generate_sdf(bitmap: &[u8], width: u32, height: u32, spread: f32) -> Vec<u8> {
    let w = width as usize;
    let h = height as usize;
    let max_dist = spread;

    // inside/outside 거리 계산
    let mut dist_outside = vec![f32::MAX; w * h];
    let mut dist_inside = vec![f32::MAX; w * h];

    // 초기화: 에지 픽셀 = 0, 나머지 = MAX
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            let is_inside = bitmap[idx] > 127;

            if is_inside {
                dist_outside[idx] = f32::MAX;
                dist_inside[idx] = 0.0;
            } else {
                dist_outside[idx] = 0.0;
                dist_inside[idx] = f32::MAX;
            }

            // 에지 감지: 인접 픽셀과 상태가 다르면 서브픽셀 거리
            if x > 0 {
                let neighbor_inside = bitmap[idx - 1] > 127;
                if is_inside != neighbor_inside {
                    dist_outside[idx] = dist_outside[idx].min(0.5);
                    dist_inside[idx] = dist_inside[idx].min(0.5);
                }
            }
            if y > 0 {
                let neighbor_inside = bitmap[(y - 1) * w + x] > 127;
                if is_inside != neighbor_inside {
                    dist_outside[idx] = dist_outside[idx].min(0.5);
                    dist_inside[idx] = dist_inside[idx].min(0.5);
                }
            }
        }
    }

    // 순방향 sweep (좌상→우하)
    sweep_forward(&mut dist_outside, w, h);
    sweep_forward(&mut dist_inside, w, h);

    // 역방향 sweep (우하→좌상)
    sweep_backward(&mut dist_outside, w, h);
    sweep_backward(&mut dist_inside, w, h);

    // signed distance → [0, 255]
    let mut result = vec![0u8; w * h];
    for i in 0..w * h {
        let d = dist_inside[i].sqrt() - dist_outside[i].sqrt();
        let normalized = (d / max_dist + 1.0) * 0.5; // -spread..+spread → 0..1
        result[i] = (normalized.clamp(0.0, 1.0) * 255.0) as u8;
    }

    result
}

fn sweep_forward(dist: &mut [f32], w: usize, h: usize) {
    for y in 0..h {
        for x in 0..w {
            let idx = y * w + x;
            if x > 0 {
                let d = dist[idx - 1] + 1.0;
                dist[idx] = dist[idx].min(d);
            }
            if y > 0 {
                let d = dist[(y - 1) * w + x] + 1.0;
                dist[idx] = dist[idx].min(d);
            }
            if x > 0 && y > 0 {
                let d = dist[(y - 1) * w + (x - 1)] + 1.414;
                dist[idx] = dist[idx].min(d);
            }
            if x + 1 < w && y > 0 {
                let d = dist[(y - 1) * w + (x + 1)] + 1.414;
                dist[idx] = dist[idx].min(d);
            }
        }
    }
}

fn sweep_backward(dist: &mut [f32], w: usize, h: usize) {
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let idx = y * w + x;
            if x + 1 < w {
                let d = dist[idx + 1] + 1.0;
                dist[idx] = dist[idx].min(d);
            }
            if y + 1 < h {
                let d = dist[(y + 1) * w + x] + 1.0;
                dist[idx] = dist[idx].min(d);
            }
            if x + 1 < w && y + 1 < h {
                let d = dist[(y + 1) * w + (x + 1)] + 1.414;
                dist[idx] = dist[idx].min(d);
            }
            if x > 0 && y + 1 < h {
                let d = dist[(y + 1) * w + (x - 1)] + 1.414;
                dist[idx] = dist[idx].min(d);
            }
        }
    }
}

/// SDF 텍스트 렌더러
///
/// wgpu 파이프라인 + SDF 아틀라스를 관리.
pub struct SdfTextRenderer {
    /// SDF 글리프 캐시
    glyphs: std::collections::HashMap<(char, u32), SdfGlyph>,
    /// 아틀라스 데이터 (CPU side)
    atlas_data: Vec<u8>,
    /// 아틀라스 크기
    atlas_size: u32,
    /// 다음 배치 위치
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
}

impl SdfTextRenderer {
    pub fn new() -> Self {
        Self {
            glyphs: std::collections::HashMap::new(),
            atlas_data: vec![0u8; (SDF_ATLAS_SIZE * SDF_ATLAS_SIZE) as usize],
            atlas_size: SDF_ATLAS_SIZE,
            cursor_x: 0,
            cursor_y: 0,
            row_height: 0,
        }
    }

    /// 글리프 SDF 등록 (비트맵 → SDF 변환 후 아틀라스에 배치)
    pub fn add_glyph(
        &mut self,
        codepoint: char,
        font_size: u32,
        bitmap: &[u8],
        width: u32,
        height: u32,
        advance: f32,
        bearing_x: f32,
        bearing_y: f32,
    ) -> Option<&SdfGlyph> {
        let key = (codepoint, font_size);
        if self.glyphs.contains_key(&key) {
            return self.glyphs.get(&key);
        }

        let padded_w = width + SDF_PADDING * 2;
        let padded_h = height + SDF_PADDING * 2;

        // 줄바꿈 체크
        if self.cursor_x + padded_w > self.atlas_size {
            self.cursor_x = 0;
            self.cursor_y += self.row_height;
            self.row_height = 0;
        }
        if self.cursor_y + padded_h > self.atlas_size {
            return None; // 아틀라스 가득 참
        }

        // 패딩된 비트맵 생성
        let mut padded = vec![0u8; (padded_w * padded_h) as usize];
        for y in 0..height {
            for x in 0..width {
                padded[((y + SDF_PADDING) * padded_w + (x + SDF_PADDING)) as usize] =
                    bitmap[(y * width + x) as usize];
            }
        }

        // SDF 생성
        let sdf = generate_sdf(&padded, padded_w, padded_h, SDF_SPREAD);

        // 아틀라스에 복사
        for y in 0..padded_h {
            for x in 0..padded_w {
                let atlas_x = self.cursor_x + x;
                let atlas_y = self.cursor_y + y;
                self.atlas_data[(atlas_y * self.atlas_size + atlas_x) as usize] =
                    sdf[(y * padded_w + x) as usize];
            }
        }

        let uv_rect = [
            self.cursor_x as f32 / self.atlas_size as f32,
            self.cursor_y as f32 / self.atlas_size as f32,
            padded_w as f32 / self.atlas_size as f32,
            padded_h as f32 / self.atlas_size as f32,
        ];

        let glyph = SdfGlyph {
            codepoint,
            uv_rect,
            advance,
            bearing_x,
            bearing_y,
            width: padded_w as f32,
            height: padded_h as f32,
        };

        self.cursor_x += padded_w;
        self.row_height = self.row_height.max(padded_h);

        self.glyphs.insert(key, glyph);
        self.glyphs.get(&key)
    }

    /// 글리프 조회
    pub fn get_glyph(&self, codepoint: char, font_size: u32) -> Option<&SdfGlyph> {
        self.glyphs.get(&(codepoint, font_size))
    }

    /// 아틀라스 데이터 (GPU 업로드용)
    pub fn atlas_data(&self) -> &[u8] {
        &self.atlas_data
    }

    /// 아틀라스 크기
    pub fn atlas_size(&self) -> u32 {
        self.atlas_size
    }
}
