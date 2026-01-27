# SKOPE UI 개선 계획서

> 언리얼 Slate 레퍼런스 기반 구현 계획

---

## Phase 1: TAttribute 패턴 (데이터 바인딩)

### 목표
위젯 프로퍼티가 런타임에 동적으로 값을 가져올 수 있도록 함.

### 언리얼 패턴 분석
```cpp
// 언리얼 Slate
SLATE_ATTRIBUTE(ECheckBoxState, IsChecked)  // TAttribute<ECheckBoxState>

// 사용 예시
SNew(SCheckBox)
    .IsChecked(this, &MyClass::GetCheckState)  // 델리게이트 바인딩
    .IsChecked(ECheckBoxState::Checked)        // 직접 값
```

### SKOPE 구현 계획

#### 파일: `crates/skope_ui/src/core/attribute.rs`

```rust
/// 속성 값 (정적 또는 동적)
pub enum Attribute<T: Clone + Send + Sync + 'static> {
    /// 정적 값
    Static(T),
    /// 동적 바인딩 (클로저)
    Bound(Arc<dyn Fn() -> T + Send + Sync>),
}

impl<T: Clone + Send + Sync + 'static> Attribute<T> {
    /// 정적 값 생성
    pub fn from_value(value: T) -> Self {
        Self::Static(value)
    }

    /// 바인딩 생성
    pub fn bind<F>(getter: F) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        Self::Bound(Arc::new(getter))
    }

    /// 현재 값 가져오기
    pub fn get(&self) -> T {
        match self {
            Self::Static(v) => v.clone(),
            Self::Bound(f) => f(),
        }
    }

    /// 값이 바인딩되어 있는지
    pub fn is_bound(&self) -> bool {
        matches!(self, Self::Bound(_))
    }
}

// From 구현으로 편의성 제공
impl<T: Clone + Send + Sync + 'static> From<T> for Attribute<T> {
    fn from(value: T) -> Self {
        Self::Static(value)
    }
}
```

#### 위젯 적용 예시

```rust
// Before
pub struct SCheckBox {
    state: CheckBoxState,
    enabled: bool,
}

// After
pub struct SCheckBox {
    is_checked: Attribute<CheckBoxState>,
    is_enabled: Attribute<bool>,
}

// 사용법
SCheckBox::new()
    .is_checked(CheckBoxState::Checked)  // 정적
    .is_checked_bind(|| some_data.is_active)  // 동적
    .build()
```

---

## Phase 2: 메뉴/팝업 시스템

### 언리얼 패턴 분석

```
MenuStack (전역 스택)
    └── IMenu (메뉴 인터페이스)
        ├── FMenuInWindow (별도 윈도우)
        └── FMenuInPopup (오버레이)

SMenuAnchor (앵커 위젯)
    ├── Content (버튼 등)
    └── MenuContent (펼쳐지는 메뉴)
```

### SKOPE 구현 계획

#### 2.1 PopupLayer 시스템

**파일:** `crates/skope_ui/src/framework/popup.rs`

```rust
/// 팝업 레이어 (전역 관리)
pub struct PopupLayer {
    /// 활성 팝업들 (Z-order)
    popups: Vec<PopupEntry>,
    /// 다음 팝업 ID
    next_id: PopupId,
}

pub struct PopupEntry {
    pub id: PopupId,
    pub content: Box<dyn Widget>,
    pub position: Vec2,
    pub anchor_rect: SlateRect,
    pub placement: MenuPlacement,
    pub dismiss_on_click_outside: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MenuPlacement {
    BelowAnchor,
    AboveAnchor,
    RightOfAnchor,
    LeftOfAnchor,
    ComboBox,
    ContextMenu,
}

impl PopupLayer {
    pub fn push(&mut self, entry: PopupEntry) -> PopupId;
    pub fn dismiss(&mut self, id: PopupId);
    pub fn dismiss_all(&mut self);
    pub fn is_any_open(&self) -> bool;

    /// 클릭 위치가 팝업 외부인지 확인
    pub fn handle_click(&mut self, pos: Vec2) -> bool;
}
```

#### 2.2 SMenuAnchor

**파일:** `crates/skope_ui/src/widget/s_menu_anchor.rs`

