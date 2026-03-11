//! 도킹 레이아웃 직렬화
//!
//! 언리얼 FTabManager::FLayout 참고
//! - ToJson/NewFromJson으로 JSON 직렬화
//! - Type, SizeCoefficient, Orientation, Tabs, Nodes 구조

use super::{NodeId, TabId, SplitDirection, DockTree, DuplicateConfig};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// 레이아웃 버전 (호환성 체크용)
/// v4: UE5 SDockingCross 스타일 (나침반 4방향, insert_index 지원)
pub const LAYOUT_VERSION: u32 = 4;

/// 최소 호환 레이아웃 버전 (S-08: UE5 MinCompatibleLayoutVersion)
///
/// `MIN_COMPATIBLE_VERSION..=LAYOUT_VERSION` 범위만 로드 허용.
/// 이 버전 미만의 레이아웃은 `migrate()` 없이 폐기.
pub const MIN_COMPATIBLE_VERSION: u32 = 3;

/// 도킹 레이아웃 (언리얼 FTabManager::FLayout)
///
/// 전체 도킹 상태를 저장하는 최상위 구조
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockLayout {
    /// 레이아웃 버전
    pub version: u32,
    /// 레이아웃 이름 (식별용)
    pub name: String,
    /// 루트 노드
    pub root: Option<LayoutNode>,
    /// 탭 정보 (ID → 탭 이름 매핑)
    pub tab_names: HashMap<u64, String>,
    /// 윈도우 배치 방식 (UE5 FArea::WindowPlacement — 플로팅 윈도우 위치/크기)
    #[serde(default)]
    pub window_placement: WindowPlacement,
}

impl DockLayout {
    /// 새 레이아웃 생성
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            version: LAYOUT_VERSION,
            name: name.into(),
            root: None,
            tab_names: HashMap::new(),
            window_placement: WindowPlacement::default(),
        }
    }

    /// 플로팅 윈도우 위치 설정 (UE5 FArea::SetWindow)
    pub fn set_window(&mut self, x: f32, y: f32, width: f32, height: f32) {
        self.window_placement = WindowPlacement::Specified { x, y, width, height };
    }

    /// 위치 지정 플로팅 윈도우인지 (UE5 DefinesPositionallySpecifiedFloatingWindow)
    pub fn defines_positionally_specified_floating_window(&self) -> bool {
        matches!(self.window_placement, WindowPlacement::Specified { .. })
    }

    /// JSON으로 직렬화 (언리얼 ToJson)
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// JSON에서 역직렬화 (언리얼 NewFromJson)
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// 버전 호환성 체크 (S-08: 범위 기반 — MIN_COMPATIBLE_VERSION..=LAYOUT_VERSION)
    pub fn is_compatible(&self) -> bool {
        (MIN_COMPATIBLE_VERSION..=LAYOUT_VERSION).contains(&self.version)
    }

    /// 마이그레이션 (S-08: 구 버전 → 현재 버전 변환)
    ///
    /// `is_compatible()`이 true인 경우에만 호출.
    /// 현재 버전이면 아무 작업 안 함. 구 버전이면 버전 번호만 갱신 (필드 기본값은 serde default로 처리).
    pub fn migrate(&mut self) {
        if self.version < LAYOUT_VERSION {
            // v3 → v4: 구조적 변경 없음 (새 필드는 serde default로 처리)
            self.version = LAYOUT_VERSION;
        }
    }

    /// 탭 확장 적용 (UE5 LayoutExtender — 타겟 탭 기준 Before/After 삽입)
    pub fn apply_tab_extensions(&mut self, extensions: &[LayoutTabExtension]) {
        if let Some(ref mut root) = self.root {
            for ext in extensions {
                Self::apply_extension_recursive(root, ext);
            }
        }
    }

    fn apply_extension_recursive(node: &mut LayoutNode, ext: &LayoutTabExtension) {
        match node {
            LayoutNode::Stack { tabs, .. } => {
                if let Some(idx) = tabs.iter().position(|t| t.tab_name == ext.target_tab_type) {
                    let insert_idx = match ext.position {
                        ExtensionPosition::Before => idx,
                        ExtensionPosition::After => idx + 1,
                    };
                    tabs.insert(insert_idx, ext.tab_to_insert.clone());
                }
            }
            LayoutNode::Splitter { nodes, .. } => {
                for child in nodes.iter_mut() {
                    Self::apply_extension_recursive(child, ext);
                }
            }
        }
    }

    /// 레이아웃 복제 (UE5 FTabManager::FLayout::Duplicate — DuplicateConfig)
    ///
    /// `Full`: 전체 deep-copy (탭 포함).
    /// `StructureOnly`: 스플리터/스택 구조만 복제, 탭 제거.
    pub fn duplicate(&self, config: DuplicateConfig) -> Self {
        let mut cloned = self.clone();
        if config == DuplicateConfig::StructureOnly {
            // 구조만 복제: 모든 스택의 탭 제거
            if let Some(ref mut root) = cloned.root {
                Self::strip_tabs_recursive(root);
            }
            cloned.tab_names.clear();
        }
        cloned
    }

    /// 재귀적으로 스택 노드의 탭 제거 (StructureOnly 복제용)
    fn strip_tabs_recursive(node: &mut LayoutNode) {
        match node {
            LayoutNode::Stack { tabs, active_tab_name, panel_drawer_active_tab, panel_drawer_inactive_tabs, .. } => {
                tabs.clear();
                *active_tab_name = None;
                *panel_drawer_active_tab = None;
                panel_drawer_inactive_tabs.clear();
            }
            LayoutNode::Splitter { nodes, .. } => {
                for child in nodes.iter_mut() {
                    Self::strip_tabs_recursive(child);
                }
            }
        }
    }

    /// JSON compact 직렬화 (INI config 저장용)
    pub fn save_to_config_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// JSON 파싱 + fallback (INI config 로드용)
    pub fn load_from_config_string(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}

