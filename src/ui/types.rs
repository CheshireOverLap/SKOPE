// SKOPE UI - Core Types
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use super::style::Style;

/// 위젯 - UI의 기본 단위
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Widget {
    /// 고유 ID (선택적)
    #[serde(default)]
    pub id: Option<String>,

    /// 위젯 타입
    #[serde(default)]
    pub widget_type: WidgetType,

    /// 레이아웃 설정
    #[serde(default)]
    pub layout: Layout,

    /// 스타일
    #[serde(default)]
    pub style: Style,

    /// 자식 위젯들
    #[serde(default)]
    pub children: Vec<Widget>,

    /// 상태별 스타일
    #[serde(default)]
    pub states: HashMap<String, Style>,

    /// 현재 상태
    #[serde(default)]
    pub current_state: String,

    /// 전환 애니메이션
    #[serde(default)]
    pub transitions: HashMap<String, Transition>,

    /// 이벤트 핸들러
    #[serde(default)]
    pub events: HashMap<String, String>,

    /// 계산된 레이아웃 결과 (런타임)
    #[serde(skip)]
    pub computed_rect: Rect,

    /// 스크롤 오프셋 (런타임, ScrollView용)
    #[serde(skip)]
    pub scroll_offset: (f32, f32),

    /// 컨텐츠 크기 (런타임, ScrollView용)
    #[serde(skip)]
    pub content_size: (f32, f32),

    /// 입력 필드 커서 위치 (런타임, InputField용)
    #[serde(skip)]
    pub input_cursor_pos: usize,

    /// 입력 필드 선택 범위 (런타임, InputField용) - (start, end)
    #[serde(skip)]
    pub input_selection: Option<(usize, usize)>,

    /// 입력 필드 포커스 상태 (런타임, InputField용)
    #[serde(skip)]
    pub input_focused: bool,

    /// 입력 필드 커서 깜빡임 타이머 (런타임, InputField용)
    #[serde(skip)]
    pub input_cursor_blink: f32,

    /// 가시성
    #[serde(default = "default_true")]
    pub visible: bool,

    /// 상호작용 가능 여부
    #[serde(default = "default_true")]
    pub interactive: bool,

    /// 드래그 가능 여부
    #[serde(default)]
    pub draggable: bool,

    /// 드롭 대상 여부 (드래그된 아이템을 받을 수 있음)
    #[serde(default)]
    pub drop_target: bool,

    /// 드래그 데이터 (드래그 시 전달할 데이터)
    #[serde(default)]
    pub drag_data: Option<String>,

    /// 드롭 그룹 (같은 그룹끼리만 드롭 가능, None이면 모두 허용)
    #[serde(default)]
    pub drag_group: Option<String>,

    /// 툴팁 텍스트 (마우스 오버 시 표시)
    #[serde(default)]
    pub tooltip: Option<String>,

    /// 툴팁 표시 지연 시간 (초, 기본 0.5초)
    #[serde(default)]
    pub tooltip_delay: Option<f32>,
}

fn default_true() -> bool {
    true
}

impl Default for Widget {
    fn default() -> Self {
        Self {
            id: None,
            widget_type: WidgetType::Container,
            layout: Layout::default(),
            style: Style::default(),
            children: Vec::new(),
            states: HashMap::new(),
            current_state: "default".to_string(),
            transitions: HashMap::new(),
            events: HashMap::new(),
            computed_rect: Rect::default(),
            scroll_offset: (0.0, 0.0),
            content_size: (0.0, 0.0),
            input_cursor_pos: 0,
            input_selection: None,
            input_focused: false,
            input_cursor_blink: 0.0,
            visible: true,
            interactive: true,
            draggable: false,
            drop_target: false,
            drag_data: None,
            drag_group: None,
            tooltip: None,
            tooltip_delay: None,
        }
    }
}

/// 위젯 타입
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum WidgetType {
    /// 컨테이너 (자식만 포함)
    #[default]
    Container,

    /// 텍스트
    Text {
        #[serde(default)]
        content: String,
        #[serde(default)]
        font: Option<String>,
        #[serde(default)]
        font_size: Option<f32>,
    },

    /// 이미지
    Image {
        src: String,
        #[serde(default)]
        color: Option<Color>,
        #[serde(default)]
        preserve_aspect: bool,
    },

    /// 9-슬라이스 이미지
    NineSlice {
        src: String,
        border: Border,
    },

    /// 버튼
    Button {
        #[serde(default)]
        text: Option<String>,
        #[serde(default)]
        states: ButtonStates,
    },

    /// 프로그레스 바
    ProgressBar {
        #[serde(default)]
        value: f32,
        #[serde(default)]
        max_value: f32,
        #[serde(default)]
        fill_image: Option<String>,
        #[serde(default)]
        background_image: Option<String>,
    },

    /// 스크롤 뷰
    ScrollView {
        #[serde(default)]
        scroll_x: bool,
        #[serde(default)]
        scroll_y: bool,
    },

    /// 입력 필드
    InputField {
        #[serde(default)]
        placeholder: String,
        #[serde(default)]
        value: String,
        #[serde(default)]
        max_length: Option<usize>,
    },

    /// 슬라이더
    Slider {
        #[serde(default)]
        value: f32,
        #[serde(default)]
        min_value: f32,
        #[serde(default = "default_max")]
        max_value: f32,
    },

    /// 토글/체크박스
    Toggle {
        #[serde(default)]
        checked: bool,
    },

    /// 스프라이트 시트에서 특정 프레임
    Sprite {
        src: String,
        #[serde(default)]
        frame: usize,
        #[serde(default)]
        grid: Option<(usize, usize)>,
    },
}

