// SKOPE UI - Layout Engine

use crate::types::*;

/// 위젯 레이아웃 계산
pub fn calculate_widget_layout(widget: &mut Widget, parent_rect: &Rect, config: &UiConfig) {
    // 스케일 계산
    let scale = calculate_scale(parent_rect, config);

    // 현재 위젯의 rect 계산
    widget.computed_rect = calculate_rect(widget, parent_rect, scale);

    // 자식 위젯들의 레이아웃 계산
    let content_rect = get_content_rect(&widget.computed_rect, &widget.layout.padding);

    match widget.layout.flex_direction {
        FlexDirection::Row | FlexDirection::RowReverse => {
            layout_flex_row(&mut widget.children, &content_rect, &widget.layout, scale);
        }
        FlexDirection::Column | FlexDirection::ColumnReverse => {
            layout_flex_column(&mut widget.children, &content_rect, &widget.layout, scale);
        }
    }

    // 자식들도 재귀적으로 계산
    // 앵커 기반 레이아웃은 부모의 content_rect를 기준으로 계산
    for child in &mut widget.children {
        calculate_widget_layout(child, &content_rect, config);
    }

    // ScrollView인 경우 자식들에게 스크롤 오프셋 적용 및 컨텐츠 크기 계산
    if matches!(widget.widget_type, WidgetType::ScrollView { .. }) {
        // 컨텐츠 크기 계산 (자식들의 최대 범위)
        let mut max_x: f32 = 0.0;
        let mut max_y: f32 = 0.0;

        for child in &widget.children {
            let child_right = child.computed_rect.x + child.computed_rect.width - widget.computed_rect.x;
            let child_bottom = child.computed_rect.y + child.computed_rect.height - widget.computed_rect.y;
            max_x = max_x.max(child_right);
            max_y = max_y.max(child_bottom);
        }

        widget.content_size = (max_x, max_y);

        // 스크롤 오프셋을 자식들의 위치에 적용
        let (scroll_x, scroll_y) = widget.scroll_offset;
        apply_scroll_offset(&mut widget.children, -scroll_x, -scroll_y);
    }
}

/// 자식 위젯들에게 스크롤 오프셋 재귀적으로 적용
fn apply_scroll_offset(children: &mut [Widget], offset_x: f32, offset_y: f32) {
    for child in children {
        child.computed_rect.x += offset_x;
        child.computed_rect.y += offset_y;

        // 자식의 자식들에게도 적용 (재귀)
        apply_scroll_offset(&mut child.children, 0.0, 0.0); // 이미 부모 오프셋이 적용됨
    }
}

/// 스케일 계산
fn calculate_scale(screen_rect: &Rect, config: &UiConfig) -> f32 {
    let (ref_w, ref_h) = config.reference_resolution;
    let screen_w = screen_rect.width;
    let screen_h = screen_rect.height;

    let scale = match config.scale_mode {
        ScaleMode::ScaleWithWidth => screen_w / ref_w,
        ScaleMode::ScaleWithHeight => screen_h / ref_h,
        ScaleMode::ScaleWithMin => (screen_w / ref_w).min(screen_h / ref_h),
        ScaleMode::ScaleWithMax => (screen_w / ref_w).max(screen_h / ref_h),
        ScaleMode::FixedPixel => 1.0,
    };

    scale.clamp(config.min_scale, config.max_scale)
}

/// 개별 위젯의 rect 계산
fn calculate_rect(widget: &Widget, parent_rect: &Rect, scale: f32) -> Rect {
    let layout = &widget.layout;

    // 앵커 포인트 계산
    let (anchor_x, anchor_y) = layout.anchor.to_normalized();

    // 부모 내에서 앵커 위치
    let anchor_pos_x = parent_rect.x + parent_rect.width * anchor_x;
    let anchor_pos_y = parent_rect.y + parent_rect.height * anchor_y;

    // 크기 계산
    let (width, height) = calculate_size(widget, parent_rect, scale);

    // 피벗 적용
    let (pivot_x, pivot_y) = layout.pivot;
    let final_x = anchor_pos_x + layout.offset.0 * scale - width * pivot_x;
    let final_y = anchor_pos_y + layout.offset.1 * scale - height * pivot_y;

    // 마진 적용
    let margin = &layout.margin;
    Rect {
        x: final_x + margin.left * scale,
        y: final_y + margin.top * scale,
        width: width - (margin.left + margin.right) * scale,
        height: height - (margin.top + margin.bottom) * scale,
    }
}