impl Default for DockLayout {
    fn default() -> Self {
        Self::new("Default")
    }
}

/// 레이아웃 노드 (언리얼 FLayoutNode)
///
/// 재귀적 트리 구조로 도킹 상태 표현
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LayoutNode {
    /// 탭 스택 (언리얼 ELayoutNodeType::Stack)
    #[serde(rename = "Stack")]
    Stack {
        /// 노드 ID (복원 시 매핑용)
        node_id: u64,
        /// 포함된 탭들
        tabs: Vec<TabLayoutInfo>,
        /// 활성 탭 이름 (UE5 ForegroundTabId — 이름 기반, 인덱스 밀림 방지)
        #[serde(default)]
        active_tab_name: Option<String>,
        /// 크기 계수 (언리얼 SizeCoefficient)
        size_coefficient: f32,
        /// 탭 바 숨김 (UE HideTabWell)
        #[serde(default)]
        hide_tab_well: bool,
        /// 확장 식별자 (커스텀 레이아웃 메타데이터)
        #[serde(default)]
        extension_id: Option<String>,
        /// PanelDrawer 활성 탭 이름 (UE5 SetPanelDrawerActiveTab)
        #[serde(default)]
        panel_drawer_active_tab: Option<String>,
        /// PanelDrawer 비활성 탭 목록 (UE5 AddPanelDrawerInactiveTab)
        #[serde(default)]
        panel_drawer_inactive_tabs: Vec<String>,
        /// PanelDrawer 드로워 폭 (UE5 FPanelDrawerTab — 드로워 크기 영속화)
        #[serde(default)]
        panel_drawer_width: Option<f32>,
    },
    /// 분할자 (언리얼 ELayoutNodeType::Splitter)
    #[serde(rename = "Splitter")]
    Splitter {
        /// 노드 ID
        node_id: u64,
        /// 분할 방향 (언리얼 Orientation)
        orientation: SplitDirection,
        /// 자식 노드들
        nodes: Vec<LayoutNode>,
        /// 각 자식의 크기 계수
        coefficients: Vec<f32>,
        /// 확장 식별자 (커스텀 레이아웃 메타데이터)
        #[serde(default)]
        extension_id: Option<String>,
    },
}