fn default_max() -> f32 {
    1.0
}

/// 레이아웃 설정
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Layout {
    /// 앵커 포인트
    #[serde(default)]
    pub anchor: Anchor,

    /// 앵커로부터의 오프셋
    #[serde(default)]
    pub offset: (f32, f32),

    /// 크기 설정
    #[serde(default)]
    pub size: Size,

    /// 피벗 (0.0 = 왼쪽/위, 0.5 = 중앙, 1.0 = 오른쪽/아래)
    #[serde(default = "default_pivot")]
    pub pivot: (f32, f32),

    /// 패딩
    #[serde(default)]
    pub padding: Edges,

    /// 마진
    #[serde(default)]
    pub margin: Edges,

    /// Flexbox 방향
    #[serde(default)]
    pub flex_direction: FlexDirection,

    /// Flexbox 정렬
    #[serde(default)]
    pub justify_content: JustifyContent,

    /// Flexbox 교차축 정렬
    #[serde(default)]
    pub align_items: AlignItems,

    /// Flexbox 간격
    #[serde(default)]
    pub gap: f32,

    /// Z-인덱스 (렌더링 순서)
    #[serde(default)]
    pub z_index: i32,
}

fn default_pivot() -> (f32, f32) {
    (0.0, 0.0)
}

/// 앵커 포인트
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum Anchor {
    #[default]
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    Center,
    MiddleRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
    /// 스트레치 (부모에 맞춤)
    Stretch,
    /// 커스텀 앵커 (0.0-1.0)
    Custom(f32, f32),
}

impl Anchor {
    pub fn to_normalized(&self) -> (f32, f32) {
        match self {
            Anchor::TopLeft => (0.0, 0.0),
            Anchor::TopCenter => (0.5, 0.0),
            Anchor::TopRight => (1.0, 0.0),
            Anchor::MiddleLeft => (0.0, 0.5),
            Anchor::Center => (0.5, 0.5),
            Anchor::MiddleRight => (1.0, 0.5),
            Anchor::BottomLeft => (0.0, 1.0),
            Anchor::BottomCenter => (0.5, 1.0),
            Anchor::BottomRight => (1.0, 1.0),
            Anchor::Stretch => (0.5, 0.5),
            Anchor::Custom(x, y) => (*x, *y),
        }
    }
}

/// 크기 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Size {
    /// 고정 픽셀 크기
    Fixed(f32, f32),
    /// 퍼센트 크기 (부모 기준)
    Percent(f32, f32),
    /// 자식 내용에 맞춤
    FitContent,
    /// 부모에 맞춤 (stretch)
    Fill,
    /// 가로만 고정, 세로 자동
    WidthFixed(f32),
    /// 세로만 고정, 가로 자동
    HeightFixed(f32),
}

impl Default for Size {
    fn default() -> Self {
        Size::FitContent
    }
}

/// 가장자리 값 (패딩, 마진, 보더)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct Edges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Edges {
    pub fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    pub fn symmetric(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }
}

/// 사각형 영역
#[derive(Debug, Clone, Copy, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.x + self.width &&
        y >= self.y && y <= self.y + self.height
    }

    pub fn center(&self) -> (f32, f32) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }
}

/// 보더 (9-슬라이스용)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct Border {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

/// 버튼 상태별 이미지
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ButtonStates {
    pub normal: Option<String>,
    pub hover: Option<String>,
    pub pressed: Option<String>,
    pub disabled: Option<String>,
}

/// Flexbox 방향
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum FlexDirection {
    #[default]
    Row,
    Column,
    RowReverse,
    ColumnReverse,
}