```rust
pub struct SMenuAnchor {
    /// 앵커 콘텐츠 (버튼 등)
    content: Option<Box<dyn Widget>>,
    /// 메뉴 콘텐츠 생성 콜백
    on_get_menu_content: Option<Box<dyn Fn() -> Box<dyn Widget> + Send + Sync>>,
    /// 메뉴 배치
    placement: MenuPlacement,
    /// 열림 상태
    is_open: bool,
    /// 열림 상태 변경 콜백
    on_open_changed: Option<Box<dyn Fn(bool) + Send + Sync>>,
}

impl SMenuAnchor {
    pub fn set_is_open(&mut self, open: bool, popup_layer: &mut PopupLayer);
    pub fn toggle(&mut self, popup_layer: &mut PopupLayer);
}
```

#### 2.3 SMenu / SMenuItem

**파일:** `crates/skope_ui/src/widget/s_menu.rs`

```rust
/// 메뉴 아이템 종류
pub enum MenuItemType {
    /// 일반 버튼
    Button,
    /// 체크박스
    Check,
    /// 라디오
    Radio,
    /// 서브메뉴
    SubMenu,
    /// 구분선
    Separator,
}

pub struct MenuItem {
    pub label: String,
    pub icon: Option<IconId>,
    pub shortcut: Option<String>,
    pub item_type: MenuItemType,
    pub is_enabled: bool,
    pub is_checked: bool,
    pub on_execute: Option<Box<dyn Fn() + Send + Sync>>,
    pub sub_menu: Option<Vec<MenuItem>>,
}

pub struct SMenu {
    items: Vec<MenuItem>,
    style: MenuStyle,
    hovered_index: Option<usize>,
    open_submenu_index: Option<usize>,
}

pub struct MenuStyle {
    pub background_color: Color,
    pub border_color: Color,
    pub item_height: f32,
    pub item_padding: Margin,
    pub separator_color: Color,
    pub hover_color: Color,
    pub disabled_color: Color,
    pub shortcut_color: Color,
    pub icon_size: f32,
}
```

#### 2.4 컨텍스트 메뉴 헬퍼

```rust
/// 간편한 컨텍스트 메뉴 생성
pub struct MenuBuilder {
    items: Vec<MenuItem>,
}

impl MenuBuilder {
    pub fn new() -> Self;
    pub fn item(self, label: &str, action: impl Fn() + 'static) -> Self;
    pub fn check(self, label: &str, checked: bool, action: impl Fn(bool) + 'static) -> Self;
    pub fn separator(self) -> Self;
    pub fn submenu(self, label: &str, builder: MenuBuilder) -> Self;
    pub fn build(self) -> SMenu;
}

// 사용 예시
MenuBuilder::new()
    .item("Cut", || clipboard.cut())
    .item("Copy", || clipboard.copy())
    .item("Paste", || clipboard.paste())
    .separator()
    .submenu("Transform", MenuBuilder::new()
        .item("Reset", || transform.reset())
        .item("Snap to Grid", || transform.snap()))
    .build()
```

---

## Phase 3: 키보드 접근성 강화

### 언리얼 패턴 분석

```cpp
// NavigationConfig - 키 매핑
TMap<FKey, EUINavigation> KeyEventRules;
TMap<FKey, EUINavigationAction> KeyActionRules;

// 위젯 포커스
virtual bool SupportsKeyboardFocus() const;
virtual FReply OnKeyDown(const FGeometry&, const FKeyEvent&);
```

### SKOPE 구현 계획

#### 3.1 NavigationConfig

**파일:** `crates/skope_ui/src/framework/navigation.rs`

```rust
/// UI 탐색 설정
pub struct NavigationConfig {
    /// Tab 탐색 활성화
    pub tab_navigation: bool,
    /// 키보드 탐색 활성화
    pub key_navigation: bool,
    /// 아날로그 스틱 탐색 (게임패드)
    pub analog_navigation: bool,

    /// 키 → 방향 매핑
    pub key_direction_rules: HashMap<KeyCode, NavigationDirection>,
    /// 키 → 액션 매핑
    pub key_action_rules: HashMap<KeyCode, NavigationAction>,

    /// 아날로그 임계값
    pub analog_threshold: f32,
}

#[derive(Clone, Copy)]
pub enum NavigationAction {
    Accept,   // Enter/Space
    Cancel,   // Escape
    Delete,
}

impl Default for NavigationConfig {
    fn default() -> Self {
        let mut key_direction = HashMap::new();
        key_direction.insert(KeyCode::ArrowUp, NavigationDirection::Up);
        key_direction.insert(KeyCode::ArrowDown, NavigationDirection::Down);
        key_direction.insert(KeyCode::ArrowLeft, NavigationDirection::Left);
        key_direction.insert(KeyCode::ArrowRight, NavigationDirection::Right);

        let mut key_action = HashMap::new();
        key_action.insert(KeyCode::Enter, NavigationAction::Accept);
        key_action.insert(KeyCode::Space, NavigationAction::Accept);
        key_action.insert(KeyCode::Escape, NavigationAction::Cancel);

        Self {
            tab_navigation: true,
            key_navigation: true,
            analog_navigation: true,
            key_direction_rules: key_direction,
            key_action_rules: key_action,
            analog_threshold: 0.5,
        }
    }
}
```

