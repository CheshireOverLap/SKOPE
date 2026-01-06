//! Orientation Gizmo (유니티 스타일)
//!
//! 뷰포트 우측 상단에 표시되는 XYZ 축 방향 표시기
//! - 6개의 원뿔 (양/음 방향)
//! - 마주보는 축 쌍이 함께 페이드

use egui::{Color32, Pos2, Rect, Stroke, Ui, pos2, vec2};

/// Orientation Gizmo 설정
pub struct OrientationGizmoConfig {
    pub size: f32,
    pub margin: f32,
    pub axis_length: f32,
    pub cone_length: f32,
    pub cone_width: f32,
    pub cube_size: f32,
}

impl Default for OrientationGizmoConfig {
    fn default() -> Self {
        Self {
            size: 70.0,
            margin: 15.0,
            axis_length: 22.0,
            cone_length: 10.0,
            cone_width: 5.0,
            cube_size: 5.0,
        }
    }
}

/// 축 정보
struct AxisInfo {
    label: Option<char>,  // None이면 음수 방향 (레이블 없음)
    dir_2d: (f32, f32),
    depth: f32,
    color: Color32,
}

/// Orientation Gizmo 그리기
pub fn draw(
    ui: &Ui,
    viewport_rect: Rect,
    view_matrix: [[f32; 4]; 4],
    config: &OrientationGizmoConfig,
) {
    let center = pos2(
        viewport_rect.right() - config.margin - config.size / 2.0,
        viewport_rect.top() + config.margin + config.size / 2.0,
    );

    // View matrix에서 축 방향 추출 → 2D 투영
    let project_axis = |col: usize, neg: bool| -> (f32, f32, f32) {
        let sign = if neg { -1.0 } else { 1.0 };
        (
            view_matrix[col][0] * sign,
            -view_matrix[col][1] * sign,  // Y 반전 (egui 좌표계)
            view_matrix[col][2] * sign,
        )
    };

    // 축 색상
    let colors = [
        Color32::from_rgb(240, 75, 75),   // X: 빨강
        Color32::from_rgb(130, 210, 80),  // Y: 초록
        Color32::from_rgb(70, 150, 255),  // Z: 파랑
    ];

    // 6개의 축 (양/음 방향)
    let mut axes: Vec<AxisInfo> = Vec::with_capacity(6);

    for (i, &color) in colors.iter().enumerate() {
        // 양수 방향 (+X, +Y, +Z)
        let (px, py, pz) = project_axis(i, false);
        axes.push(AxisInfo {
            label: Some(['x', 'y', 'z'][i]),
            dir_2d: (px, py),
            depth: pz,
            color,
        });

        // 음수 방향 (-X, -Y, -Z) - 더 어두운 색상
        let (nx, ny, nz) = project_axis(i, true);
        axes.push(AxisInfo {
            label: None,
            dir_2d: (nx, ny),
            depth: nz,
            color: darken_color(color),
        });
    }

    // 깊이순 정렬 (뒤에서 앞으로)
    axes.sort_by(|a, b| a.depth.partial_cmp(&b.depth).unwrap());

    let painter = ui.painter();

    // 각 축 그리기
    for axis in &axes {
        // 2D 투영 길이로 투명도 계산 (카메라와 수직이면 짧음 → 투명)
        let len_2d = (axis.dir_2d.0.powi(2) + axis.dir_2d.1.powi(2)).sqrt();
        let alpha = projection_length_to_alpha(len_2d);

        let axis_color = Color32::from_rgba_unmultiplied(
            axis.color.r(), axis.color.g(), axis.color.b(), alpha
        );

        let end = center + vec2(axis.dir_2d.0 * config.axis_length, axis.dir_2d.1 * config.axis_length);

        // 축 라인
        painter.line_segment([center, end], Stroke::new(1.5, axis_color));

        // 원뿔 (너무 짧으면 스킵)
        if len_2d > 0.15 {
            draw_cone(painter, end, axis.dir_2d, axis_color, config);
        }

        // 양수 방향만 레이블 표시 (충분히 보일 때)
        if let Some(label) = axis.label {
            if len_2d > 0.5 {
                let (nx, ny) = (axis.dir_2d.0 / len_2d, axis.dir_2d.1 / len_2d);
                painter.text(
                    end + vec2(nx * (config.cone_length + 6.0), ny * (config.cone_length + 6.0)),
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(11.0),
                    axis_color,
                );
            }
        }
    }

    // 중앙 큐브
    let cube_rect = Rect::from_center_size(center, vec2(config.cube_size * 2.0, config.cube_size * 2.0));
    painter.rect_filled(cube_rect, 1.0, Color32::from_rgb(200, 200, 200));

    // 뷰 모드 표시
    painter.text(
        pos2(center.x, center.y + config.size / 2.0 + 8.0),
        egui::Align2::CENTER_CENTER,
        "Persp",
        egui::FontId::proportional(10.0),
        Color32::from_rgb(150, 150, 150),
    );
}

/// 원뿔 그리기
fn draw_cone(
    painter: &egui::Painter,
    base: Pos2,
    dir: (f32, f32),
    color: Color32,
    config: &OrientationGizmoConfig,
) {
    let len = (dir.0.powi(2) + dir.1.powi(2)).sqrt();
    if len < 0.01 {
        return;
    }

    let (nx, ny) = (dir.0 / len, dir.1 / len);
    let tip = base + vec2(nx * config.cone_length, ny * config.cone_length);
    let perp = vec2(-ny * config.cone_width, nx * config.cone_width);

    painter.add(egui::Shape::convex_polygon(
        vec![tip, base + perp, base - perp],
        color,
        Stroke::NONE,
    ));
}

/// 2D 투영 길이를 투명도로 변환
/// len_2d: 0(카메라와 수직) ~ 1(화면과 평행)
fn projection_length_to_alpha(len_2d: f32) -> u8 {
    // 짧으면 투명, 길면 불투명
    (len_2d * 255.0).clamp(30.0, 255.0) as u8
}

/// 색상을 어둡게 (음수 방향용)
fn darken_color(color: Color32) -> Color32 {
    Color32::from_rgb(
        (color.r() as f32 * 0.5) as u8,
        (color.g() as f32 * 0.5) as u8,
        (color.b() as f32 * 0.5) as u8,
    )
}