impl LayoutNode {
    /// 노드 ID 가져오기
    pub fn node_id(&self) -> u64 {
        match self {
            Self::Stack { node_id, .. } => *node_id,
            Self::Splitter { node_id, .. } => *node_id,
        }
    }

    /// 스택 노드 생성
    pub fn new_stack(node_id: NodeId, tabs: Vec<TabLayoutInfo>, active_tab_name: Option<String>, size_coefficient: f32) -> Self {
        Self::Stack {
            node_id: node_id.0,
            tabs,
            active_tab_name,
            size_coefficient,
            hide_tab_well: false,
            extension_id: None,
            panel_drawer_active_tab: None,
            panel_drawer_inactive_tabs: Vec::new(),
            panel_drawer_width: None,
        }
    }

    /// 크기 계수 반환 (Stack이면 size_coefficient, Splitter면 1.0)
    pub fn size_coefficient(&self) -> f32 {
        match self {
            Self::Stack { size_coefficient, .. } => *size_coefficient,
            Self::Splitter { .. } => 1.0,
        }
    }

    /// 분할자 노드 생성
    pub fn new_splitter(
        node_id: NodeId,
        orientation: SplitDirection,
        nodes: Vec<LayoutNode>,
        coefficients: Vec<f32>,
    ) -> Self {
        Self::Splitter {
            node_id: node_id.0,
            orientation,
            nodes,
            coefficients,
            extension_id: None,
        }
    }
}

/// 탭 레이아웃 정보 (언리얼 FTab)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabLayoutInfo {
    /// 탭 ID
    pub tab_id: u64,
    /// 탭 이름 (식별용, 복원 시 매칭)
    pub tab_name: String,
    /// 탭 상태
    pub state: TabState,
    /// 사이드바 크기 계수 (UE5 SidebarSizeCoefficient)
    #[serde(default)]
    pub sidebar_size_coefficient: f32,
    /// 사이드바 고정 여부 (UE5 bPinnedInSidebar)
    #[serde(default)]
    pub pinned_in_sidebar: bool,
}

impl TabLayoutInfo {
    pub fn new(tab_id: TabId, tab_name: impl Into<String>) -> Self {
        Self {
            tab_id: tab_id.0,
            tab_name: tab_name.into(),
            state: TabState::Open,
            sidebar_size_coefficient: 0.0,
            pinned_in_sidebar: false,
        }
    }

    pub fn with_state(mut self, state: TabState) -> Self {
        self.state = state;
        self
    }
}

/// 탭 상태 (언리얼 ETabState — 비트플래그 호환)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabState {
    /// 열려있음 (UE5 OpenedTab = 0x1)
    #[serde(rename = "Open")]
    Open,
    /// 닫혀있음 — 복원 가능 (UE5 ClosedTab = 0x2)
    #[serde(rename = "Closed")]
    Closed,
    /// 사이드바로 최소화 (UE5 SidebarTab = 0x4)
    #[serde(rename = "Sidebar")]
    Sidebar,
    /// 무효 탭 — 플러그인 미로드 등 (UE5 InvalidTab = 0x8)
    ///
    /// 레이아웃 복원 시 인식 불가한 탭 ID에 할당.
    /// 플러그인 로드 후 Open으로 전환 가능.
    #[serde(rename = "Invalid")]
    Invalid,
}

impl Default for TabState {
    fn default() -> Self {
        Self::Open
    }
}

/// 플로팅 윈도우 레이아웃 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FloatingWindowLayout {
    /// DockTree 레이아웃 (분할 구조 포함)
    #[serde(default)]
    pub dock_tree: Option<DockTree>,
    /// 탭 목록 (v1 호환용, dock_tree 없을 때 사용)
    #[serde(default)]
    pub tabs: Vec<TabLayoutInfo>,
    /// 활성 탭 이름 (UE5 ForegroundTabId — 이름 기반)
    #[serde(default)]
    pub active_tab_name: Option<String>,
    /// 윈도우 위치 [x, y] (스크린 좌표)
    pub position: [f32; 2],
    /// 윈도우 크기 [width, height]
    pub size: [f32; 2],
    /// 최대화 상태 (UE5 bIsMaximized)
    #[serde(default)]
    pub is_maximized: bool,
}