#### 3.2 Widget 트레이트 확장

**파일:** `crates/skope_ui/src/widget/traits.rs` (수정)

```rust
pub trait Widget {
    // 기존 메서드들...

    /// 키보드 포커스 지원 여부
    fn supports_keyboard_focus(&self) -> bool {
        false
    }

    /// 키 다운 이벤트
    fn on_key_down(&mut self, _event: &KeyEvent) -> Reply {
        Reply::unhandled()
    }

    /// 키 업 이벤트
    fn on_key_up(&mut self, _event: &KeyEvent) -> Reply {
        Reply::unhandled()
    }

    /// 포커스 받았을 때
    fn on_focus_received(&mut self, _cause: FocusCause) {}

    /// 포커스 잃었을 때
    fn on_focus_lost(&mut self) {}
}
```

#### 3.3 입력 위젯 키보드 지원 추가

모든 입력 위젯에 키보드 지원 추가:

| 위젯 | 키보드 동작 |
|------|-----------|
| SCheckBox | Space/Enter → 토글 |
| SButton | Space/Enter → 클릭 |
| SSlider | ←/→ → 값 증감, Home/End → 최소/최대 |
| SSpinBox | ←/→ → 값 증감, Enter → 편집모드 |
| SComboBox | Space → 열기, ↑/↓ → 선택, Enter → 확정, Esc → 취소 |
| SEditableTextBox | 전체 텍스트 편집 키 |
| STreeView | ↑/↓ → 이동, ←/→ → 접기/펴기, Enter → 선택 |

---

## Phase 4: Brush 시스템 (이미지 기반 스타일링)

### 언리얼 패턴 분석

```cpp
struct FSlateBrush {
    ESlateBrushDrawType::Type DrawAs;     // Box, Border, Image, NoDrawType
    ESlateBrushTileType::Type Tiling;     // NoTile, Horizontal, Vertical, Both
    FMargin Margin;                        // 9-slice margin
    FSlateColor TintColor;
    FVector2D ImageSize;
    UObject* ResourceObject;               // 텍스처
};
```

### SKOPE 구현 계획

#### 파일: `crates/skope_ui/src/core/brush.rs`

```rust
/// 브러시 (이미지/컬러 스타일링)
#[derive(Clone)]
pub enum SlateBrush {
    /// 단색
    Color(Color),
    /// 이미지
    Image {
        texture_id: TextureId,
        size: Vec2,
        tint: Color,
        draw_type: BrushDrawType,
        margin: Margin,  // 9-slice용
        tiling: BrushTiling,
    },
    /// 둥근 사각형
    RoundedBox {
        color: Color,
        corner_radius: f32,
    },
    /// 그라데이션
    Gradient {
        start_color: Color,
        end_color: Color,
        orientation: Orientation,
    },
    /// 없음
    None,
}

#[derive(Clone, Copy, Default)]
pub enum BrushDrawType {
    #[default]
    Image,      // 그대로 그리기
    Box,        // 9-slice 박스
    Border,     // 테두리만
    RoundedBox, // 둥근 박스
}

#[derive(Clone, Copy, Default)]
pub enum BrushTiling {
    #[default]
    NoTile,
    Horizontal,
    Vertical,
    Both,
}

impl SlateBrush {
    pub fn color(color: Color) -> Self {
        Self::Color(color)
    }

    pub fn image(texture_id: TextureId) -> SlateBrushBuilder {
        SlateBrushBuilder::new(texture_id)
    }

    pub fn rounded(color: Color, radius: f32) -> Self {
        Self::RoundedBox { color, corner_radius: radius }
    }

    /// 렌더링용 정보 얻기
    pub fn get_draw_info(&self) -> BrushDrawInfo { ... }
}
```

#### 렌더러 확장