/// 크기 계산
fn calculate_size(widget: &Widget, parent_rect: &Rect, scale: f32) -> (f32, f32) {
    match &widget.layout.size {
        Size::Fixed(w, h) => (*w * scale, *h * scale),
        Size::Percent(w, h) => (parent_rect.width * w / 100.0, parent_rect.height * h / 100.0),
        Size::Fill => (parent_rect.width, parent_rect.height),
        Size::FitContent => calculate_content_size(widget, scale),
        Size::WidthFixed(w) => {
            let h = calculate_content_size(widget, scale).1;
            (*w * scale, h)
        }
        Size::HeightFixed(h) => {
            let w = calculate_content_size(widget, scale).0;
            (w, *h * scale)
        }
    }
}

/// 컨텐츠 크기 계산 (자식들 기반)
fn calculate_content_size(widget: &Widget, scale: f32) -> (f32, f32) {
    // 위젯 타입에 따른 기본 크기
    let base_size = match &widget.widget_type {
        WidgetType::Text { content, font_size, .. } => {
            let fs = font_size.unwrap_or(16.0) * scale;
            // 대략적인 텍스트 크기 계산 (실제로는 폰트 메트릭 필요)
            let char_width = fs * 0.6;
            let width = content.len() as f32 * char_width;
            (width, fs * 1.2)
        }
        WidgetType::Image { .. } => (100.0 * scale, 100.0 * scale), // 기본 크기
        WidgetType::Button { text, .. } => {
            if let Some(text) = text {
                let fs = 16.0 * scale;
                let width = text.len() as f32 * fs * 0.6 + 32.0 * scale; // 패딩 포함
                (width, fs * 2.5)
            } else {
                (100.0 * scale, 40.0 * scale)
            }
        }
        _ => (0.0, 0.0),
    };

    // 자식들의 크기도 고려
    if widget.children.is_empty() {
        return base_size;
    }

    let padding = &widget.layout.padding;
    let gap = widget.layout.gap * scale;

    let (mut total_w, mut total_h) = (0.0f32, 0.0f32);

    for child in &widget.children {
        let (cw, ch) = calculate_content_size(child, scale);
        match widget.layout.flex_direction {
            FlexDirection::Row | FlexDirection::RowReverse => {
                total_w += cw + gap;
                total_h = total_h.max(ch);
            }
            FlexDirection::Column | FlexDirection::ColumnReverse => {
                total_w = total_w.max(cw);
                total_h += ch + gap;
            }
        }
    }

    // 마지막 gap 제거
    if !widget.children.is_empty() {
        match widget.layout.flex_direction {
            FlexDirection::Row | FlexDirection::RowReverse => total_w -= gap,
            FlexDirection::Column | FlexDirection::ColumnReverse => total_h -= gap,
        }
    }

    // 패딩 추가
    total_w += (padding.left + padding.right) * scale;
    total_h += (padding.top + padding.bottom) * scale;

    // 기본 크기와 비교
    (total_w.max(base_size.0), total_h.max(base_size.1))
}

/// 패딩을 제외한 컨텐츠 영역 계산
fn get_content_rect(rect: &Rect, padding: &Edges) -> Rect {
    Rect {
        x: rect.x + padding.left,
        y: rect.y + padding.top,
        width: (rect.width - padding.left - padding.right).max(0.0),
        height: (rect.height - padding.top - padding.bottom).max(0.0),
    }
}

/// Flex Row 레이아웃
fn layout_flex_row(children: &mut [Widget], content_rect: &Rect, parent_layout: &Layout, scale: f32) {
    if children.is_empty() {
        return;
    }

    let gap = parent_layout.gap * scale;

    // 자식들의 총 크기 계산
    let mut total_width = 0.0;
    let sizes: Vec<(f32, f32)> = children
        .iter()
        .map(|c| calculate_content_size(c, scale))
        .collect();

    for (w, _) in &sizes {
        total_width += *w;
    }
    total_width += gap * (children.len() - 1) as f32;

    // 시작 위치 계산 (justify-content)
    let remaining = content_rect.width - total_width;
    let (mut x, extra_gap) = match parent_layout.justify_content {
        JustifyContent::Start => (content_rect.x, 0.0),
        JustifyContent::End => (content_rect.x + remaining, 0.0),
        JustifyContent::Center => (content_rect.x + remaining / 2.0, 0.0),
        JustifyContent::SpaceBetween => {
            let extra = remaining / (children.len() - 1).max(1) as f32;
            (content_rect.x, extra)
        }
        JustifyContent::SpaceAround => {
            let extra = remaining / children.len() as f32;
            (content_rect.x + extra / 2.0, extra)
        }
        JustifyContent::SpaceEvenly => {
            let extra = remaining / (children.len() + 1) as f32;
            (content_rect.x + extra, extra)
        }
    };

    // Reverse 처리
    let iter: Box<dyn Iterator<Item = (&mut Widget, (f32, f32))>> =
        if matches!(parent_layout.flex_direction, FlexDirection::RowReverse) {
            Box::new(children.iter_mut().zip(sizes.into_iter()).rev())
        } else {
            Box::new(children.iter_mut().zip(sizes.into_iter()))
        };

    for (child, (w, h)) in iter {
        // 교차축 정렬 (align-items)
        let y = match parent_layout.align_items {
            AlignItems::Start => content_rect.y,
            AlignItems::End => content_rect.y + content_rect.height - h,
            AlignItems::Center => content_rect.y + (content_rect.height - h) / 2.0,
            AlignItems::Stretch => content_rect.y,
        };

        let height = if matches!(parent_layout.align_items, AlignItems::Stretch) {
            content_rect.height
        } else {
            h
        };

        child.computed_rect = Rect {
            x,
            y,
            width: w,
            height,
        };

        x += w + gap + extra_gap;
    }
}

