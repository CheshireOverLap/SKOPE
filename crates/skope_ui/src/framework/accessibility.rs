//! Accessibility — 접근성 시스템
//!
//! UE의 접근성 프레임워크 참고. 위젯별 역할/상태/이름 제공.
//! winit의 AccessibilityRequest 이벤트와 연동 가능.

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

/// 접근성 이벤트 (스크린 리더에 전달)
#[derive(Debug, Clone)]
pub enum AccessibilityEvent {
    /// 포커스 변경
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
    /// 공지 (알림 읽기)
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
        self.pending_events.push(AccessibilityEvent::StateChanged {
            widget_name: widget_name.into(),
            state_desc: state_desc.into(),
        });
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