```rust
// DrawElementList 확장
impl DrawElementList {
    pub fn add_brush(
        &mut self,
        layer: u32,
        geometry: PaintGeometry,
        brush: &SlateBrush,
    );
}
```

---

## Phase 5: Tooltip 시스템

### 언리얼 패턴 분석

```cpp
// 위젯에 툴팁 설정
.ToolTip(SNew(SToolTip).Text(LOCTEXT("MyTooltip", "Tooltip text")))
.ToolTipText(LOCTEXT("Simple", "Simple tooltip"))
```

### SKOPE 구현 계획

#### 파일: `crates/skope_ui/src/framework/tooltip.rs`

```rust
/// 툴팁 관리자
pub struct TooltipManager {
    /// 현재 표시 중인 툴팁
    current_tooltip: Option<TooltipState>,
    /// 표시 대기 중
    pending_tooltip: Option<PendingTooltip>,
    /// 지연 시간 (기본 0.5초)
    show_delay: f32,
    /// 사라지기 전 지연
    hide_delay: f32,
}

struct TooltipState {
    content: Box<dyn Widget>,
    position: Vec2,
    anchor_widget: WidgetId,
}

struct PendingTooltip {
    content: TooltipContent,
    hover_start_time: f64,
    widget_id: WidgetId,
}

pub enum TooltipContent {
    Text(String),
    Widget(Box<dyn Widget>),
}

impl TooltipManager {
    /// 마우스가 위젯 위에 있을 때 호출
    pub fn on_widget_hover(&mut self, widget_id: WidgetId, tooltip: TooltipContent, current_time: f64);

    /// 마우스가 위젯을 떠났을 때
    pub fn on_widget_leave(&mut self, widget_id: WidgetId);

    /// 매 프레임 업데이트
    pub fn tick(&mut self, current_time: f64, cursor_pos: Vec2);

    /// 툴팁 렌더링
    pub fn paint(&self, draw_elements: &mut DrawElementList, layer: u32);
}
```

#### Widget 트레이트 확장

```rust
pub trait Widget {
    /// 툴팁 콘텐츠 (있으면)
    fn get_tooltip(&self) -> Option<TooltipContent> {
        None
    }
}
```

---

## Phase 6: 사운드 피드백

### 구현 계획

#### 파일: `crates/skope_ui/src/framework/sound.rs`

```rust
/// UI 사운드 관리
pub struct UISoundManager {
    /// 사운드 재생 콜백
    play_sound: Option<Box<dyn Fn(UISoundEvent) + Send + Sync>>,
}

#[derive(Clone, Copy)]
pub enum UISoundEvent {
    ButtonClick,
    ButtonHover,
    CheckOn,
    CheckOff,
    SliderTick,
    MenuOpen,
    MenuClose,
    TabSwitch,
    Error,
    Success,
}

impl UISoundManager {
    pub fn set_handler<F>(&mut self, handler: F)
    where
        F: Fn(UISoundEvent) + Send + Sync + 'static;

    pub fn play(&self, event: UISoundEvent);
}
```

---

## Phase 7: 애니메이션 시스템 개선

### 언리얼 패턴 분석

```cpp
// AnimatedAttributeManager - 전역 틱 관리
// TAttributeInterpolator - 다양한 보간 방식
//   - TEasingAttributeInterpolator (이징)
//   - TArriveAttributeInterpolator (도착)
//   - TVerletAttributeInterpolator (물리)
```

### SKOPE 구현 계획

#### 7.1 Interpolator 확장

**파일:** `crates/skope_ui/src/framework/animation.rs` (확장)

```rust
/// 보간기 타입
pub enum InterpolatorType {
    /// 이징 기반
    Easing(EasingSettings),
    /// 도착 기반 (부드러운 감속)
    Arrive(ArriveSettings),
    /// 물리 기반 (Verlet)
    Verlet(VerletSettings),
}

pub struct EasingSettings {
    pub easing: EasingFunction,
    pub duration: f32,
}

pub struct ArriveSettings {
    pub iterations: u32,
    pub strength: f32,
}

pub struct VerletSettings {
    pub blend: f32,
    pub strength: f32,
    pub damping: f32,
}

/// 범용 보간기
pub struct Interpolator<T: Interpolable> {
    interpolator_type: InterpolatorType,
    current_value: T,
    target_value: T,
    last_value: T,
    velocity: T,  // Verlet용
    elapsed: f32,
    is_playing: bool,
}

pub trait Interpolable: Clone + Default {
    fn lerp(a: &Self, b: &Self, t: f32) -> Self;
    fn distance(a: &Self, b: &Self) -> f32;
}
```