/// 에디터 전체 레이아웃 (모든 MajorTab 포함)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorLayout {
    /// 레이아웃 버전
    pub version: u32,
    /// 레이아웃 이름
    pub name: String,
    /// 각 MajorTab의 레이아웃
    pub major_tabs: Vec<MajorTabLayout>,
    /// 활성 MajorTab 인덱스
    pub active_major: usize,
    /// 플로팅 윈도우 레이아웃
    #[serde(default)]
    pub floating_windows: Vec<FloatingWindowLayout>,
    /// 닫힌 플로팅 윈도우 레이아웃 보존 (UE5 CollapsedDockAreas)
    #[serde(default)]
    pub collapsed_areas: Vec<DockLayout>,
    /// 미인식 탭 보존 (UE5 InvalidDockAreas — 플러그인 미로드)
    #[serde(default)]
    pub invalid_tabs: Vec<TabLayoutInfo>,

    // ── 13차: FLayout 확장 필드 ──

    /// 레이아웃 이름 (UE5 FLayout::GetLayoutName — name과 별도의 표시 이름)
    #[serde(default)]
    pub layout_name: String,
    /// 기본 영역 인덱스 (UE5 FLayout::GetPrimaryArea)
    #[serde(default)]
    pub primary_area_index: Option<usize>,

    // ── 17차: FLayoutSaveRestore 갭 클로저 ──

    /// 추가 레이아웃 설정 INI 경로 (UE5 FLayoutSaveRestore::GetAdditionalLayoutConfigIni)
    #[serde(default)]
    pub additional_config_ini: Option<String>,
}

impl EditorLayout {
    /// JSON으로 직렬화
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// JSON에서 역직렬화
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// 버전 호환성 체크 (S-08: 범위 기반)
    pub fn is_compatible(&self) -> bool {
        (MIN_COMPATIBLE_VERSION..=LAYOUT_VERSION).contains(&self.version)
    }

    /// 마이그레이션 (S-08: 구 버전 → 현재 버전 변환)
    ///
    /// version 갱신 + 각 major_tab의 dock_layout도 마이그레이션.
    pub fn migrate(&mut self) {
        if self.version < LAYOUT_VERSION {
            self.version = LAYOUT_VERSION;
        }
        for major in &mut self.major_tabs {
            major.dock_layout.migrate();
        }
    }

    // ── 13차 Batch D: FLayout 확장 메서드 ──

    /// 레이아웃 이름 조회 (UE5 FLayout::GetLayoutName)
    pub fn get_layout_name(&self) -> &str {
        if self.layout_name.is_empty() { &self.name } else { &self.layout_name }
    }

    /// 확장 처리 (UE5 FLayout::ProcessExtensions)
    ///
    /// 각 MajorTab의 DockLayout에 탭 확장을 적용.
    pub fn process_extensions(&mut self, extensions: &[super::layout::LayoutTabExtension]) {
        for major in &mut self.major_tabs {
            major.dock_layout.apply_tab_extensions(extensions);
        }
    }

    /// 기본 영역 참조 (UE5 FLayout::GetPrimaryArea)
    pub fn get_primary_area(&self) -> Option<&MajorTabLayout> {
        let idx = self.primary_area_index.unwrap_or(0);
        self.major_tabs.get(idx)
    }

    /// 전체 영역 목록 (UE5 FLayout::GetAreas)
    pub fn get_areas(&self) -> &[MajorTabLayout] {
        &self.major_tabs
    }

    // ── 17차: FLayoutSaveRestore 갭 클로저 ──

    /// 추가 레이아웃 설정 INI 경로 (UE5 FLayoutSaveRestore::GetAdditionalLayoutConfigIni)
    ///
    /// 플러그인이나 프로젝트별 추가 레이아웃 설정 파일의 경로를 반환.
    pub fn get_additional_layout_config_ini(&self) -> Option<&str> {
        self.additional_config_ini.as_deref()
    }

