//! Accessibility — 접근성 시스템
//!
//! UE5.7 SlateCore/Slate 접근성 프레임워크 매칭.
//! - `AccessibleBehavior`: 위젯별 접근성 트리 참여 방식 (EAccessibleBehavior)
//! - `AccessibleWidgetData`: 위젯별 접근성 구성 (FAccessibleWidgetData)
//! - `IAccessibleActivatable`/`IAccessibleProperty`/`IAccessibleText`: 역할별 인터페이스
//! - `AccessibilityProvider`: 이벤트 큐 + 플랫폼 브릿지 기초
//!
//! winit의 AccessibilityRequest 이벤트와 연동 가능.

// ============================================================================
// AccessibleBehavior — UE5.7 EAccessibleBehavior
// ============================================================================

/// 위젯의 접근성 트리 참여 방식 — UE5.7 EAccessibleBehavior
///
/// 위젯이 스크린 리더에 노출되는 방식을 제어합니다.
/// 각 위젯은 Main(직접 쿼리) / Summary(부모 요약) 두 모드에 대해
/// 독립적으로 Behavior를 설정할 수 있습니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AccessibleBehavior {
    /// 접근성 트리에 포함되지 않음
    #[default]
    NotAccessible,
    /// 위젯 콘텐츠에서 자동 감지
    Auto,
    /// 자식 위젯들의 텍스트를 연결하여 요약
    Summary,
    /// 수동 지정 텍스트 사용 (SetAccessibleText로 설정)
    Custom,
    /// 위젯의 툴팁을 접근성 텍스트로 사용
    ToolTip,
}

/// 접근성 쿼리 타입 — UE5.7 EAccessibleType
///
/// `Main`: 스크린 리더가 위젯을 직접 포커스할 때
/// `Summary`: 부모 위젯이 자식들을 요약할 때
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessibleType {
    /// 직접 접근성 쿼리 (위젯 포커스 시)
    Main,
    /// 부모 요약 쿼리 (Summary behavior에서 자식 수집 시)
    Summary,
}

// ============================================================================
// AccessibleWidgetData — UE5.7 FAccessibleWidgetData
// ============================================================================

/// 위젯별 접근성 구성 데이터 — UE5.7 FAccessibleWidgetData
///
/// 각 위젯이 소유하며, 접근성 트리 참여 방식과 텍스트를 정의합니다.
/// `SWidget::SetAccessibleBehavior()`로 설정.
#[derive(Debug, Clone)]
pub struct AccessibleWidgetData {
    /// 자식 위젯들이 접근성 트리에 포함될 수 있는지
    pub can_children_be_accessible: bool,
    /// 직접 쿼리 시 behavior
    pub accessible_behavior: AccessibleBehavior,
    /// 부모 요약 시 behavior
    pub accessible_summary_behavior: AccessibleBehavior,
    /// 커스텀 접근성 텍스트 (Custom behavior용)
    pub accessible_text: Option<String>,
    /// 커스텀 요약 텍스트 (Custom summary behavior용)
    pub accessible_summary_text: Option<String>,
}

impl Default for AccessibleWidgetData {
    fn default() -> Self {
        Self {
            can_children_be_accessible: true,
            accessible_behavior: AccessibleBehavior::NotAccessible,
            accessible_summary_behavior: AccessibleBehavior::NotAccessible,
            accessible_text: None,
            accessible_summary_text: None,
        }
    }
}

impl AccessibleWidgetData {
    /// behavior 설정 — UE5.7 SWidget::SetAccessibleBehavior
    pub fn set_behavior(&mut self, behavior: AccessibleBehavior, text: Option<String>, accessible_type: AccessibleType) {
        match accessible_type {
            AccessibleType::Main => {
                self.accessible_behavior = behavior;
                self.accessible_text = text;
            }
            AccessibleType::Summary => {
                self.accessible_summary_behavior = behavior;
                self.accessible_summary_text = text;
            }
        }
    }

    /// 접근성 트리에 포함되는지 (Main behavior가 NotAccessible이 아닌 경우)
    pub fn is_accessible(&self) -> bool {
        self.accessible_behavior != AccessibleBehavior::NotAccessible
    }

    /// 지정된 타입의 behavior 반환
    pub fn get_behavior(&self, accessible_type: AccessibleType) -> AccessibleBehavior {
        match accessible_type {
            AccessibleType::Main => self.accessible_behavior,
            AccessibleType::Summary => self.accessible_summary_behavior,
        }
    }