#### 7.2 AnimationManager (전역 틱)

```rust
/// 애니메이션 매니저 (전역 틱 관리)
pub struct AnimationManager {
    /// 활성 애니메이션들
    animations: Vec<Box<dyn AnimationTick>>,
}

pub trait AnimationTick: Send + Sync {
    fn tick(&mut self, delta_time: f32);
    fn is_complete(&self) -> bool;
}

impl AnimationManager {
    pub fn register<T: AnimationTick + 'static>(&mut self, animation: T);
    pub fn tick_all(&mut self, delta_time: f32);
}
```

---

## 구현 우선순위 및 의존성

```
Phase 1: TAttribute ──────────────────┐
                                      │
Phase 2: 메뉴/팝업 ←──────────────────┤
                                      │
Phase 3: 키보드 접근성 ←──────────────┤
                                      │
Phase 4: Brush 시스템 ────────────────┤
                                      │
Phase 5: Tooltip ←────────────────────┤
                                      │
Phase 6: 사운드 (독립적) ─────────────┘

Phase 7: 애니메이션 개선 (독립적) ────────
```

### 권장 구현 순서

| 순서 | Phase | 이유 |
|-----|-------|------|
| 1 | Phase 1 (TAttribute) | 다른 기능들의 기반 |
| 2 | Phase 3 (키보드) | 기존 위젯 개선, 즉시 효과 |
| 3 | Phase 2 (메뉴/팝업) | 에디터 필수 기능 |
| 4 | Phase 5 (Tooltip) | 사용성 향상 |
| 5 | Phase 4 (Brush) | 비주얼 품질 |
| 6 | Phase 7 (애니메이션) | 폴리시 |
| 7 | Phase 6 (사운드) | 마무리 |

---

## 파일 구조 요약

```
crates/skope_ui/src/
├── core/
│   ├── mod.rs
│   ├── attribute.rs      # [NEW] TAttribute 패턴
│   └── brush.rs          # [NEW] Brush 시스템
│
├── framework/
│   ├── mod.rs
│   ├── focus.rs          # [EXISTS]
│   ├── animation.rs      # [MODIFY] 보간기 확장
│   ├── navigation.rs     # [NEW] 키보드 탐색 설정
│   ├── popup.rs          # [NEW] 팝업 레이어
│   ├── tooltip.rs        # [NEW] 툴팁 관리
│   └── sound.rs          # [NEW] 사운드 관리
│
├── widget/
│   ├── s_menu_anchor.rs  # [NEW]
│   ├── s_menu.rs         # [NEW]
│   └── ... (기존 위젯들 키보드 지원 추가)
```

---

## 예상 결과

구현 완료 시 SKOPE UI 완성도:

```
┌──────────────────────────────────────────────────────┐
│  예상 SKOPE UI 완성도 vs Unreal Slate                 │
├──────────────────────────────────────────────────────┤
│  코어 아키텍처     ████████████████████████  95%     │
│  기본 위젯        ████████████████████████  90%     │
│  입력 위젯        ████████████████████░░░░  85%     │
│  레이아웃 위젯    ████████████████████░░░░  85%     │
│  도킹 시스템      ████████████████████░░░░  85%     │
│  포커스/키보드    ████████████████████████  90%     │
│  애니메이션       ████████████████████░░░░  80%     │
│  메뉴/팝업        ████████████████████░░░░  85%     │
│  스타일링         ████████████████░░░░░░░░  75%     │
├──────────────────────────────────────────────────────┤
│  전체 평균                                   ~85%    │
└──────────────────────────────────────────────────────┘
```

---

## 참고 레퍼런스 파일

| 기능 | 레퍼런스 파일 |
|------|-------------|
| TAttribute | `SlateCore/Public/Misc/Attribute.h` |
| 메뉴 앵커 | `Slate/Public/Widgets/Input/SMenuAnchor.h` |
| 메뉴 스택 | `Slate/Public/Framework/Application/MenuStack.h` |
| 탐색 설정 | `Slate/Public/Framework/Application/NavigationConfig.h` |
| 브러시 | `SlateCore/Public/Styling/SlateBrush.h` |
| 보간기 | `Slate/Public/Framework/Animation/AttributeInterpolator.h` |
| 애니메이션 속성 | `Slate/Public/Framework/Animation/AnimatedAttribute.h` |