    /// 추가 레이아웃 설정 INI 경로 설정 (UE5 FLayoutSaveRestore::SetAdditionalLayoutConfigIni)
    pub fn set_additional_layout_config_ini(&mut self, path: impl Into<String>) {
        self.additional_config_ini = Some(path.into());
    }

    // ── 18차: FLayoutSaveRestore 갭 클로저 — Batch D (2건) ──

    /// 설정 키로 레이아웃 저장 (UE5 FLayoutSaveRestore::SaveToConfig)
    ///
    /// (config_key, json) 튜플을 반환. 호출자가 실제 파일/DB에 저장.
    pub fn save_to_config(&self, config_key: &str) -> (String, String) {
        (config_key.to_string(), self.to_json().unwrap_or_default())
    }

    /// 설정 키로 레이아웃 로드 (UE5 FLayoutSaveRestore::LoadFromConfig)
    ///
    /// JSON 문자열에서 EditorLayout을 복원. 호출자가 config_key로 JSON을 조회.
    pub fn load_from_config(json: &str) -> Option<Self> {
        Self::from_json(json).ok()
    }
}

/// 개별 MajorTab 레이아웃
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MajorTabLayout {
    /// MajorTab 제목
    pub title: String,
    /// 아이콘
    pub icon: Option<String>,
    /// 닫기 가능 여부
    pub closable: bool,
    /// 내부 DockTree 레이아웃
    pub dock_layout: DockLayout,
    /// 왼쪽 사이드바 탭들
    #[serde(default)]
    pub left_sidebar_tabs: Vec<SidebarTabLayoutInfo>,
    /// 오른쪽 사이드바 탭들
    #[serde(default)]
    pub right_sidebar_tabs: Vec<SidebarTabLayoutInfo>,
}

/// 사이드바 탭 직렬화 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidebarTabLayoutInfo {
    /// 탭 타입명 (스포너 복원용)
    pub tab_type_name: String,
    /// 표시 이름
    pub display_name: String,
    /// 아이콘
    pub icon: Option<String>,
}

/// 레이아웃 프리셋 (저장된 레이아웃 설정)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutPreset {
    /// 프리셋 이름
    pub name: String,
    /// 설명
    pub description: String,
    /// 빌트인 여부 (빌트인은 삭제 불가)
    pub is_builtin: bool,
    /// 직렬화된 레이아웃 JSON
    pub layout_json: String,
}

/// 레이아웃 프리셋 레지스트리
#[derive(Debug, Clone, Default)]
pub struct LayoutPresetRegistry {
    presets: Vec<LayoutPreset>,
}

impl LayoutPresetRegistry {
    pub fn new() -> Self {
        Self { presets: Vec::new() }
    }

    /// 빌트인 프리셋 등록
    pub fn register_builtin(&mut self, name: impl Into<String>, description: impl Into<String>, layout_json: String) {
        self.presets.push(LayoutPreset {
            name: name.into(),
            description: description.into(),
            is_builtin: true,
            layout_json,
        });
    }

    /// 사용자 프리셋 등록 (빌트인 이름 충돌 시 거부)
    pub fn register_user(&mut self, name: impl Into<String>, description: impl Into<String>, layout_json: String) -> bool {
        let name = name.into();
        // 빌트인 이름과 충돌 시 등록 거부
        if self.presets.iter().any(|p| p.name == name && p.is_builtin) {
            return false;
        }
        // 같은 이름의 사용자 프리셋 덮어쓰기
        self.presets.retain(|p| p.name != name);
        self.presets.push(LayoutPreset {
            name,
            description: description.into(),
            is_builtin: false,
            layout_json,
        });
        true
    }

    /// 프리셋 조회
    pub fn get(&self, name: &str) -> Option<&LayoutPreset> {
        self.presets.iter().find(|p| p.name == name)
    }

    /// 모든 프리셋 목록
    pub fn list(&self) -> &[LayoutPreset] {
        &self.presets
    }