    /// 지정된 타입의 커스텀 텍스트 반환
    pub fn get_text(&self, accessible_type: AccessibleType) -> Option<&str> {
        match accessible_type {
            AccessibleType::Main => self.accessible_text.as_deref(),
            AccessibleType::Summary => self.accessible_summary_text.as_deref(),
        }
    }
}

// ============================================================================
// IAccessibleActivatable — UE5.7 IAccessibleActivatable
// ============================================================================

/// 활성화 가능 위젯 인터페이스 — UE5.7 IAccessibleActivatable
///
/// 버튼, 체크박스, 하이퍼링크 등 클릭/토글 가능한 위젯이 구현합니다.
pub trait IAccessibleActivatable {
    /// 위젯 활성화 (클릭/누르기)
    fn activate(&mut self);

    /// 토글 가능한 위젯인지 (체크박스, 라디오 버튼)
    fn is_checkable(&self) -> bool { false }

    /// 현재 체크 상태 — UE5.7 GetCheckedState
    ///
    /// None = 미체크, Some(true) = 체크, Some(false) = 불확정
    fn get_checked_state(&self) -> Option<bool> { None }
}

// ============================================================================
// IAccessibleProperty — UE5.7 IAccessibleProperty
// ============================================================================

/// 값 보유 위젯 인터페이스 — UE5.7 IAccessibleProperty
///
/// 슬라이더, 텍스트 입력, 스핀박스 등 값을 가진 위젯이 구현합니다.
pub trait IAccessibleProperty {
    /// 현재 값 (문자열)
    fn get_value(&self) -> String;

    /// 값 설정 (스크린 리더에서 수정 시)
    fn set_value(&mut self, _value: &str) {}

    /// 읽기 전용 여부
    fn is_read_only(&self) -> bool { false }

    /// 비밀번호 필드 여부
    fn is_password(&self) -> bool { false }

    /// 슬라이더 스텝 크기 (0이면 연속)
    fn get_step_size(&self) -> f32 { 0.0 }

    /// 최소값 (슬라이더/스핀박스)
    fn get_minimum(&self) -> f32 { 0.0 }

    /// 최대값 (슬라이더/스핀박스)
    fn get_maximum(&self) -> f32 { 1.0 }
}

// ============================================================================
// IAccessibleText — UE5.7 IAccessibleText
// ============================================================================

/// 텍스트 표시 위젯 인터페이스 — UE5.7 IAccessibleText
///
/// 텍스트 블록, 레이블 등 텍스트를 표시하는 위젯이 구현합니다.
pub trait IAccessibleText {
    /// 전체 텍스트 내용 반환
    fn get_text(&self) -> &str;
}

// ============================================================================
// AccessibilityRole (기존)
// ============================================================================

/// 접근성 역할 (WAI-ARIA role 매핑)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessibilityRole {
    None,
    Button,
    CheckBox,
    RadioButton,
    Slider,
    TextInput,
    Label,
    List,
    ListItem,
    Tree,
    TreeItem,
    Tab,
    TabPanel,
    Menu,
    MenuItem,
    Dialog,
    ProgressBar,
    ScrollBar,
    Separator,
    Image,
    Window,
    Panel,
    ToolTip,
    ComboBox,
    SearchBox,
    SpinButton,
    Link,
}

impl Default for AccessibilityRole {
    fn default() -> Self {
        Self::None
    }
}

impl AccessibilityRole {
    /// ARIA 역할 문자열
    pub fn aria_role(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Button => "button",
            Self::CheckBox => "checkbox",
            Self::RadioButton => "radio",
            Self::Slider => "slider",
            Self::TextInput => "textbox",
            Self::Label => "label",
            Self::List => "list",
            Self::ListItem => "listitem",
            Self::Tree => "tree",
            Self::TreeItem => "treeitem",
            Self::Tab => "tab",
            Self::TabPanel => "tabpanel",
            Self::Menu => "menu",
            Self::MenuItem => "menuitem",
            Self::Dialog => "dialog",
            Self::ProgressBar => "progressbar",
            Self::ScrollBar => "scrollbar",
            Self::Separator => "separator",
            Self::Image => "img",
            Self::Window => "window",
            Self::Panel => "group",
            Self::ToolTip => "tooltip",
            Self::ComboBox => "combobox",
            Self::SearchBox => "searchbox",
            Self::SpinButton => "spinbutton",
            Self::Link => "link",
        }
    }
}