/// Flex Column 레이아웃
fn layout_flex_column(children: &mut [Widget], content_rect: &Rect, parent_layout: &Layout, scale: f32) {
    if children.is_empty() {
        return;
    }

    let gap = parent_layout.gap * scale;

    // 자식들의 총 크기 계산
    let mut total_height = 0.0;
    let sizes: Vec<(f32, f32)> = children
        .iter()
        .map(|c| calculate_content_size(c, scale))
        .collect();

    for (_, h) in &sizes {
        total_height += *h;
    }
    total_height += gap * (children.len() - 1) as f32;

    // 시작 위치 계산 (justify-content)
    let remaining = content_rect.height - total_height;
    let (mut y, extra_gap) = match parent_layout.justify_content {
        JustifyContent::Start => (content_rect.y, 0.0),
        JustifyContent::End => (content_rect.y + remaining, 0.0),
        JustifyContent::Center => (content_rect.y + remaining / 2.0, 0.0),
        JustifyContent::SpaceBetween => {
            let extra = remaining / (children.len() - 1).max(1) as f32;
            (content_rect.y, extra)
        }
        JustifyContent::SpaceAround => {
            let extra = remaining / children.len() as f32;
            (content_rect.y + extra / 2.0, extra)
        }
        JustifyContent::SpaceEvenly => {
            let extra = remaining / (children.len() + 1) as f32;
            (content_rect.y + extra, extra)
        }
    };

    // Reverse 처리
    let iter: Box<dyn Iterator<Item = (&mut Widget, (f32, f32))>> =
        if matches!(parent_layout.flex_direction, FlexDirection::ColumnReverse) {
            Box::new(children.iter_mut().zip(sizes.into_iter()).rev())
        } else {
            Box::new(children.iter_mut().zip(sizes.into_iter()))
        };

    for (child, (w, h)) in iter {
        // 교차축 정렬 (align-items)
        let x = match parent_layout.align_items {
            AlignItems::Start => content_rect.x,
            AlignItems::End => content_rect.x + content_rect.width - w,
            AlignItems::Center => content_rect.x + (content_rect.width - w) / 2.0,
            AlignItems::Stretch => content_rect.x,
        };

        let width = if matches!(parent_layout.align_items, AlignItems::Stretch) {
            content_rect.width
        } else {
            w
        };

        child.computed_rect = Rect {
            x,
            y,
            width,
            height: h,
        };

        y += h + gap + extra_gap;
    }
}

/// 포인트가 위젯 내에 있는지 확인 (히트 테스트)
pub fn hit_test(widget: &Widget, x: f32, y: f32) -> Option<&Widget> {
    if !widget.visible || !widget.interactive {
        return None;
    }

    // 자식들 먼저 확인 (z-order: 나중에 그려진 것이 위에)
    for child in widget.children.iter().rev() {
        if let Some(hit) = hit_test(child, x, y) {
            return Some(hit);
        }
    }

    // 현재 위젯 확인
    if widget.computed_rect.contains(x, y) {
        Some(widget)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn test_anchor_positions() {
        assert_eq!(Anchor::TopLeft.to_normalized(), (0.0, 0.0));
        assert_eq!(Anchor::Center.to_normalized(), (0.5, 0.5));
        assert_eq!(Anchor::BottomRight.to_normalized(), (1.0, 1.0));
    }

    #[test]
    fn test_rect_contains() {
        let rect = Rect::new(10.0, 10.0, 100.0, 50.0);
        assert!(rect.contains(50.0, 30.0));
        assert!(!rect.contains(5.0, 5.0));
        assert!(!rect.contains(200.0, 30.0));
    }
}