    /// 사용자 프리셋 삭제 (빌트인은 삭제 불가)
    pub fn remove_user(&mut self, name: &str) -> bool {
        let before = self.presets.len();
        self.presets.retain(|p| p.name != name || p.is_builtin);
        self.presets.len() < before
    }

    /// 전체 프리셋을 JSON으로 직렬화
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.presets)
    }

    /// JSON에서 사용자 프리셋 로드 (빌트인은 유지, 사용자 프리셋만 추가/덮어쓰기)
    pub fn from_json(&mut self, json: &str) -> Result<(), serde_json::Error> {
        let loaded: Vec<LayoutPreset> = serde_json::from_str(json)?;
        for preset in loaded {
            if !preset.is_builtin {
                self.register_user(preset.name, preset.description, preset.layout_json);
            }
        }
        Ok(())
    }

    /// 파일에 프리셋 저장
    pub fn save_to_file(&self, path: impl AsRef<std::path::Path>) -> Result<(), std::io::Error> {
        let json = self.to_json()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, json)
    }

    /// 파일에서 프리셋 로드
    pub fn load_from_file(&mut self, path: impl AsRef<std::path::Path>) -> Result<(), Box<dyn std::error::Error>> {
        let json = std::fs::read_to_string(path)?;
        self.from_json(&json)?;
        Ok(())
    }

    /// 디렉토리에 모든 프리셋 저장
    pub fn save_all_to_directory(&self, dir: &std::path::Path) -> Result<(), std::io::Error> {
        std::fs::create_dir_all(dir)?;
        for preset in &self.presets {
            let path = dir.join(format!("{}.json", preset.name));
            std::fs::write(&path, &preset.layout_json)?;
        }
        Ok(())
    }

    /// 디렉토리에서 프리셋 로드
    pub fn load_all_from_directory(&mut self, dir: &std::path::Path) -> Result<(), std::io::Error> {
        if !dir.exists() { return Ok(()); }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(json) = std::fs::read_to_string(&path) {
                        self.register_user(name, "", json);
                    }
                }
            }
        }
        Ok(())
    }
}

/// 윈도우 배치 정책 (UE5 FArea::EWindowPlacement)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WindowPlacement {
    /// 윈도우 없음 (메인 윈도우에 도킹)
    NoWindow,
    /// 자동 배치 (OS가 결정)
    Automatic,
    /// 지정된 위치/크기로 배치
    Specified {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
}

impl Default for WindowPlacement {
    fn default() -> Self {
        Self::Automatic
    }
}

/// 탭 기반 레이아웃 확장 위치 (UE5 LayoutExtender Before/After)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExtensionPosition {
    /// 타겟 앞에 삽입
    Before,
    /// 타겟 뒤에 삽입
    After,
}

/// 탭 레이아웃 확장 (UE5 FTabManager::FLayoutExtender — 탭 단위)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutTabExtension {
    /// 타겟 탭 타입명
    pub target_tab_type: String,
    /// 삽입 위치
    pub position: ExtensionPosition,
    /// 삽입할 탭 정보
    pub tab_to_insert: TabLayoutInfo,
}

/// 스택 레이아웃 확장 (UE5 FTabManager::FLayoutExtender — 스택 단위)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutStackExtension {
    /// 타겟 스택 확장 식별자
    pub target_stack_extension_id: String,
    /// 삽입 위치
    pub position: ExtensionPosition,
    /// 삽입할 탭 정보
    pub tab: TabLayoutInfo,
}

/// 영역 레이아웃 확장 (UE5 FLayoutExtender::ExtendArea — 영역 단위)
#[derive(Clone)]
pub struct LayoutAreaExtension {
    /// 타겟 영역 확장 식별자
    pub target_area_extension_id: String,
    /// 확장 콜백 (DockArea 복원 시 호출)
    pub callback: std::sync::Arc<dyn Fn(NodeId) + Send + Sync>,
}