/// 접근성 상태
#[derive(Debug, Clone, Default)]
pub struct AccessibilityState {
    pub focusable: bool,
    pub focused: bool,
    pub enabled: bool,
    /// CheckBox/RadioButton
    pub checked: Option<bool>,
    /// Tree/Expandable
    pub expanded: Option<bool>,
    pub selected: bool,
    /// Slider/ProgressBar 값
    pub value_now: Option<f32>,
    pub value_min: Option<f32>,
    pub value_max: Option<f32>,
    pub value_text: Option<String>,
    /// 읽기 전용
    pub read_only: bool,
    /// 필수 입력
    pub required: bool,
}

/// 접근성 정보
#[derive(Debug, Clone)]
pub struct AccessibilityInfo {
    pub role: AccessibilityRole,
    pub name: String,
    pub description: Option<String>,
    pub state: AccessibilityState,
}

impl Default for AccessibilityInfo {
    fn default() -> Self {
        Self {
            role: AccessibilityRole::None,
            name: String::new(),
            description: None,
            state: AccessibilityState::default(),
        }
    }
}

/// 접근성 이벤트 — UE5.7 EAccessibleEvent 확장
///
/// 스크린 리더/플랫폼 접근성 API에 전달됩니다.
#[derive(Debug, Clone)]
pub enum AccessibilityEvent {
    /// 포커스 변경 — UE5.7 EAccessibleEvent::FocusChange
    FocusChanged {
        widget_name: String,
        role: AccessibilityRole,
    },
    /// 값 변경
    ValueChanged {
        widget_name: String,
        value: String,
    },
    /// 상태 변경
    StateChanged {
        widget_name: String,
        state_desc: String,
    },
    /// 위젯 활성화 (클릭/토글) — UE5.7 EAccessibleEvent::Activate
    Activate {
        widget_name: String,
    },
    /// 위젯 제거됨 — UE5.7 EAccessibleEvent::WidgetRemoved
    WidgetRemoved {
        widget_id: u64,
    },
    /// 부모 변경됨 — UE5.7 EAccessibleEvent::ParentChanged
    ParentChanged {
        widget_name: String,
    },
    /// 공지 (알림 읽기) — UE5.7 EAccessibleEvent::Notification
    Announcement {
        message: String,
        priority: AnnouncePriority,
    },
}

/// 공지 우선순위
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnouncePriority {
    /// 현재 읽기 완료 후 읽음
    Polite,
    /// 즉시 읽음 (인터럽트)
    Assertive,
}

/// 접근성 제공자
#[allow(dead_code)]
pub struct AccessibilityProvider {
    /// 활성 여부
    pub enabled: bool,
    /// 대기 중인 이벤트 큐
    pending_events: Vec<AccessibilityEvent>,
    /// 로그 출력 여부 (디버그용)
    pub log_events: bool,
}

impl Default for AccessibilityProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl AccessibilityProvider {
    pub fn new() -> Self {
        Self {
            enabled: false,
            pending_events: Vec::new(),
            log_events: false,
        }
    }