/// Flexbox 주축 정렬
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum JustifyContent {
    #[default]
    Start,
    End,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// Flexbox 교차축 정렬
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum AlignItems {
    #[default]
    Start,
    End,
    Center,
    Stretch,
}

/// 색상
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Color {
    /// RGBA (0.0-1.0)
    Rgba(f32, f32, f32, f32),
    /// Hex 문자열 (#RRGGBB 또는 #RRGGBBAA)
    Hex(u32),
    /// 그라데이션
    Gradient {
        from: u32,
        to: u32,
        direction: GradientDirection,
    },
}

impl Default for Color {
    fn default() -> Self {
        Color::Rgba(1.0, 1.0, 1.0, 1.0)
    }
}

impl Color {
    pub fn from_hex(hex: &str) -> Self {
        let hex = hex.trim_start_matches('#');
        let value = u32::from_str_radix(hex, 16).unwrap_or(0xFFFFFF);
        Color::Hex(value)
    }

    pub fn to_rgba(&self) -> [f32; 4] {
        match self {
            Color::Rgba(r, g, b, a) => [*r, *g, *b, *a],
            Color::Hex(hex) => {
                let r = ((hex >> 16) & 0xFF) as f32 / 255.0;
                let g = ((hex >> 8) & 0xFF) as f32 / 255.0;
                let b = (hex & 0xFF) as f32 / 255.0;
                [r, g, b, 1.0]
            }
            Color::Gradient { from, .. } => {
                // 시작 색상 반환 (실제로는 셰이더에서 처리)
                let r = ((from >> 16) & 0xFF) as f32 / 255.0;
                let g = ((from >> 8) & 0xFF) as f32 / 255.0;
                let b = (from & 0xFF) as f32 / 255.0;
                [r, g, b, 1.0]
            }
        }
    }

    pub const WHITE: Color = Color::Rgba(1.0, 1.0, 1.0, 1.0);
    pub const BLACK: Color = Color::Rgba(0.0, 0.0, 0.0, 1.0);
    pub const RED: Color = Color::Rgba(1.0, 0.0, 0.0, 1.0);
    pub const GREEN: Color = Color::Rgba(0.0, 1.0, 0.0, 1.0);
    pub const BLUE: Color = Color::Rgba(0.0, 0.0, 1.0, 1.0);
    pub const TRANSPARENT: Color = Color::Rgba(0.0, 0.0, 0.0, 0.0);
}

/// 그라데이션 방향
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum GradientDirection {
    #[default]
    Horizontal,
    Vertical,
    Diagonal,
}

/// 전환 애니메이션 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transition {
    pub duration: f32,
    #[serde(default)]
    pub easing: Easing,
    #[serde(default)]
    pub delay: f32,
    #[serde(default)]
    pub properties: Vec<AnimatableProperty>,
}

/// 이징 함수
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum Easing {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    EaseInQuad,
    EaseOutQuad,
    EaseInOutQuad,
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,
    EaseOutBack,
    EaseOutBounce,
    EaseOutElastic,
    /// 커스텀 베지어 커브
    Bezier(f32, f32, f32, f32),
}

impl Easing {
    pub fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::EaseIn => t * t,
            Easing::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Easing::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
            Easing::EaseInQuad => t * t,
            Easing::EaseOutQuad => 1.0 - (1.0 - t).powi(2),
            Easing::EaseInOutQuad => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
            Easing::EaseInCubic => t * t * t,
            Easing::EaseOutCubic => 1.0 - (1.0 - t).powi(3),
            Easing::EaseInOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Easing::EaseOutBack => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
            }
            Easing::EaseOutBounce => {
                let n1 = 7.5625;
                let d1 = 2.75;
                if t < 1.0 / d1 {
                    n1 * t * t
                } else if t < 2.0 / d1 {
                    let t = t - 1.5 / d1;
                    n1 * t * t + 0.75
                } else if t < 2.5 / d1 {
                    let t = t - 2.25 / d1;
                    n1 * t * t + 0.9375
                } else {
                    let t = t - 2.625 / d1;
                    n1 * t * t + 0.984375
                }
            }
            Easing::EaseOutElastic => {
                if t == 0.0 || t == 1.0 {
                    t
                } else {
                    let c4 = (2.0 * std::f32::consts::PI) / 3.0;
                    2.0_f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0
                }
            }
            Easing::Bezier(x1, y1, x2, y2) => {
                // 간단한 큐빅 베지어 근사
                cubic_bezier(t, *x1, *y1, *x2, *y2)
            }
        }
    }
}

fn cubic_bezier(t: f32, _x1: f32, y1: f32, _x2: f32, y2: f32) -> f32 {
    // 간단한 근사 (실제로는 뉴턴-랩슨 필요)
    let t2 = t * t;
    let t3 = t2 * t;
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    let _mt3 = mt2 * mt;

    3.0 * mt2 * t * y1 + 3.0 * mt * t2 * y2 + t3
}

/// 애니메이션 가능한 속성
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnimatableProperty {
    Opacity,
    Offset,
    Scale,
    Rotation,
    Color,
    Width,
    Height,
    All,
}

/// UI 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// 기준 해상도
    pub reference_resolution: (f32, f32),
    /// 스케일링 모드
    pub scale_mode: ScaleMode,
    /// 최소 스케일
    pub min_scale: f32,
    /// 최대 스케일
    pub max_scale: f32,
    /// Safe area 적용
    pub respect_safe_area: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            reference_resolution: (1920.0, 1080.0),
            scale_mode: ScaleMode::ScaleWithWidth,
            min_scale: 0.5,
            max_scale: 2.0,
            respect_safe_area: true,
        }
    }
}

/// 스케일링 모드
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum ScaleMode {
    #[default]
    ScaleWithWidth,
    ScaleWithHeight,
    ScaleWithMin,
    ScaleWithMax,
    FixedPixel,
}