impl DockLayout {
    /// 영역 수준 확장 적용 (UE5 FLayoutExtender::ExtendArea)
    ///
    /// 영역 확장 콜백을 루트 노드의 모든 스택에 전파.
    pub fn apply_area_extensions(&self, extensions: &[LayoutAreaExtension], area_id: NodeId) {
        for ext in extensions {
            (ext.callback)(area_id);
        }
    }

    /// 스택 확장 동적 쿼리 (UE5 FLayoutExtender::FindStackExtensions)
    ///
    /// 주어진 extension_id에 매칭되는 스택 확장을 검색.
    pub fn find_stack_extensions<'a>(
        &self,
        extensions: &'a [LayoutStackExtension],
        extension_id: &str,
    ) -> Vec<&'a LayoutStackExtension> {
        extensions
            .iter()
            .filter(|ext| ext.target_stack_extension_id == extension_id)
            .collect()
    }
}

/// 사이드바 탭 복원 정보
#[derive(Debug, Clone)]
pub struct SidebarRestoreInfo {
    pub tab_name: String,
    pub sidebar_size_coefficient: f32,
    pub pinned: bool,
    /// 사이드바 위치 (UE5 ESidebarLocation)
    pub side: super::SidebarSide,
}

/// 축소된 DockArea 정보 (UE5 CollapsedDockAreas)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollapsedAreaInfo {
    /// 축소된 영역의 레이아웃
    pub layout: DockLayout,
    /// 축소 시점의 탭 목록
    pub tabs: Vec<TabLayoutInfo>,
}

impl CollapsedAreaInfo {
    pub fn new(layout: DockLayout, tabs: Vec<TabLayoutInfo>) -> Self {
        Self { layout, tabs }
    }

    /// 특정 탭 타입이 포함되어 있는지 검색
    pub fn contains_tab_type(&self, tab_type: &str) -> bool {
        self.tabs.iter().any(|t| t.tab_name == tab_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_serialization() {
        let mut layout = DockLayout::new("TestLayout");

        // 스택 노드 추가
        let tab1 = TabLayoutInfo::new(TabId(1), "Viewport");
        let tab2 = TabLayoutInfo::new(TabId(2), "Hierarchy");

        let stack = LayoutNode::new_stack(
            NodeId::new(100),
            vec![tab1, tab2],
            Some("Viewport".to_string()),
            1.0,
        );

        layout.root = Some(stack);
        layout.tab_names.insert(1, "Viewport".to_string());
        layout.tab_names.insert(2, "Hierarchy".to_string());

        // JSON 직렬화
        let json = layout.to_json().expect("Serialization failed");
        println!("JSON:\n{}", json);

        // JSON 역직렬화
        let restored = DockLayout::from_json(&json).expect("Deserialization failed");

        assert_eq!(restored.version, LAYOUT_VERSION);
        assert_eq!(restored.name, "TestLayout");
        assert!(restored.root.is_some());
    }

    #[test]
    fn test_splitter_layout() {
        let mut layout = DockLayout::new("SplitterTest");

        let left_stack = LayoutNode::new_stack(
            NodeId::new(1),
            vec![TabLayoutInfo::new(TabId(1), "Left")],
            Some("Left".to_string()),
            0.3,
        );

        let right_stack = LayoutNode::new_stack(
            NodeId::new(2),
            vec![TabLayoutInfo::new(TabId(2), "Right")],
            Some("Right".to_string()),
            0.7,
        );

        let splitter = LayoutNode::new_splitter(
            NodeId::new(100),
            SplitDirection::Horizontal,
            vec![left_stack, right_stack],
            vec![0.3, 0.7],
        );

        layout.root = Some(splitter);

        let json = layout.to_json().expect("Serialization failed");
        println!("Splitter JSON:\n{}", json);

        let restored = DockLayout::from_json(&json).expect("Deserialization failed");

        if let Some(LayoutNode::Splitter { orientation, nodes, coefficients, .. }) = restored.root {
            assert_eq!(orientation, SplitDirection::Horizontal);
            assert_eq!(nodes.len(), 2);
            assert_eq!(coefficients.len(), 2);
        } else {
            panic!("Expected Splitter node");
        }
    }
}