    /// 활성화
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if enabled {
            log::info!("[Accessibility] Provider enabled");
        }
    }

    /// 공지 큐잉
    pub fn announce(&mut self, message: impl Into<String>, priority: AnnouncePriority) {
        if !self.enabled {
            return;
        }
        let msg = message.into();
        if self.log_events {
            log::info!("[Accessibility] Announce ({:?}): {}", priority, msg);
        }
        self.pending_events.push(AccessibilityEvent::Announcement {
            message: msg,
            priority,
        });
    }

    /// 포커스 변경 알림
    pub fn notify_focus_changed(&mut self, info: &AccessibilityInfo) {
        if !self.enabled {
            return;
        }
        if self.log_events {
            log::info!(
                "[Accessibility] Focus → {} ({})",
                info.name, info.role.aria_role()
            );
        }
        self.pending_events.push(AccessibilityEvent::FocusChanged {
            widget_name: info.name.clone(),
            role: info.role,
        });
    }

    /// 값 변경 알림
    pub fn notify_value_changed(&mut self, widget_name: impl Into<String>, value: impl Into<String>) {
        if !self.enabled {
            return;
        }
        let name = widget_name.into();
        let val = value.into();
        if self.log_events {
            log::info!("[Accessibility] Value changed: {} = {}", name, val);
        }
        self.pending_events.push(AccessibilityEvent::ValueChanged {
            widget_name: name,
            value: val,
        });
    }

    /// 상태 변경 알림
    pub fn notify_state_changed(&mut self, widget_name: impl Into<String>, state_desc: impl Into<String>) {
        if !self.enabled {
            return;
        }
        let name = widget_name.into();
        let desc = state_desc.into();
        if self.log_events {
            log::info!("[Accessibility] State changed: {} → {}", name, desc);
        }
        self.pending_events.push(AccessibilityEvent::StateChanged {
            widget_name: name,
            state_desc: desc,
        });
    }

    /// 부모 변경 알림 — UE5.7 EAccessibleEvent::ParentChanged
    pub fn notify_parent_changed(&mut self, widget_name: impl Into<String>) {
        if !self.enabled {
            return;
        }
        let name = widget_name.into();
        if self.log_events {
            log::info!("[Accessibility] Parent changed: {}", name);
        }
        self.pending_events.push(AccessibilityEvent::ParentChanged {
            widget_name: name,
        });
    }

    /// 위젯 활성화 알림 — UE5.7 EAccessibleEvent::Activate
    pub fn notify_activate(&mut self, widget_name: impl Into<String>) {
        if !self.enabled {
            return;
        }
        let name = widget_name.into();
        if self.log_events {
            log::info!("[Accessibility] Activate: {}", name);
        }
        self.pending_events.push(AccessibilityEvent::Activate {
            widget_name: name,
        });
    }

    /// 위젯 제거 알림 — UE5.7 EAccessibleEvent::WidgetRemoved
    pub fn notify_widget_removed(&mut self, widget_id: u64) {
        if !self.enabled {
            return;
        }
        self.pending_events.push(AccessibilityEvent::WidgetRemoved { widget_id });
    }

    /// 대기 중인 이벤트 소비
    pub fn drain_events(&mut self) -> Vec<AccessibilityEvent> {
        std::mem::take(&mut self.pending_events)
    }

    /// 대기 중인 이벤트 수
    pub fn pending_count(&self) -> usize {
        self.pending_events.len()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_accessible_behavior_default() {
        let data = AccessibleWidgetData::default();
        assert_eq!(data.accessible_behavior, AccessibleBehavior::NotAccessible);
        assert_eq!(data.accessible_summary_behavior, AccessibleBehavior::NotAccessible);
        assert!(!data.is_accessible());
        assert!(data.can_children_be_accessible);
    }

    #[test]
    fn test_accessible_widget_data_set_behavior() {
        let mut data = AccessibleWidgetData::default();

        // Main behavior
        data.set_behavior(AccessibleBehavior::Auto, None, AccessibleType::Main);
        assert_eq!(data.get_behavior(AccessibleType::Main), AccessibleBehavior::Auto);
        assert!(data.is_accessible());

        // Summary behavior with custom text
        data.set_behavior(
            AccessibleBehavior::Custom,
            Some("Submit button".to_string()),
            AccessibleType::Summary,
        );
        assert_eq!(data.get_behavior(AccessibleType::Summary), AccessibleBehavior::Custom);
        assert_eq!(data.get_text(AccessibleType::Summary), Some("Submit button"));
    }

    #[test]
    fn test_provider_activate_event() {
        let mut provider = AccessibilityProvider::new();
        provider.set_enabled(true);

        provider.notify_activate("OK Button");
        assert_eq!(provider.pending_count(), 1);

        let events = provider.drain_events();
        assert!(matches!(&events[0], AccessibilityEvent::Activate { widget_name } if widget_name == "OK Button"));
    }

    #[test]
    fn test_provider_widget_removed_event() {
        let mut provider = AccessibilityProvider::new();
        provider.set_enabled(true);

        provider.notify_widget_removed(42);
        let events = provider.drain_events();
        assert!(matches!(&events[0], AccessibilityEvent::WidgetRemoved { widget_id: 42 }));
    }

    #[test]
    fn test_provider_disabled_skips_events() {
        let mut provider = AccessibilityProvider::new();
        // enabled is false by default

        provider.notify_activate("Button");
        provider.notify_widget_removed(1);
        provider.notify_focus_changed(&AccessibilityInfo::default());
        assert_eq!(provider.pending_count(), 0);
    }

    #[test]
    fn test_accessibility_role_aria() {
        assert_eq!(AccessibilityRole::Button.aria_role(), "button");
        assert_eq!(AccessibilityRole::Slider.aria_role(), "slider");
        assert_eq!(AccessibilityRole::None.aria_role(), "none");
    }
}
